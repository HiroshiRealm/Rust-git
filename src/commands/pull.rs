use anyhow::Result;
use std::env;
use crate::repository::Repository;
use super::{fetch, merge};

pub fn execute(remote_path: &str, remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    println!("Pulling from remote '{}' at '{}'", remote_name, remote_path);
    
    // 1. Fetch from the remote
    println!("Fetching...");
    fetch::execute(remote_path, remote_name)?;
    
    // 2. Merge the fetched branch
    println!("Merging...");
    let current_branch = repo.current_branch()?;
    let remote_branch_to_merge = format!("{}/{}", remote_name, current_branch);
    
    merge::execute(&remote_branch_to_merge)?;
    
    println!("Successfully pulled and merged from remote '{}'.", remote_name);
    
    Ok(())
} 