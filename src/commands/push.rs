use anyhow::Result;
use std::env;
use std::fs::File;

use crate::repository::{bundle, Repository};

pub fn execute(remote_path: &str, remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    println!("Pushing to remote '{}' at '{}'", remote_name, remote_path);

    // 1. Create the bundle file at the destination path.
    let bundle_file = File::create(remote_path)
        .map_err(|e| anyhow::anyhow!("Failed to create remote bundle at '{}': {}", remote_path, e))?;

    // 2. Call the create_bundle function to generate the bundle from the local repo.
    bundle::create_bundle(&repo, bundle_file)?;
    
    let current_branch = repo.current_branch()?;
    
    println!("Successfully pushed branch '{}' to remote '{}'.", current_branch, remote_name);
    
    Ok(())
} 