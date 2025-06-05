use anyhow::Result;
use std::env;
use crate::repository::Repository;

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    // Create pack file
    crate::repository::pack::create_pack(&repo.git_dir.join("objects"))?;

    // Remove loose objects that are now in the pack file
    let objects_dir = repo.git_dir.join("objects");
    for entry in std::fs::read_dir(&objects_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let dir_name = entry.file_name();
            if dir_name.len() == 2 {
                for file_entry in std::fs::read_dir(entry.path())? {
                    let file_entry = file_entry?;
                    if file_entry.file_type()?.is_file() {
                        let file_name = file_entry.file_name();
                        let object_id = format!("{}{}", dir_name.to_string_lossy(), file_name.to_string_lossy());
                        
                        // Check if object exists in pack file
                        if crate::repository::pack::read_pack_object(&objects_dir, &object_id).is_ok() {
                            // Object exists in pack file, remove loose object
                            std::fs::remove_file(file_entry.path())?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
} 