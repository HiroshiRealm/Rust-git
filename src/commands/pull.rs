use anyhow::Result;
use std::env;
use crate::repository::Repository;
use super::{fetch, merge};

pub fn execute(remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;
    
    // We need the URL to print it, so we get it from the config here.
    // The `fetch` command will look it up again, which is slightly inefficient
    // but keeps the command logic separate and clean.
    let remote_url = repo.config.get_remote_url(remote_name)
        .ok_or_else(|| anyhow::anyhow!("Remote '{}' not found in config", remote_name))?;

    println!("Pulling from remote '{}' at '{}'", remote_name, remote_url);
    
    // 1. Fetch from the remote
    println!("Fetching...");
    fetch::execute(remote_name)?;
    
    // 2. Merge the fetched branch
    println!("Merging...");
    let current_branch = repo.current_branch()?;
    let remote_branch_to_merge = format!("{}/{}", remote_name, current_branch);
    
    merge::execute(&remote_branch_to_merge)?;
    
    println!("Successfully pulled and merged from remote '{}'.", remote_name);
    
    Ok(())
} 