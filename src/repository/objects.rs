use anyhow::{Context, Result};
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;
use sha1::{Sha1, Digest};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::str;
use chrono::Utc;
use hex;
use super::Repository;

// Hash an object and return its ID
pub fn hash_object(data: &[u8], object_type: &str) -> String {
    let header = format!("{} {}", object_type, data.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(b"\0");
    hasher.update(data);
    
    let result = hasher.finalize();
    hex::encode(result)
}

// Write a blob object to the object store
pub fn write_blob<P: AsRef<Path>>(objects_dir: P, data: &[u8]) -> Result<String> {
    write_object(objects_dir, data, "blob")
}

// Write an object to the object store
pub fn write_object<P: AsRef<Path>>(objects_dir: P, data: &[u8], object_type: &str) -> Result<String> {
    let object_id = hash_object(data, object_type);
    let dir_name = &object_id[0..2];
    let file_name = &object_id[2..];
    
    let dir_path = objects_dir.as_ref().join(dir_name);
    fs::create_dir_all(&dir_path)?;
    
    let object_path = dir_path.join(file_name);
    if !object_path.exists() {
        let header = format!("{} {}", object_type, data.len());
        let mut content = Vec::new();
        content.extend_from_slice(header.as_bytes());
        content.push(0);
        content.extend_from_slice(data);
        
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&content)?;
        let compressed = encoder.finish()?;
        
        fs::write(object_path, compressed)?;
    }
    
    Ok(object_id)
}

/// Read a raw git object (header + data) from the object store.
fn read_raw_git_object<P: AsRef<Path>>(objects_dir: P, object_id: &str) -> Result<Vec<u8>> {
    let dir_name = &object_id[0..2];
    let file_name = &object_id[2..];
    
    let object_path = objects_dir.as_ref().join(dir_name).join(file_name);
    let compressed = fs::read(object_path)?;
    
    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

// Read an object from the object store and parse its header
pub fn read_object<P: AsRef<Path>>(objects_dir: P, object_id: &str) -> Result<(String, Vec<u8>)> {
    let decompressed = read_raw_git_object(objects_dir, object_id)?;
    
    // Parse header
    let null_pos = decompressed
        .iter()
        .position(|&b| b == 0)
        .context("Invalid git object: no null byte")?;
    
    let header = str::from_utf8(&decompressed[0..null_pos])?;
    let parts: Vec<&str> = header.split(' ').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid git object header: '{}'", header);
    }
    
    let object_type = parts[0].to_string();
    let size: usize = parts[1].parse().context("Invalid object size in header")?;
    let data = decompressed[null_pos + 1..].to_vec();

    if data.len() != size {
        anyhow::bail!(
            "Object size mismatch for OID {}: header says {} but data is {} bytes",
            object_id,
            size,
            data.len()
        );
    }
    
    Ok((object_type, data))
}

/// Read only the raw data of an object, without the git header.
/// This is useful for operations like diffing.
pub fn read_raw_object<P: AsRef<Path>>(objects_dir: P, object_id: &str) -> Result<(String, Vec<u8>)> {
    // For packing, we need to know the type to group similar objects, and the raw data for diffing.
    // The existing read_object function already separates type and data, which is what we need.
    // So this function can just be an alias that properly calls the parsing function.
    read_object(objects_dir.as_ref(), object_id)
}

// Create a tree object from the index
pub fn write_tree(repo: &super::Repository) -> Result<String> {
    let mut tree_entries = Vec::new();
    
    for (path, entry) in repo.index.get_entries() {
        // In a real implementation, we would handle subdirectories by creating subtrees
        // For simplicity, we'll just create a flat tree
        
        let mode_str = format!("{:o}", entry.mode);
        let path_str = path.to_string_lossy();
        
        // Convert hex object_id to binary
        let object_id_bytes = hex::decode(&entry.object_id)?;
        if object_id_bytes.len() != 20 {
            anyhow::bail!("Invalid SHA-1 hash length: expected 20 bytes, got {}", object_id_bytes.len());
        }
        
        // Create tree entry: mode + space + filename + null + 20-byte sha1
        let mut entry_data = Vec::new();
        entry_data.extend_from_slice(mode_str.as_bytes());
        entry_data.push(b' ');
        entry_data.extend_from_slice(path_str.as_bytes());
        entry_data.push(0);
        entry_data.extend_from_slice(&object_id_bytes);
        
        tree_entries.push(entry_data);
    }
    
    // Sort by filename (Git requirement)
    tree_entries.sort_by(|a, b| {
        // Find the filename part (after mode and space, before null byte)
        let find_filename_string = |entry: &[u8]| -> String {
            if let Some(space_pos) = entry.iter().position(|&b| b == b' ') {
                if let Some(null_pos) = entry[space_pos + 1..].iter().position(|&b| b == 0) {
                    let filename_bytes = &entry[space_pos + 1..space_pos + 1 + null_pos];
                    return String::from_utf8_lossy(filename_bytes).to_string();
                }
            }
            String::new()
        };
        
        let filename_a = find_filename_string(a);
        let filename_b = find_filename_string(b);
        filename_a.cmp(&filename_b)
    });
    
    let mut tree_content = Vec::new();
    for entry in tree_entries {
        tree_content.extend_from_slice(&entry);
    }
    
    write_object(&repo.git_dir.join("objects"), &tree_content, "tree")
}

// Create a commit object
pub fn write_commit<P: AsRef<Path>>(
    objects_dir: P,
    tree_id: &str,
    parent_ids: &[&str],
    message: &str,
    author: &str,
) -> Result<String> {
    let timestamp = Utc::now().format("%s %z").to_string();
    
    let mut commit_content = format!("tree {}\n", tree_id);
    
    for parent_id in parent_ids {
        commit_content.push_str(&format!("parent {}\n", parent_id));
    }
    
    commit_content.push_str(&format!("author {} {}\n", author, timestamp));
    commit_content.push_str(&format!("committer {} {}\n", author, timestamp));
    commit_content.push_str("\n");
    commit_content.push_str(message);
    commit_content.push_str("\n");
    
    write_object(objects_dir, commit_content.as_bytes(), "commit")
}

/// Check if `potential_ancestor_id` is an ancestor of `commit_id`.
pub fn is_ancestor(repo: &Repository, potential_ancestor_id: &str, commit_id: &str) -> Result<bool> {
    if potential_ancestor_id == commit_id {
        return Ok(true);
    }
    
    let mut queue = vec![commit_id.to_string()];
    let mut visited = std::collections::HashSet::new();

    while let Some(current_commit_id) = queue.pop() {
        if !visited.insert(current_commit_id.clone()) {
            continue; // Already visited this commit
        }

        if current_commit_id == potential_ancestor_id {
            return Ok(true);
        }

        // Get parents of the current commit and add them to the queue
        if let Ok((commit_type, commit_data)) = read_object(&repo.git_dir.join("objects"), &current_commit_id) {
            if commit_type == "commit" {
                let commit_content = String::from_utf8_lossy(&commit_data);
                for line in commit_content.lines() {
                    if line.starts_with("parent ") {
                        if let Some(parent_id) = line.strip_prefix("parent ") {
                            queue.push(parent_id.trim().to_string());
                        }
                    }
                }
            }
        } else {
            // Could not read object, might be a shallow clone or corrupted history.
            // For this check, we assume it means the ancestor is not found down this path.
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_hash_object() {
        let data = b"test content";
        let hash = hash_object(data, "blob");
        
        // The hash should be a 40-character hex string
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        
        // Same content should produce the same hash
        let hash2 = hash_object(data, "blob");
        assert_eq!(hash, hash2);
        
        // Different content should produce different hashes
        let hash3 = hash_object(b"different content", "blob");
        assert_ne!(hash, hash3);
        
        // Different types should produce different hashes
        let hash4 = hash_object(data, "commit");
        assert_ne!(hash, hash4);
    }
    
    #[test]
    fn test_write_and_read_blob() -> Result<()> {
        let temp_dir = tempdir()?;
        let objects_dir = temp_dir.path().join("objects");
        fs::create_dir_all(&objects_dir)?;
        
        let data = b"test content";
        let object_id = write_blob(&objects_dir, data)?;
        
        // Read the object back
        let (object_type, content) = read_object(&objects_dir, &object_id)?;
        
        // Check that the content and type are correct
        assert_eq!(object_type, "blob");
        assert_eq!(content, data);
        
        Ok(())
    }
    
    #[test]
    fn test_write_commit() -> Result<()> {
        let temp_dir = tempdir()?;
        let objects_dir = temp_dir.path().join("objects");
        fs::create_dir_all(&objects_dir)?;
        
        let tree_id = "1234567890123456789012345678901234567890";
        let parent_id = "abcdef0123456789abcdef0123456789abcdef01";
        let message = "Test commit message";
        let author = "Test User <test@example.com>";
        
        let commit_id = write_commit(
            &objects_dir,
            tree_id,
            &[parent_id],
            message,
            author,
        )?;
        
        // Read the commit back
        let (object_type, content) = read_object(&objects_dir, &commit_id)?;
        
        // Check that the content and type are correct
        assert_eq!(object_type, "commit");
        let content_str = str::from_utf8(&content)?;
        
        // Check that the commit contains the expected data
        assert!(content_str.contains(&format!("tree {}", tree_id)));
        assert!(content_str.contains(&format!("parent {}", parent_id)));
        assert!(content_str.contains(message));
        assert!(content_str.contains(author));
        
        Ok(())
    }
} 