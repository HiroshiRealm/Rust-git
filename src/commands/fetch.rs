use anyhow::Result;
use std::env;
use std::fs::File;

use crate::repository::{bundle, Repository};

pub fn execute(remote_path: &str, remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    println!("Fetching from remote '{}' at '{}'", remote_name, remote_path);

    // 1. Open the bundle file from the provided path.
    let bundle_file = File::open(remote_path)
        .map_err(|e| anyhow::anyhow!("Failed to open remote bundle at '{}': {}", remote_path, e))?;

    // 2. Call the unbundle function to extract objects and update refs.
    bundle::unbundle(&repo, bundle_file, Some(remote_name))?;
    
    println!("Successfully fetched from remote '{}'.", remote_name);
    
    Ok(())
} 