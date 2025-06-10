use anyhow::{anyhow, Result};
use std::env;
use crate::repository::{bundle, Repository};

pub fn execute(remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    // 1. Get the URL for the remote name from the config.
    let remote_url = repo.config.get_remote_url(remote_name)
        .ok_or_else(|| anyhow!("Remote '{}' not found in config", remote_name))?;

    println!("Fetching from remote '{}' at '{}'", remote_name, remote_url);

    // 2. Make an HTTP GET request to the remote URL.
    let response = reqwest::blocking::get(remote_url)
        .map_err(|e| anyhow!("Failed to connect to remote url '{}': {}", remote_url, e))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch from remote. Server responded with status {}: {}",
            response.status(),
            response.text().unwrap_or_else(|_| "No body".into())
        );
    }

    // 3. The response body is the bundle. Call the unbundle function to process it.
    bundle::unbundle(&repo, response, Some(remote_name))?;
    
    println!("Successfully fetched from remote '{}'.", remote_name);
    
    Ok(())
} 