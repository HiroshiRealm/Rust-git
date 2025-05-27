use anyhow::Result;
use std::env;
use crate::repository::{Repository, refs, objects};

pub fn execute(branch: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    // Get the current branch
    let current_branch = repo.current_branch()?;
    
    // Check if the branch to merge exists
    let branch_path = repo.git_dir.join("refs/heads").join(branch);
    if !branch_path.exists() {
        anyhow::bail!("Branch '{}' not found", branch);
    }
    
    // Get commit IDs
    let current_commit = refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", current_branch))?;
    let merge_commit = refs::read_ref(&repo.git_dir, &format!("refs/heads/{}", branch))?;
    
    if current_commit == merge_commit {
        println!("Already up to date.");
        return Ok(());
    }
    
    // In a real implementation, we would perform a three-way merge
    // For simplicity, we'll just create a merge commit with two parents
    
    // Create tree from current index
    let tree_id = objects::write_tree(&repo)?;
    
    // Create merge commit
    let author = "In Rust We Git <user@example.com>";
    let message = format!("Merge branch '{}'", branch);
    let commit_id = objects::write_commit(
        &repo.git_dir.join("objects"),
        &tree_id,
        &[&current_commit, &merge_commit],
        &message,
        author,
    )?;
    
    // Update branch reference
    refs::update_ref(
        &repo.git_dir,
        &format!("refs/heads/{}", current_branch),
        &commit_id,
    )?;
    
    println!("Merged branch '{}' into {}", branch, current_branch);
    
    Ok(())
} 