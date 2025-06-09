use anyhow::Result;
use std::env;
use crate::repository::{bundle, Repository};

pub fn execute(remote_url: &str, remote_name: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    println!("Pushing to remote '{}' at '{}'", remote_name, remote_url);

    // 1. Create the bundle in an in-memory buffer.
    let mut buffer: Vec<u8> = Vec::new();
    bundle::create_bundle(&repo, &mut buffer)?;
    
    // 2. Make an HTTP POST request with the bundle as the body.
    let client = reqwest::blocking::Client::new();
    let response = client.post(remote_url)
        .header("Content-Type", "application/octet-stream")
        .body(buffer)
        .send()
        .map_err(|e| anyhow::anyhow!("Failed to connect to remote url '{}': {}", remote_url, e))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to push to remote. Server responded with status {}: {}",
            response.status(),
            response.text().unwrap_or_else(|_| "No body".into())
        );
    }
    
    let current_branch = repo.current_branch()?;
    
    println!("Successfully pushed branch '{}' to remote '{}'.", current_branch, remote_name);
    
    Ok(())
} 