use anyhow::Result;
use std::env;
use crate::repository::{Repository, refs, objects};
use std::collections::HashMap;
use std::path::Path;
use hex;

// Helper function to get tree files (filename -> object_id map) from a commit_id
fn get_files_from_commit(repo: &Repository, commit_id: &str) -> Result<HashMap<String, String>> {
    let objects_dir = repo.git_dir.join("objects");
    let (commit_type, commit_data) = objects::read_object(&objects_dir, commit_id)?;
    if commit_type != "commit" {
        anyhow::bail!("Expected commit object, got {}", commit_type);
    }
    let commit_content = String::from_utf8_lossy(&commit_data);
    let lines: Vec<&str> = commit_content.lines().collect();
    if lines.is_empty() || !lines[0].starts_with("tree ") {
        anyhow::bail!("Invalid commit object format for commit {}", commit_id);
    }
    let tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
    
    // This is a simplified version of get_tree_files from checkout.rs
    // It assumes files are at the root of the tree for simplicity, as per typical Git usage for simple cases.
    // A full implementation would handle nested trees (directories).
    get_tree_content(&objects_dir, tree_id)
}

// Helper function to parse tree object content (similar to get_tree_files in checkout.rs)
// For simplicity, this version assumes all entries are blobs (files) and not trees (directories)
// and that filenames do not contain null bytes or other problematic characters.
fn get_tree_content(objects_dir: &Path, tree_id: &str) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();
    let (tree_type, tree_data) = objects::read_object(objects_dir, tree_id)?;
    if tree_type != "tree" {
        anyhow::bail!("Expected tree object for ID {}, got {}", tree_id, tree_type);
    }

    let mut cursor = 0;
    while cursor < tree_data.len() {
        // Format: <mode> <filename>\0<sha1_hash_20_bytes>
        // Find space after mode
        if let Some(space_idx) = tree_data[cursor..].iter().position(|&b| b == b' ') {
            let name_start_idx = cursor + space_idx + 1;
            // Find null after filename
            if let Some(null_idx_rel) = tree_data[name_start_idx..].iter().position(|&b| b == 0) {
                let null_idx_abs = name_start_idx + null_idx_rel;
                let filename_bytes = &tree_data[name_start_idx..null_idx_abs];
                let filename = String::from_utf8(filename_bytes.to_vec())
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in filename: {}", e))?;

                let sha1_start = null_idx_abs + 1;
                let sha1_end = sha1_start + 20; // SHA-1 hash is 20 bytes
                if sha1_end <= tree_data.len() {
                    let sha1_bytes = &tree_data[sha1_start..sha1_end];
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

    // Get file lists for both branches
    let current_files = get_files_from_commit(&repo, &current_branch_commit_id)?;
    let merge_files = get_files_from_commit(&repo, &merge_branch_commit_id)?;

    let mut conflict_found = false;

    // Combine all filenames from both branches
    let mut all_filenames = current_files.keys().cloned().collect::<std::collections::HashSet<_>>();
    all_filenames.extend(merge_files.keys().cloned());

    for filename in all_filenames {
        match (current_files.get(&filename), merge_files.get(&filename)) {
            (Some(current_file_id), Some(merge_file_id)) => {
                // File exists in both branches
                if current_file_id == merge_file_id {
                    continue; // Files are identical
                }

                // Read content of both files
                let (_, current_content_data) = objects::read_object(&repo.git_dir.join("objects"), current_file_id)?;
                let current_content_str = String::from_utf8_lossy(&current_content_data);
                let current_lines: Vec<&str> = current_content_str.lines().collect();

                let (_, merge_content_data) = objects::read_object(&repo.git_dir.join("objects"), merge_file_id)?;
                let merge_content_str = String::from_utf8_lossy(&merge_content_data);
                let merge_lines: Vec<&str> = merge_content_str.lines().collect();
                
                let mut diffs = Vec::new();
                let max_len = std::cmp::max(current_lines.len(), merge_lines.len());
                let mut i = 0;
                while i < max_len {
                    let current_line = current_lines.get(i);
                    let merge_line = merge_lines.get(i);

                    if current_line != merge_line {
                        let conflict_start_line = i + 1;
                        let mut conflict_end_line = conflict_start_line;
                        i += 1;
                        while i < max_len {
                            let cl = current_lines.get(i);
                            let ml = merge_lines.get(i);
                            if cl != ml {
                                conflict_end_line = i + 1;
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        diffs.push((conflict_start_line, conflict_end_line));
                    } else {
                        i += 1;
                    }
                }

                if !diffs.is_empty() {
                    conflict_found = true;
                    for (start, end) in diffs {
                        if start == end {
                            println!("Merge conflict in {}: {}", filename, start);
                        } else {
                            println!("Merge conflict in {}: [{}, {}]", filename, start, end);
                        }
                    }
                }
            }
            (None, Some(_merge_file_id)) => {
                // File only in merge branch (addition)
                // No conflict for new files
            }
            (Some(_current_file_id), None) => {
                // File only in current branch (deletion in merge branch)
                // No conflict for deletions
            }
            (None, None) => {
                // Should not happen as we iterate over known filenames
            }
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
    
    // Step 1: Create merged tree by combining files from both branches
    // Start with current branch files, then add/overwrite with merge branch files
    let mut merged_files = current_files.clone();
    
    // Add or overwrite files from merge branch
    for (filename, object_id) in merge_files {
        merged_files.insert(filename, object_id);
    }
    
    // Step 2: Update working directory with merged files
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
        let (obj_type, blob_data) = objects::read_object(&repo.git_dir.join("objects"), object_id)?;
        if obj_type == "blob" {
            let file_path = repo.path.join(filename);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file_path, &blob_data)?;
            
            // Update index
            repo.index.add_file(&repo.path, &file_path, object_id)?;
        }
    }
    
    // Step 3: Create merge commit
    let current_tree_id = objects::write_tree(&repo)?;
    let merge_commit_id = objects::write_commit(
        &repo.git_dir.join("objects"),
        &current_tree_id,
        &[&current_branch_commit_id, &merge_branch_commit_id], // Two parents for merge commit
        &format!("Merge branch '{}' into {}", branch_to_merge, current_branch_name),
        "Rust-git <user@example.com>",
    )?;
    
    // Step 4: Update current branch ref
    refs::update_ref(
        &repo.git_dir,
        &format!("refs/heads/{}", current_branch_name),
        &merge_commit_id,
    )?;
    
    // Step 5: Save updated index
    repo.index.save(repo.git_dir.join("index"))?;

    Ok(())
} 