use anyhow::Result;
use std::env;
use crate::repository::Repository;

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    // Create new pack file
    crate::repository::pack::create_pack(&repo.git_dir.join("objects"))?;

    // Remove old pack files
    let pack_dir = repo.git_dir.join("objects").join("pack");
    if pack_dir.exists() {
        for entry in std::fs::read_dir(&pack_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("pack") {
                    // Keep the newest pack file
                    if path.file_name().and_then(|s| s.to_str()) != Some("pack-1.pack") {
                        std::fs::remove_file(&path)?;
                        // Also remove corresponding index file
                        let idx_path = path.with_extension("idx");
                        let _ = std::fs::remove_file(idx_path);
                    }
                }
            }
        }
    }

    Ok(())
} 