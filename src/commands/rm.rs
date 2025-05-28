use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use crate::repository::Repository;

pub fn execute(paths: &[String]) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let mut repo = Repository::open(&current_dir)?;
    
    let mut removed_files = Vec::new();
    
    // Remove each path
    for path_str in paths {
        let path = Path::new(path_str);
        
        // Check if file exists in working directory
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        };
        
        if !full_path.exists() {
            println!("pathspec '{}' did not match any files (file does not exist)", path_str);
            continue;
        }
        
        // Try to remove from index
        let removed = repo.index.remove_path(&repo.path, path)?;
        
        if removed.is_empty() {
            println!("pathspec '{}' did not match any files in the index", path_str);
            continue;
        }
        
        // Remove from working directory
        if full_path.is_file() {
            fs::remove_file(&full_path)?;
            println!("rm '{}'", path_str);
        } else if full_path.is_dir() {
            fs::remove_dir_all(&full_path)?;
            println!("rm -r '{}'", path_str);
        }
        
        removed_files.extend(removed);
    }
    
    // Save the index
    repo.index.save(repo.git_dir.join("index"))?;
    
    if !removed_files.is_empty() {
        println!("Removed {} file(s) from the index and working directory", removed_files.len());
    }
    
    Ok(())
} 