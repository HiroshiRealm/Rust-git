use anyhow::Result;
use clap::Args;
use std::env;
use crate::repository::Repository;

/// Pack all loose objects into a pack file
#[derive(Args)]
#[command(name = "repack")]
pub struct Command;

impl Command {
    pub fn run(&self, repo: &Repository) -> Result<()> {
        repo.repack()
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
    use crate::repository::{objects, Repository};
    use std::fs;

    #[test]
    fn test_repack_creates_pack_and_deletes_loose() -> Result<()> {
        let temp_dir = tempdir()?;
        let repo = Repository::init(&temp_dir)?;
        let objects_dir = repo.git_dir.join("objects");
        fs::create_dir_all(&objects_dir)?;
        // Create loose objects
        let id1 = objects::write_blob(&objects_dir, b"1")?;
        let id2 = objects::write_blob(&objects_dir, b"2")?;
        let path1 = objects_dir.join(&id1[0..2]).join(&id1[2..]);
        let path2 = objects_dir.join(&id2[0..2]).join(&id2[2..]);
        assert!(path1.exists());
        assert!(path2.exists());
        let cmd = Command;
        cmd.run(&repo)?;
        let pack_dir = objects_dir.join("pack");
        let entries: Vec<_> = fs::read_dir(&pack_dir)?.filter_map(|e| e.ok()).collect();
        assert!(entries.iter().any(|e| e.path().extension() == Some("pack".as_ref())));
        assert!(entries.iter().any(|e| e.path().extension() == Some("idx".as_ref())));
        assert!(!path1.exists());
        assert!(!path2.exists());
        Ok(())
    }
}