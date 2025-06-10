use anyhow::{anyhow, Result};
use std::env;
use crate::repository::{bundle, Repository};

// A helper function to resolve a remote name or a raw URL into a URL.
// Returns a tuple of (resolved_url, remote_name_or_url).
// The second element is used for creating the remote branch ref, e.g., "origin/master".
fn resolve_url(repo: &Repository, remote_or_url: &str) -> Result<(String, String)> {
    if remote_or_url.starts_with("http://") || remote_or_url.starts_with("https://") {
        // It's a URL, so use it directly.
        // We'll use the URL itself as the "name" for the purpose of creating refs.
        // A more advanced implementation might try to derive a name, but this is simple and works.
        Ok((remote_or_url.to_string(), remote_or_url.to_string()))
    } else {
        // It's a remote name, look it up in the config.
        let url = repo.config.get_remote_url(remote_or_url)
            .ok_or_else(|| anyhow!("Remote '{}' not found in config", remote_or_url))?;
        Ok((url.to_string(), remote_or_url.to_string()))
    }
}

pub fn execute(remote_or_url: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    // 1. Resolve the remote name or URL.
    let (remote_url, remote_name) = resolve_url(&repo, remote_or_url)?;

    println!("Fetching from remote '{}' at '{}'", remote_name, remote_url);

    // 2. Make an HTTP GET request to the remote URL.
    let response = reqwest::blocking::get(&remote_url)
        .map_err(|e| anyhow!("Failed to connect to remote url '{}': {}", remote_url, e))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to fetch from remote. Server responded with status {}: {}",
            response.status(),
            response.text().unwrap_or_else(|_| "No body".into())
        );
    }

    // 3. The response body is the bundle. Call the unbundle function to process it.
    bundle::unbundle(&repo, response, Some(&remote_name))?;
    
    println!("Successfully fetched from remote '{}'.", remote_name);
    
    Ok(())
} 