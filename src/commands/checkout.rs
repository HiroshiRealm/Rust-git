use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use hex;
use crate::repository::{Repository, refs, objects};

pub fn execute(branch_name: &str, create_branch_flag: bool) -> Result<()> {
    let current_dir = env::current_dir()?;
    let mut repo = Repository::open(&current_dir)?;

    if create_branch_flag {
        // Check if branch already exists
        let branch_path = repo.git_dir.join("refs/heads").join(branch_name);
        if branch_path.exists() {
            anyhow::bail!("Branch '{}' already exists", branch_name);
        }

        let head_commit = refs::get_head_commit(&repo.git_dir)?;
        refs::create_branch(&repo.git_dir, branch_name, &head_commit)?;
        #[cfg(not(feature = "online_judge"))]
        println!("Switched to a new branch '{}'", branch_name);
    } else {
        // Check if the branch exists
        let branch_path = repo.git_dir.join("refs/heads").join(branch_name);
        if !branch_path.exists() {
            anyhow::bail!("Branch '{}' not found. If you want to create it, use -b option.", branch_name);
        }
        #[cfg(not(feature = "online_judge"))]
        println!("Switched to branch '{}'", branch_name);
    }

    // Get current HEAD commit BEFORE updating HEAD
    let current_head_commit = refs::get_head_commit(&repo.git_dir).ok();
    
    // Update HEAD to point to the new branch
    fs::write(
        repo.git_dir.join("HEAD"),
        format!("ref: refs/heads/{}\n", branch_name),
    )?;
    
    // Update working directory and index to match the target branch
    update_working_directory_and_index(&mut repo, branch_name, current_head_commit)?;
    
    Ok(())
}

fn update_working_directory_and_index(repo: &mut Repository, branch_name: &str, current_head_commit: Option<String>) -> Result<()> {
    // Get the commit ID for the target branch
    let target_commit_id = refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", branch_name))?;
    
    // Get current HEAD tree files (if exists)
    let current_tree_files = if let Some(current_head_commit_id) = current_head_commit {
        let (commit_type, commit_data) = objects::read_object(&repo.git_dir.join("objects"), &current_head_commit_id)?;
        if commit_type == "commit" {
            let commit_content = String::from_utf8_lossy(&commit_data);
            let lines: Vec<&str> = commit_content.lines().collect();
            if !lines.is_empty() && lines[0].starts_with("tree ") {
                let current_tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
                get_tree_files(&repo.git_dir.join("objects"), current_tree_id)?
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };
    
    // Get target branch tree files
    let (commit_type, commit_data) = objects::read_object(&repo.git_dir.join("objects"), &target_commit_id)?;
    if commit_type != "commit" {
        anyhow::bail!("Expected commit object, got {}", commit_type);
    }
    
    let commit_content = String::from_utf8_lossy(&commit_data);
    let lines: Vec<&str> = commit_content.lines().collect();
    if lines.is_empty() || !lines[0].starts_with("tree ") {
        anyhow::bail!("Invalid commit object format");
    }
    
    let target_tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
    let target_tree_files = get_tree_files(&repo.git_dir.join("objects"), target_tree_id)?;
    
    // Step 1: Remove files that exist in current tree but not in target tree
    for (file_path, _) in &current_tree_files {
        if !target_tree_files.contains_key(file_path.as_path()) {
            let full_path = repo.path.join(file_path);
            if full_path.is_file() {
                match fs::remove_file(&full_path) {
                    Ok(_) => {},
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, // Already gone
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    
    // Step 2: Add/update files from target tree
    for (file_path, object_id) in &target_tree_files {
        let (obj_type, blob_data) = objects::read_object(&repo.git_dir.join("objects"), object_id)?;
        if obj_type != "blob" {
            continue; // Skip non-blob objects
        }
        
        // Write file to working directory
        let full_path = repo.path.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &blob_data)?;
        
        // Step 3: Update index only if the file is different from current tree
        // or if it's not in the current tree at all
        let should_update_index = match current_tree_files.get(file_path.as_path()) {
            Some(current_object_id) => current_object_id != object_id, // Different content
            None => true, // New file in target branch
        };
        
        if should_update_index {
            repo.index.add_file(&repo.path, &full_path, object_id)?;
        }
        // If the file is the same in both trees, preserve whatever is in the index
    }
    
    // Step 4: Remove index entries for files that no longer exist in target tree
    // but preserve staged changes for files that still exist
    let mut paths_to_remove_from_index = Vec::new();
    for (indexed_path, _) in repo.index.get_entries() {
        // If this path was in the current tree but not in target tree,
        // and it's not a staged change (i.e., it matches the current tree),
        // then remove it from index
        if let Some(current_object_id) = current_tree_files.get(indexed_path.as_path()) {
            if !target_tree_files.contains_key(indexed_path.as_path()) {
                // File was removed in target branch
                let index_entry = repo.index.get_entries().get(indexed_path).unwrap();
                if &index_entry.object_id == current_object_id {
                    // Index matches current tree, so this is not a staged change
                    paths_to_remove_from_index.push(indexed_path.clone());
                }
                // If index doesn't match current tree, it's a staged change - preserve it
            }
        }
    }
    
    // Remove the identified paths from index
    for path in paths_to_remove_from_index {
        repo.index.remove_path(&repo.path, &path)?;
    }
    
    // Step 5: Save the updated index
    repo.index.save(repo.git_dir.join("index"))?;
    
    Ok(())
}

// Modified to return HashMap<PathBuf, String>
fn get_tree_files(objects_dir: &Path, tree_id: &str) -> Result<HashMap<PathBuf, String>> {
    let mut files = HashMap::new();
    
    let (tree_type, tree_data) = objects::read_object(objects_dir, tree_id)?;
    if tree_type != "tree" {
        anyhow::bail!("Expected tree object, got {}", tree_type);
    }
    
    let mut cursor = 0;
    while cursor < tree_data.len() {
        if let Some(space_idx) = tree_data[cursor..].iter().position(|&b| b == b' ') {
            let space_idx_abs = space_idx + cursor;
            
            if let Some(null_idx) = tree_data[space_idx_abs + 1..].iter().position(|&b| b == 0) {
                let null_idx_abs = null_idx + space_idx_abs + 1;
                let filename_bytes = &tree_data[space_idx_abs + 1..null_idx_abs];
                let filename_str = std::str::from_utf8(filename_bytes)?;
                let filename_path = PathBuf::from(filename_str); // Store as PathBuf
                
                let sha1_start = null_idx_abs + 1;
                let sha1_end = sha1_start + 20;
                if sha1_end <= tree_data.len() {
                    let sha1_bytes = &tree_data[sha1_start..sha1_end];
                    let sha1_hex = hex::encode(sha1_bytes);
                    
                    files.insert(filename_path, sha1_hex);
                    cursor = sha1_end;
                } else {
                    // Malformed tree entry or end of data
                    anyhow::bail!("Malformed tree object: not enough data for SHA1 hash");
                }
            } else {
                // Malformed tree entry: no null terminator for filename
                anyhow::bail!("Malformed tree object: no null terminator for filename");
            }
        } else {
            // End of tree data or malformed entry
            break;
        }
    }
    
    Ok(files)
} 