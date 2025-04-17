use anyhow::Result;
use std::env;
use crate::repository::{Repository, objects, refs};

pub fn execute(message: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    if repo.index.is_empty() {
        println!("Nothing to commit, working tree clean");
        return Ok(());
    }
    
    // Write the tree
    let tree_id = objects::write_tree(&repo)?;
    
    // Get the current branch and parent commit
    let branch = repo.current_branch()?;
    let parent_commits = match refs::get_head_commit(&repo.git_dir) {
        Ok(commit) => vec![commit],
        Err(_) => Vec::new(),
    };
    
    // Create the commit
    let author = "In Rust We Git <user@example.com>";
    let parent_refs: Vec<&str> = parent_commits.iter().map(|s| s.as_str()).collect();
    
    let commit_id = objects::write_commit(
        &repo.git_dir.join("objects"),
        &tree_id,
        &parent_refs,
        message,
        author,
    )?;
    
    // Update the branch reference
    refs::update_ref(
        &repo.git_dir,
        &format!("refs/heads/{}", branch),
        &commit_id,
    )?;
    
    println!("[{}] {}", branch, message);
    
    Ok(())
} 