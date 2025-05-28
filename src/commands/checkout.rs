use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
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
        println!("Switched to a new branch '{}'", branch_name);
    } else {
        // Check if the branch exists
        let branch_path = repo.git_dir.join("refs/heads").join(branch_name);
        if !branch_path.exists() {
            anyhow::bail!("Branch '{}' not found. If you want to create it, use -b option.", branch_name);
        }
        println!("Switched to branch '{}'", branch_name);
    }

    // Update HEAD to point to the new branch
    fs::write(
        repo.git_dir.join("HEAD"),
        format!("ref: refs/heads/{}\n", branch_name),
    )?;
    
    // Update working directory and index to match the target branch
    update_working_directory_and_index(&mut repo, branch_name)?;
    
    Ok(())
}

fn update_working_directory_and_index(repo: &mut Repository, branch_name: &str) -> Result<()> {
    // Get the commit ID for the target branch
    let branch_commit_id = match refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", branch_name)) {
        Ok(commit_id) => commit_id,
        Err(_) => {
            // Branch has no commits yet, clear everything
            clear_working_directory_and_index(repo)?;
            return Ok(());
        }
    };
    
    // Get the tree from the target commit
    let (commit_type, commit_data) = objects::read_object(&repo.git_dir.join("objects"), &branch_commit_id)?;
    if commit_type != "commit" {
        anyhow::bail!("Expected commit object, got {}", commit_type);
    }
    
    let commit_content = String::from_utf8_lossy(&commit_data);
    let lines: Vec<&str> = commit_content.lines().collect();
    if lines.is_empty() || !lines[0].starts_with("tree ") {
        anyhow::bail!("Invalid commit object format");
    }
    
    let tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
    
    // Get files from the target tree
    let target_files = get_tree_files(&repo.git_dir.join("objects"), tree_id)?;
    
    // Clear current working directory (except .git)
    clear_working_directory_except_git(repo)?;
    
    // Create new index and populate working directory
    repo.index = crate::repository::index::Index::new();
    
    for (file_path, object_id) in target_files {
        // Read the blob content
        let (obj_type, blob_data) = objects::read_object(&repo.git_dir.join("objects"), &object_id)?;
        if obj_type != "blob" {
            continue; // Skip non-blob objects
        }
        
        // Write file to working directory
        let full_path = repo.path.join(&file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &blob_data)?;
        
        // Add to index
        repo.index.add_file(&repo.path, &full_path, &object_id)?;
    }
    
    // Save the updated index
    repo.index.save(repo.git_dir.join("index"))?;
    
    Ok(())
}

fn clear_working_directory_and_index(repo: &mut Repository) -> Result<()> {
    clear_working_directory_except_git(repo)?;
    repo.index = crate::repository::index::Index::new();
    repo.index.save(repo.git_dir.join("index"))?;
    Ok(())
}

fn clear_working_directory_except_git(repo: &Repository) -> Result<()> {
    for entry in fs::read_dir(&repo.path)? {
        let entry = entry?;
        let path = entry.path();
        
        // Skip .git directory
        if path.file_name().unwrap() == ".git" {
            continue;
        }
        
        if path.is_file() {
            fs::remove_file(&path)?;
        } else if path.is_dir() {
            fs::remove_dir_all(&path)?;
        }
    }
    Ok(())
}

fn get_tree_files(objects_dir: &Path, tree_id: &str) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();
    
    let (tree_type, tree_data) = objects::read_object(objects_dir, tree_id)?;
    if tree_type != "tree" {
        anyhow::bail!("Expected tree object, got {}", tree_type);
    }
    
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
                    
                    files.insert(filename.to_string(), sha1_hex);
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
    
    Ok(files)
} 