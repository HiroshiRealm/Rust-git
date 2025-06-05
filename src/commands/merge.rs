use anyhow::Result;
use std::env;
use crate::repository::{Repository, refs, objects};
use std::collections::HashMap;
use std::path::Path;
use hex;

// Helper function to get tree files (filename -> object_id map) from a commit_id
fn get_files_from_commit(repo: &Repository, commit_id: &str) -> Result<HashMap<String, String>> {
    let commit_obj = objects::read_object(repo, commit_id)?;
    if commit_obj.object_type != "commit" {
        anyhow::bail!("Expected commit object, got {}", commit_obj.object_type);
    }
    let commit_content = String::from_utf8_lossy(&commit_obj.data);
    let lines: Vec<&str> = commit_content.lines().collect();
    if lines.is_empty() || !lines[0].starts_with("tree ") {
        anyhow::bail!("Invalid commit object format for commit {}", commit_id);
    }
    let tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
    
    // This is a simplified version of get_tree_files from checkout.rs
    // It assumes files are at the root of the tree for simplicity, as per typical Git usage for simple cases.
    // A full implementation would handle nested trees (directories).
    get_tree_content(repo, tree_id)
}

// Helper function to parse tree object content (similar to get_tree_files in checkout.rs)
// For simplicity, this version assumes all entries are blobs (files) and not trees (directories)
// and that filenames do not contain null bytes or other problematic characters.
fn get_tree_content(repo: &Repository, tree_id: &str) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();
    let tree_obj = objects::read_object(repo, tree_id)?;
    if tree_obj.object_type != "tree" {
        anyhow::bail!("Expected tree object for ID {}, got {}", tree_id, tree_obj.object_type);
    }

    let mut cursor = 0;
    while cursor < tree_obj.data.len() {
        // Format: <mode> <filename>\0<sha1_hash_20_bytes>
        // Find space after mode
        if let Some(space_idx) = tree_obj.data[cursor..].iter().position(|&b| b == b' ') {
            let name_start_idx = cursor + space_idx + 1;
            // Find null after filename
            if let Some(null_idx_rel) = tree_obj.data[name_start_idx..].iter().position(|&b| b == 0) {
                let null_idx_abs = name_start_idx + null_idx_rel;
                let filename_bytes = &tree_obj.data[name_start_idx..null_idx_abs];
                let filename = String::from_utf8(filename_bytes.to_vec())
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in filename: {}", e))?;

                let sha1_start = null_idx_abs + 1;
                let sha1_end = sha1_start + 20; // SHA-1 hash is 20 bytes
                if sha1_end <= tree_obj.data.len() {
                    let sha1_bytes = &tree_obj.data[sha1_start..sha1_end];
                    let sha1_hex = hex::encode(sha1_bytes);
                    files.insert(filename, sha1_hex);
                    cursor = sha1_end;
                } else {
                    // Not enough data for SHA1 hash, indicates malformed tree or end of data
                    break; 
                }
            } else {
                // No null terminator found for filename, malformed tree
                break;
            }
        } else {
            // No space found for mode, malformed tree or end of data
            break;
        }
    }
    Ok(files)
}

// Helper function to find the merge base (common ancestor) of two commits
// This is a simplified implementation that finds the most recent common ancestor
fn find_merge_base(repo: &Repository, commit1: &str, commit2: &str) -> Result<Option<String>> {
    // For simplicity, we'll implement a basic algorithm
    // In a real Git implementation, this would be more sophisticated
    
    // Get all ancestors of commit1
    let mut ancestors1 = std::collections::HashSet::new();
    let mut queue = vec![commit1.to_string()];
    
    while let Some(commit_id) = queue.pop() {
        if ancestors1.contains(&commit_id) {
            continue;
        }
        ancestors1.insert(commit_id.clone());
        
        // Get parents of this commit
        if let Ok(commit_obj) = objects::read_object(repo, &commit_id) {
            if commit_obj.object_type == "commit" {
                let commit_content = String::from_utf8_lossy(&commit_obj.data);
                for line in commit_content.lines() {
                    if line.starts_with("parent ") {
                        let parent_id = line.strip_prefix("parent ").unwrap().trim();
                        queue.push(parent_id.to_string());
                    }
                }
            }
        }
    }
    
    // Find first common ancestor in commit2's ancestry
    let mut queue = vec![commit2.to_string()];
    let mut visited = std::collections::HashSet::new();
    
    while let Some(commit_id) = queue.pop() {
        if visited.contains(&commit_id) {
            continue;
        }
        visited.insert(commit_id.clone());
        
        if ancestors1.contains(&commit_id) {
            return Ok(Some(commit_id));
        }
        
        // Get parents of this commit
        if let Ok(commit_obj) = objects::read_object(repo, &commit_id) {
            if commit_obj.object_type == "commit" {
                let commit_content = String::from_utf8_lossy(&commit_obj.data);
                for line in commit_content.lines() {
                    if line.starts_with("parent ") {
                        let parent_id = line.strip_prefix("parent ").unwrap().trim();
                        queue.push(parent_id.to_string());
                    }
                }
            }
        }
    }
    
    Ok(None)
}

pub fn execute(branch_to_merge: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let mut repo = Repository::open(&current_dir)?;
    let current_branch_name = repo.current_branch()?;

    // Check if trying to merge onto itself
    if current_branch_name == branch_to_merge {
        #[cfg(not(feature = "online_judge"))]
        println!("Already on '{}'", branch_to_merge);
        return Ok(());
    }

    // Get commit IDs
    let current_branch_commit_id = refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", current_branch_name))?;
    let merge_branch_commit_id = match refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", branch_to_merge)) {
        Ok(id) => id,
        Err(_) => anyhow::bail!("Branch '{}' not found", branch_to_merge),
    };

    if current_branch_commit_id == merge_branch_commit_id {
        #[cfg(not(feature = "online_judge"))]
        println!("Already up-to-date.");
        return Ok(());
    }

    // Find merge base (common ancestor)
    let merge_base = find_merge_base(&repo, &current_branch_commit_id, &merge_branch_commit_id)?;
    
    // Get file lists for three versions
    let current_files = get_files_from_commit(&repo, &current_branch_commit_id)?;
    let merge_files = get_files_from_commit(&repo, &merge_branch_commit_id)?;
    let base_files = if let Some(base_commit) = &merge_base {
        get_files_from_commit(&repo, base_commit)?
    } else {
        HashMap::new() // No common ancestor, treat as empty
    };

    let mut conflict_found = false;
    let mut merged_files = HashMap::new();

    // Combine all filenames from all three versions
    let mut all_filenames = std::collections::HashSet::new();
    all_filenames.extend(current_files.keys().cloned());
    all_filenames.extend(merge_files.keys().cloned());
    all_filenames.extend(base_files.keys().cloned());

    for filename in all_filenames {
        let base_id = base_files.get(&filename);
        let current_id = current_files.get(&filename);
        let merge_id = merge_files.get(&filename);

        match (base_id, current_id, merge_id) {
            // File exists in all three versions
            (Some(base), Some(current), Some(merge)) => {
                if base == current && base == merge {
                    // No changes in either branch
                    merged_files.insert(filename.clone(), current.clone());
                } else if base == current && base != merge {
                    // Only changed in merge branch, use merge version
                    merged_files.insert(filename.clone(), merge.clone());
                } else if base != current && base == merge {
                    // Only changed in current branch, use current version
                    merged_files.insert(filename.clone(), current.clone());
                } else if current == merge {
                    // Both branches made same change
                    merged_files.insert(filename.clone(), current.clone());
                } else {
                    // Both branches changed differently - conflict!
                    conflict_found = true;
                    
                    // For line-level conflict detection, compare file contents
                    let current_obj = objects::read_object(&repo, current)?;
                    let current_content = String::from_utf8_lossy(&current_obj.data);
                    let current_lines: Vec<&str> = current_content.lines().collect();

                    let merge_obj = objects::read_object(&repo, merge)?;
                    let merge_content = String::from_utf8_lossy(&merge_obj.data);
                    let merge_lines: Vec<&str> = merge_content.lines().collect();
                    
                    // Find conflicting line ranges
                    let max_len = std::cmp::max(current_lines.len(), merge_lines.len());
                    let mut i = 0;
                    while i < max_len {
                        let current_line = current_lines.get(i);
                        let merge_line = merge_lines.get(i);

                        if current_line != merge_line {
                            let conflict_start = i + 1;
                            let mut conflict_end = conflict_start;
                            i += 1;
                            
                            while i < max_len {
                                let cl = current_lines.get(i);
                                let ml = merge_lines.get(i);
                                if cl != ml {
                                    conflict_end = i + 1;
                                    i += 1;
                                } else {
                                    break;
                                }
                            }
                            
                            if conflict_start == conflict_end {
                                println!("Merge conflict in {}: {}", filename, conflict_start);
                            } else {
                                println!("Merge conflict in {}: [{}, {}]", filename, conflict_start, conflict_end);
                            }
                        } else {
                            i += 1;
                        }
                    }
                    
                    // For now, use current version in merged result (could be improved)
                    merged_files.insert(filename.clone(), current.clone());
                }
            }
            // File exists in base and current, but not in merge (deleted in merge)
            (Some(base), Some(current), None) => {
                if base == current {
                    // Not modified in current, deleted in merge - accept deletion
                    // Don't add to merged_files
                } else {
                    // Modified in current, deleted in merge - conflict
                    conflict_found = true;
                    println!("Merge conflict in {}: modified in current branch but deleted in merge branch", filename);
                    // Keep current version
                    merged_files.insert(filename.clone(), current.clone());
                }
            }
            // File exists in base and merge, but not in current (deleted in current)
            (Some(base), None, Some(merge)) => {
                if base == merge {
                    // Not modified in merge, deleted in current - accept deletion
                    // Don't add to merged_files
                } else {
                    // Modified in merge, deleted in current - conflict
                    conflict_found = true;
                    println!("Merge conflict in {}: modified in merge branch but deleted in current branch", filename);
                    // Use merge version
                    merged_files.insert(filename.clone(), merge.clone());
                }
            }
            // File exists only in current and merge (new in both)
            (None, Some(current), Some(merge)) => {
                if current == merge {
                    // Same new file in both branches
                    merged_files.insert(filename.clone(), current.clone());
                } else {
                    // Different new files - conflict
                    conflict_found = true;
                    println!("Merge conflict in {}: different versions of new file", filename);
                    merged_files.insert(filename.clone(), current.clone());
                }
            }
            // File exists only in current (new in current)
            (None, Some(current), None) => {
                merged_files.insert(filename.clone(), current.clone());
            }
            // File exists only in merge (new in merge)
            (None, None, Some(merge)) => {
                merged_files.insert(filename.clone(), merge.clone());
            }
            // File exists only in base (deleted in both) - ignore
            (Some(_), None, None) => {
                // Both branches deleted the file, accept deletion
            }
            // Should not happen
            (None, None, None) => {}
        }
    }

    if conflict_found {
        #[cfg(not(feature = "online_judge"))]
        println!("Merge conflicts detected. Please resolve conflicts manually.");
        return Ok(());
    }

    // If no conflicts, perform the actual merge
    #[cfg(not(feature = "online_judge"))]
    println!("Merge successful. No conflicts found.");
    
    // Update working directory with merged files
    // Remove files that exist in current but not in merged result
    for (filename, _) in &current_files {
        if !merged_files.contains_key(filename) {
            let file_path = repo.path.join(filename);
            if file_path.is_file() {
                std::fs::remove_file(&file_path)?;
            }
        }
    }
    
    // Add/update files in working directory
    for (filename, object_id) in &merged_files {
        let obj = objects::read_object(&repo, object_id)?;
        if obj.object_type == "blob" {
            let file_path = repo.path.join(filename);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file_path, &obj.data)?;
            
            // Update index
            repo.index.add_file(&repo.path, &file_path, object_id)?;
        }
    }
    
    // Create merge commit
    let current_tree_id = objects::write_tree(&repo)?;
    let merge_commit_id = objects::write_commit(
        &repo.git_dir,
        &current_tree_id,
        &[&current_branch_commit_id, &merge_branch_commit_id], // Two parents for merge commit
        &format!("Merge branch '{}' into {}", branch_to_merge, current_branch_name),
        "Rust-git <user@example.com>",
    )?;
    
    // Update current branch ref
    refs::update_ref(
        &repo.git_dir,
        &format!("refs/heads/{}", current_branch_name),
        &merge_commit_id,
    )?;
    
    // Save updated index
    repo.index.save(repo.git_dir.join("index"))?;

    Ok(())
} 