use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use crate::repository::Repository;

pub fn execute(paths: &[String]) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let mut repo = Repository::open(&current_dir)?;
    
    let mut added_files = Vec::new();
    
    // Add each path
    for path_str in paths {
        let path = Path::new(path_str);
        
        if !path.exists() {
            println!("pathspec '{}' did not match any files", path_str);
            continue;
        }
        
        if path.is_dir() {
            let files = repo.index.add_directory(
                &repo.path,
                path,
                &repo.git_dir.join("objects"),
            )?;
            added_files.extend(files);
        } else {
            let content = fs::read(path)?;
            let object_id = crate::repository::objects::write_blob(
                &repo.git_dir.join("objects"),
                &content,
            )?;
            
            repo.index.add_file(&repo.path, path, &object_id)?;
            
            let relative_path = path.strip_prefix(&repo.path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            added_files.push(relative_path);
        }
    }
    
    // Save the index
    repo.index.save(repo.git_dir.join("index"))?;
    
    if !added_files.is_empty() {
        println!("Added {} file(s) to the index", added_files.len());
    }
    
    Ok(())
} 