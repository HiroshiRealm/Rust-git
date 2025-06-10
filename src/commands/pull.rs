use anyhow::Result;
use std::env;
use crate::repository::Repository;
use super::{fetch, merge};

pub fn execute(remote_or_url: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;
    
    // The `pull` command is a combination of `fetch` followed by `merge`.
    // We can reuse the fetch logic entirely. The `fetch` command will
    // handle resolving the name/URL and printing appropriate messages.
    
    // 1. Fetch from the remote or URL
    fetch::execute(remote_or_url)?;
    
    // 2. Merge the fetched branch
    // We need to determine what branch to merge. By convention, it's `remote_name/current_branch`.
    // If a URL was passed, the `fetch` command uses the URL as the remote name, which is
    // not ideal for merging. A more robust solution would be needed for complex cases,
    // but for the common case (pulling into the current branch from a remote of the same name),
    // this works. The remote name for merging is simply the argument we were passed.
    println!("Merging...");
    let current_branch = repo.current_branch()?;
    let remote_branch_to_merge = format!("{}/{}", remote_or_url, current_branch);
    
    merge::execute(&remote_branch_to_merge)?;
    
    println!("Successfully pulled and merged from remote '{}'.", remote_or_url);
    
    Ok(())
} 