use anyhow::Result;
use std::env;
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
        
        let removed = repo.index.remove_path(&repo.path, path)?;
        
        if removed.is_empty() {
            println!("pathspec '{}' did not match any files", path_str);
        } else {
            removed_files.extend(removed);
        }
    }
    
    // Save the index
    repo.index.save(repo.git_dir.join("index"))?;
    
    if !removed_files.is_empty() {
        println!("Removed {} file(s) from the index", removed_files.len());
    }
    
    Ok(())
} 