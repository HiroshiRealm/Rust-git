use anyhow::{Result, Context};
use std::env;
use std::fs;
use std::path::Path;
use crate::repository::Repository;
use tar::Archive;

pub fn execute(remote: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Pulling from remote '{}'", remote);
    
    // Create a temporary directory for the remote git data
    let temp_dir = std::env::temp_dir().join("rust_git_pull");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;
    
    // Download and extract the remote .git directory
    if let Err(e) = download_remote_git(remote, &temp_dir) {
        #[cfg(not(feature = "online_judge"))]
        eprintln!("Failed to download from remote: {}", e);
        
        // Fallback: try to use local filesystem path
        if let Err(e2) = copy_local_git(remote, &temp_dir) {
            anyhow::bail!("Failed to download from remote: {} and failed local fallback: {}", e, e2);
        }
    }
    
    // Merge the remote objects into our repository
    merge_remote_objects(&repo, &temp_dir)?;
    
    // Update remote tracking branches
    update_remote_refs(&repo, remote, &temp_dir)?;
    
    // Merge the remote branch into current branch using existing merge command
    let current_branch = repo.current_branch()?;
    let remote_ref = format!("{}/{}", remote, current_branch);
    
    #[cfg(not(feature = "online_judge"))]
    println!("Merging remote branch: {}", remote_ref);
    
    // Call the existing merge command
    crate::commands::merge::execute(&remote_ref)?;
    
    // Clean up temp directory
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    
    #[cfg(not(feature = "online_judge"))]
    println!("Pull completed successfully");
    
    Ok(())
}

fn download_remote_git(remote: &str, temp_dir: &Path) -> Result<()> {
    // For this simple implementation, we'll assume remote is a HTTP URL
    // where we can download a tar.gz file of the .git directory
    
    let tar_url = if remote.ends_with('/') {
        format!("{}git-repo.tar.gz", remote)
    } else {
        format!("{}/git-repo.tar.gz", remote)
    };
    
    #[cfg(not(feature = "online_judge"))]
    println!("Downloading from: {}", tar_url);
    
    let response = reqwest::blocking::get(&tar_url)
        .context("Failed to download remote repository")?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP error: {}", response.status());
    }
    
    let tar_data = response.bytes()?;
    
    // Extract the tar file
    let mut archive = Archive::new(tar_data.as_ref());
    archive.unpack(temp_dir)?;
    
    Ok(())
}

fn copy_local_git(remote: &str, temp_dir: &Path) -> Result<()> {
    // Try to treat remote as a local filesystem path
    let remote_path = Path::new(remote);
    if !remote_path.exists() {
        anyhow::bail!("Remote path does not exist: {}", remote);
    }
    
    let remote_git = remote_path.join(".git");
    if !remote_git.exists() {
        anyhow::bail!("Remote .git directory does not exist: {}", remote_git.display());
    }
    
    // Copy the .git directory
    copy_dir_recursive(&remote_git, &temp_dir.join(".git"))?;
    
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

fn merge_remote_objects(repo: &Repository, temp_dir: &Path) -> Result<()> {
    let remote_objects = temp_dir.join(".git/objects");
    if !remote_objects.exists() {
        return Ok(());
    }
    
    let local_objects = &repo.git_dir.join("objects");
    
    // Copy all objects from remote to local
    for entry in fs::read_dir(&remote_objects)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let dir_name = entry.file_name();
            if dir_name.to_str().unwrap().len() == 2 {
                let local_dir = local_objects.join(&dir_name);
                fs::create_dir_all(&local_dir)?;
                
                for object_entry in fs::read_dir(entry.path())? {
                    let object_entry = object_entry?;
                    let src_path = object_entry.path();
                    let dst_path = local_dir.join(object_entry.file_name());
                    
                    if !dst_path.exists() {
                        fs::copy(&src_path, &dst_path)?;
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn update_remote_refs(repo: &Repository, remote: &str, temp_dir: &Path) -> Result<()> {
    let remote_refs = temp_dir.join(".git/refs/heads");
    if !remote_refs.exists() {
        return Ok(());
    }
    
    let local_remote_refs = repo.git_dir.join(format!("refs/remotes/{}", remote));
    fs::create_dir_all(&local_remote_refs)?;
    
    // Copy all branch refs
    for entry in fs::read_dir(&remote_refs)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let branch_name = entry.file_name();
            let src_path = entry.path();
            let dst_path = local_remote_refs.join(&branch_name);
            
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
} 