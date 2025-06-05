use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;
use crate::repository::{Repository, objects, refs};

#[derive(Debug)]
struct FileStatus {
    staged: Option<String>,    // Object ID in index, None if not staged
    head: Option<String>,      // Object ID in HEAD, None if not in HEAD
    working: Option<String>,   // Object ID in working dir, None if deleted
}

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    let _repo = Repository::open(&current_dir)?;
    
    #[cfg(not(feature = "online_judge"))] {
        println!("On branch {}", _repo.current_branch()?);
    
        // Get files from HEAD commit
        let head_files = get_head_files(&_repo)?;
        
        // Get files from index
        let index_files = get_index_files(&_repo);
        
        // Get files from working directory
        let working_files = get_working_files(&_repo)?;
        
        // Debug: show what's actually in the index
        println!("DEBUG: Current index contents:");
        for (path, object_id) in &index_files {
            println!("  '{}' -> {}", path.display(), &object_id[..8]);
        }
        
        // Combine all file paths
        let mut all_files: HashSet<PathBuf> = HashSet::new();
        all_files.extend(head_files.keys().cloned());
        all_files.extend(index_files.keys().cloned());
        all_files.extend(working_files.keys().cloned());
        
        // Categorize files
        let mut staged_changes = Vec::new();
        let mut unstaged_changes = Vec::new();
        let mut untracked_files = Vec::new();
        
        for file_path in all_files {
            let head_id = head_files.get(&file_path).cloned();
            let index_id = index_files.get(&file_path).cloned();
            let working_id = working_files.get(&file_path).cloned();
            
            // Check if file is untracked (not in HEAD or index)
            if head_id.is_none() && index_id.is_none() && working_id.is_some() {
                untracked_files.push(file_path.to_string_lossy().to_string());
                continue;
            }
            
            // Check staged changes (index vs HEAD)
            if index_id != head_id {
                let status = match (&head_id, &index_id) {
                    (None, Some(_)) => "new file",
                    (Some(_), None) => "deleted",
                    (Some(_), Some(_)) => "modified",
                    (None, None) => continue, // shouldn't happen
                };
                staged_changes.push((file_path.to_string_lossy().to_string(), status));
            }
            
            // Check unstaged changes (working vs index)
            if working_id != index_id {
                let status = match (&index_id, &working_id) {
                    (Some(_), None) => "deleted",
                    (Some(_), Some(_)) => "modified",
                    (None, Some(_)) => continue, // untracked, already handled
                    (None, None) => continue, // shouldn't happen
                };
                unstaged_changes.push((file_path.to_string_lossy().to_string(), status));
            }
        }
        
        // Print results
        let has_staged = !staged_changes.is_empty();
        let has_unstaged = !unstaged_changes.is_empty();
        let has_untracked = !untracked_files.is_empty();
        
        if has_staged {
            println!("\nChanges to be committed:");
            println!("  (use \"rust-git rm <file>...\" to unstage)");
            println!();
            for (file, status) in staged_changes {
                println!("\t{}: {}", status, file);
            }
        }
        
        if has_unstaged {
            println!("\nChanges not staged for commit:");
            println!("  (use \"rust-git add <file>...\" to update what will be committed)");
            println!("  (use \"rust-git checkout -- <file>...\" to discard changes in working directory)");
            println!();
            for (file, status) in unstaged_changes {
                println!("\t{}: {}", status, file);
            }
        }
        
        if has_untracked {
            println!("\nUntracked files:");
            println!("  (use \"rust-git add <file>...\" to include in what will be committed)");
            println!();
            for file in untracked_files {
                println!("\t{}", file);
            }
            if !has_staged && !has_unstaged {
                println!("\nnothing added to commit but untracked files present (use \"rust-git add\" to track)");
            }
        }
        
        if !has_staged && !has_unstaged && !has_untracked {
            println!("\nnothing to commit, working tree clean");
        }
    }
    Ok(())
}

fn get_head_files(repo: &Repository) -> Result<HashMap<PathBuf, String>> {
    let mut files = HashMap::new();
    
    if let Ok(head_commit_id) = refs::get_head_commit(&repo.git_dir) {
        if let Ok(commit_obj) = objects::read_object(repo, &head_commit_id) {
            if commit_obj.object_type == "commit" {
                let commit_content = String::from_utf8_lossy(&commit_obj.data);
                let lines: Vec<&str> = commit_content.lines().collect();
                if !lines.is_empty() && lines[0].starts_with("tree ") {
                    let tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
                    
                    if let Ok(tree_obj) = objects::read_object(repo, tree_id) {
                        if tree_obj.object_type == "tree" {
                            parse_tree_entries(&tree_obj.data, &mut files)?;
                        }
                    }
                }
            }
        }
    }
    
    Ok(files)
}

fn get_index_files(repo: &Repository) -> HashMap<PathBuf, String> {
    let mut files = HashMap::new();
    
    for (path, entry) in repo.index.get_entries() {
        // Paths in index are already normalized, just use them directly
        files.insert(path.clone(), entry.object_id.clone());
    }
    
    files
}

fn get_working_files(repo: &Repository) -> Result<HashMap<PathBuf, String>> {
    let mut files = HashMap::new();
    
    for entry in WalkDir::new(&repo.path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        
        // Skip .git directory
        if path.to_string_lossy().contains("/.git/") {
            continue;
        }
        
        let relative_path = if path.starts_with(&repo.path) {
            path.strip_prefix(&repo.path)?
        } else {
            path
        };
        
        // Use the unified normalize_path function
        let normalized_path = crate::repository::normalize_path(relative_path);
        let content = fs::read(path)?;
        let object_id = objects::hash_object(&content, "blob");
        
        files.insert(normalized_path, object_id);
    }
    
    Ok(files)
}

fn parse_tree_entries(tree_data: &[u8], files: &mut HashMap<PathBuf, String>) -> Result<()> {
    let mut cursor = 0;
    
    while cursor < tree_data.len() {
        // Find space after mode
        if let Some(space_idx) = tree_data[cursor..].iter().position(|&b| b == b' ') {
            let space_idx = space_idx + cursor;
            
            // Find null after filename
            if let Some(null_idx) = tree_data[space_idx + 1..].iter().position(|&b| b == 0) {
                let null_idx = null_idx + space_idx + 1;
                let filename = std::str::from_utf8(&tree_data[space_idx + 1..null_idx])?;
                
                // Get SHA1 hash (next 20 bytes)
                let sha1_start = null_idx + 1;
                let sha1_end = sha1_start + 20;
                if sha1_end <= tree_data.len() {
                    let sha1_bytes = &tree_data[sha1_start..sha1_end];
                    let sha1_hex = hex::encode(sha1_bytes);
                    
                    // Normalize the path before inserting
                    let normalized_path = crate::repository::normalize_path(&PathBuf::from(filename));
                    files.insert(normalized_path, sha1_hex);
                    cursor = sha1_end;
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    Ok(())
} 