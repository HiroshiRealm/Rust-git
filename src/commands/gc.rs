use anyhow::Result;
use clap::Args;
use std::env;
use crate::repository::Repository;

/// Garbage collect unnecessary files and optimize the local repository
#[derive(Args)]
#[command(name = "gc")]
pub struct Command;

impl Command {
    pub fn run(&self, repo: &Repository) -> Result<()> {
        repo.gc()
    }
}
 
pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;
    Command{}.run(&repo)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use crate::repository::{Repository, objects};
    use anyhow::Result;

    #[test]
    fn test_gc_removes_unreachable_loose_objects_and_packs_reachable() -> Result<()> {
        let temp_dir = tempdir()?;
        let repo = Repository::init(&temp_dir)?;
        let objects_dir = repo.git_dir.join("objects");

        // Create a reachable object (added to repository)
        let reachable_id = objects::write_blob(&objects_dir, b"reachable")?;
        // Create an unreachable object (not referenced)
        let unreachable_id = objects::write_blob(&objects_dir, b"unreachable")?;

        // Ensure both exist as loose objects
        let reachable_path = objects_dir.join(&reachable_id[0..2]).join(&reachable_id[2..]);
        let unreachable_path = objects_dir.join(&unreachable_id[0..2]).join(&unreachable_id[2..]);
        assert!(reachable_path.exists());
        assert!(unreachable_path.exists());

        // Run gc
        let cmd = Command;
        cmd.run(&repo)?;

        // Check that a pack file exists
        let pack_dir = repo.git_dir.join("objects").join("pack");
        assert!(pack_dir.exists());
        let entries: Vec<_> = fs::read_dir(&pack_dir)?
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "Expected pack files to be created");

        // Loose objects should be removed
        assert!(!reachable_path.exists(), "Expected reachable loose object to be moved to pack");
        assert!(!unreachable_path.exists(), "Expected unreachable loose object to be deleted");

        Ok(())
    }
}