use anyhow::{anyhow, Result};
use std::env;
use crate::repository::{bundle, Repository};

pub fn execute(remote_arg: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    // 1. Determine the URL. The argument could be a remote name or a direct URL.
    let (remote_name, remote_url) = 
        if let Some(url_from_config) = repo.config.get_remote_url(remote_arg) {
            // The argument is a configured remote name.
            (remote_arg.to_string(), url_from_config.clone())
        } else if remote_arg.starts_with("http://") || remote_arg.starts_with("https://") {
            // The argument looks like a URL. Use it directly.
            // We'll use the URL itself as the "name" for display purposes.
            (remote_arg.to_string(), remote_arg.to_string())
        } else {
            // The argument is neither a configured remote nor a valid URL.
            anyhow::bail!(
                "'{}' does not appear to be a git repository. \
                Please use a valid URL or a remote name defined with 'remote add'.",
                remote_arg
            );
        };

    println!("Pushing to remote '{}' at '{}'", remote_name, remote_url);

    // 2. Create the bundle in an in-memory buffer.
    let mut buffer: Vec<u8> = Vec::new();
    bundle::create_bundle(&repo, &mut buffer)?;
    
    // 3. Make an HTTP POST request with the bundle as the body.
    let client = reqwest::blocking::Client::new();
    let response = client.post(&remote_url)
        .header("Content-Type", "application/octet-stream")
        .body(buffer)
        .send()
        .map_err(|e| anyhow!("Failed to connect to remote url '{}': {}", remote_url, e))?;

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