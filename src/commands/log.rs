use anyhow::Result;
use std::env;
use crate::repository::{Repository, refs, objects};

pub fn execute(_args: &[String]) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    let head_commit = refs::get_head_commit(&repo.git_dir)?;
    let mut current_commit = head_commit;

    while !current_commit.is_empty() {
        let commit_obj = objects::read_object(&repo, &current_commit)?;
        if commit_obj.object_type != "commit" {
            break;
        }

        let commit_content = String::from_utf8_lossy(&commit_obj.data);
        println!("commit {}", current_commit);
        println!("{}", commit_content);
        println!();

        // Get parent commit
        let lines: Vec<&str> = commit_content.lines().collect();
        current_commit = lines.iter()
            .find(|line| line.starts_with("parent "))
            .map(|line| line[7..].to_string())
            .unwrap_or_default();
    }

    Ok(())
} 