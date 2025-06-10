use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::Write;
use tar::Builder;
use walkdir::WalkDir;

use super::{objects, refs, Repository};

/// Creates a bundle file from the repository.
///
/// The bundle will be a .tar.gz file containing:
/// - All objects from the .git/objects directory.
/// - A 'packed-refs' file with a list of all branches and their commit SHAs.
/// - The HEAD file.
pub fn create_bundle(repo: &Repository, writer: impl Write) -> Result<()> {
    let git_dir = &repo.git_dir;
    let encoder = GzEncoder::new(writer, Compression::default());
    let mut ar = Builder::new(encoder);

    // 1. Add all objects
    let objects_dir = git_dir.join("objects");
    if objects_dir.exists() {
        ar.append_dir_all("objects", &objects_dir)
            .context("Failed to add objects directory to bundle")?;
    }

    // 2. Create and add packed-refs file
    let mut packed_refs_content = String::new();
    let branches = super::refs::list_branches(git_dir)?;
    for branch_name in branches {
        let ref_name = format!("refs/heads/{}", branch_name);
        if let Ok(commit_id) = super::refs::read_ref(git_dir, &ref_name) {
            packed_refs_content.push_str(&format!("{} {}\n", commit_id, ref_name));
        }
    }

    if !packed_refs_content.is_empty() {
        let mut header = tar::Header::new_gnu();
        header.set_size(packed_refs_content.len() as u64);
        header.set_cksum();
        ar.append_data(&mut header, "packed-refs", packed_refs_content.as_bytes())
            .context("Failed to add packed-refs to bundle")?;
    }

    // 3. Add HEAD
    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        ar.append_path_with_name(&head_path, "HEAD")
            .context("Failed to add HEAD to bundle")?;
    }

    ar.finish()?;

    Ok(())
}

/// Extracts a bundle file into the repository.
///
/// This will:
/// - Unpack all objects into the .git/objects directory.
/// - Update refs from the 'packed-refs' file.
/// - If `remote_name` is Some, it creates remote-tracking branches.
/// - If `remote_name` is None, it updates local branches (e.g. for a push).
pub fn unbundle(repo: &Repository, reader: impl std::io::Read, remote_name: Option<&str>) -> Result<()> {
    let git_dir = &repo.git_dir;
    let gz_decoder = flate2::read::GzDecoder::new(reader);
    let mut ar = tar::Archive::new(gz_decoder);

    let temp_dir = tempfile::tempdir_in(git_dir.parent().unwrap())?;
    
    ar.unpack(&temp_dir)?;

    // 1. Copy all objects
    let bundle_objects_path = temp_dir.path().join("objects");
    let local_objects_path = git_dir.join("objects");
    if bundle_objects_path.exists() {
        for entry in WalkDir::new(bundle_objects_path.clone()) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(&bundle_objects_path)?;
                let dest_path = local_objects_path.join(relative_path);
                
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(path, dest_path)?;
            }
        }
    }

    // 2. Update refs from packed-refs
    let packed_refs_path = temp_dir.path().join("packed-refs");
    if packed_refs_path.exists() {
        let packed_refs_content = fs::read_to_string(packed_refs_path)?;
        for line in packed_refs_content.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() == 2 {
                let commit_id = parts[0];
                let orig_ref_name = parts[1]; // e.g., "refs/heads/main"

                if let Some(r_name) = remote_name {
                    // This is a FETCH operation. Create remote-tracking refs.
                    if let Some(branch_name) = orig_ref_name.strip_prefix("refs/heads/") {
                        let remote_ref_name = format!("refs/remotes/{}/{}", r_name, branch_name);
                        refs::update_ref(git_dir, &remote_ref_name, commit_id)?;
                    }
                } else {
                    // This is a PUSH operation. Check for fast-forward and update the ref.
                    if orig_ref_name.starts_with("refs/heads/") {
                        // Get the server's current commit for this branch.
                        let server_commit_id_result = refs::read_ref(git_dir, orig_ref_name);
                        
                        if let Ok(server_commit_id) = server_commit_id_result {
                            // The branch exists on the server. Check for fast-forward.
                            if server_commit_id == commit_id {
                                // The commits are the same, nothing to do.
                            } else {
                                let is_fast_forward = objects::is_ancestor(repo, &server_commit_id, commit_id)?;
                                
                                if is_fast_forward {
                                    refs::update_ref(git_dir, orig_ref_name, commit_id)?;
                                } else {
                                    anyhow::bail!(
                                        "non-fast-forward push to branch '{}' is not allowed",
                                        orig_ref_name
                                    );
                                }
                            }
                        } else {
                            // If the branch doesn't exist on the server (server_commit_id_result is Err),
                            // it's a new branch, which is always a fast-forward. So we can update.
                            refs::update_ref(git_dir, orig_ref_name, commit_id)?;
                        }
                    }
                }
            }
        }
    }

    // 3. Update the remote-tracking HEAD file during a FETCH.
    //    We do not touch the remote's actual HEAD during a PUSH.
    if let Some(r_name) = remote_name {
        let head_path = temp_dir.path().join("HEAD");
        if head_path.exists() {
            let head_content = fs::read_to_string(head_path)?;
            if let Some(orig_ref_name) = head_content.trim().strip_prefix("ref: ") {
                if let Some(branch_name) = orig_ref_name.strip_prefix("refs/heads/") {
                    let remote_head_content = format!("ref: refs/remotes/{}/{}", r_name, branch_name);
                    let remote_head_path = git_dir.join(format!("refs/remotes/{}/HEAD", r_name));
                    if let Some(parent) = remote_head_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(remote_head_path, remote_head_content)?;
                }
            }
        }
    }

    Ok(())
} 