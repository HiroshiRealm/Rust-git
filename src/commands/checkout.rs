use anyhow::Result;
use std::env;
use std::fs;
use crate::repository::{Repository, refs};

pub fn execute(branch_name: &str, create_branch_flag: bool) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    if create_branch_flag {
        // Check if branch already exists
        let g_branch_path = repo.git_dir.join("refs/heads").join(branch_name);
        if g_branch_path.exists() {
            anyhow::bail!("Branch '{}' already exists", branch_name);
        }

        let g_head_commit = refs::get_head_commit(&repo.git_dir)?;
        refs::create_branch(&repo.git_dir, branch_name, &g_head_commit)?;
        println!("Switched to a new branch '{}'", branch_name);
    } else {
        // Check if the branch exists
        let g_branch_path = repo.git_dir.join("refs/heads").join(branch_name);
        if !g_branch_path.exists() {
            anyhow::bail!("Branch '{}' not found. If you want to create it, use -b option.", branch_name);
        }
        println!("Switched to branch '{}'", branch_name);
    }

    // Update HEAD to point to the new branch
    fs::write(
        repo.git_dir.join("HEAD"),
        format!("ref: refs/heads/{}\n", branch_name),
    )?;
    
    Ok(())
} 