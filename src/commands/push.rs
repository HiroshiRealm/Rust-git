use anyhow::{Result, Context};
use std::env;
use std::fs;
use std::path::Path;
use std::io::{self, Write};
use crate::repository::Repository;
use tar::Builder;
use std::fs::File;

pub fn execute(remote: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Pushing to remote '{}'", remote);
    
    let current_branch = repo.current_branch()?;
    
    // Create a temporary tar file with our .git directory
    let temp_dir = std::env::temp_dir().join("rust_git_push");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    
    let tar_path = temp_dir.join("git-repo.tar.gz");
    
    // Create tar archive of .git directory
    create_git_archive(&repo.git_dir, &tar_path)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Created archive: {}", tar_path.display());
    
    // Upload the archive
    if let Err(e) = upload_to_remote(remote, &tar_path) {
        #[cfg(not(feature = "online_judge"))]
        eprintln!("Failed to upload to remote: {}", e);
        
        // Fallback: try to copy to local filesystem path
        if let Err(e2) = copy_to_local_remote(remote, &repo.git_dir) {
            anyhow::bail!("Failed to upload to remote: {} and failed local fallback: {}", e, e2);
        }
    }
    
    #[cfg(not(feature = "online_judge"))]
    println!("Successfully pushed branch '{}' to remote '{}'", current_branch, remote);
    
    // Clean up temp directory
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    
    Ok(())
}

fn create_git_archive(git_dir: &Path, output_path: &Path) -> Result<()> {
    let tar_file = File::create(output_path)?;
    let enc = flate2::write::GzEncoder::new(tar_file, flate2::Compression::default());
    let mut tar = Builder::new(enc);
    
    // Add the entire .git directory to the archive
    tar.append_dir_all(".git", git_dir)?;
    tar.finish()?;
    
    Ok(())
}

fn upload_to_remote(remote: &str, tar_path: &Path) -> Result<()> {
    // For this simple implementation, we'll try HTTP POST upload
    let upload_url = if remote.ends_with('/') {
        format!("{}upload", remote)
    } else {
        format!("{}/upload", remote)
    };
    
    #[cfg(not(feature = "online_judge"))]
    println!("Uploading to: {}", upload_url);
    
    let tar_data = fs::read(tar_path)?;
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&upload_url)
        .header("Content-Type", "application/octet-stream")
        .body(tar_data)
        .send()
        .context("Failed to upload to remote repository")?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP error: {}", response.status());
    }
    
    Ok(())
}

fn copy_to_local_remote(remote: &str, git_dir: &Path) -> Result<()> {
    // Try to treat remote as a local filesystem path
    let remote_path = Path::new(remote);
    
    // Create the remote directory if it doesn't exist
    if !remote_path.exists() {
        fs::create_dir_all(remote_path)?;
    }
    
    let remote_git = remote_path.join(".git");
    
    // Remove existing .git directory if it exists
    if remote_git.exists() {
        fs::remove_dir_all(&remote_git)?;
    }
    
    // Copy our .git directory to the remote location
    copy_dir_recursive(git_dir, &remote_git)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Copied .git directory to: {}", remote_git.display());
    
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
} 