use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

// Get the commit ID that a ref points to
pub fn read_ref<P: AsRef<Path>>(git_dir: P, ref_name: &str) -> Result<String> {
    let git_dir = git_dir.as_ref();
    let ref_path = resolve_ref_path(git_dir, ref_name);
    
    if ref_path.exists() {
        let content = fs::read_to_string(&ref_path)?;
        Ok(content.trim().to_string())
    } else {
        anyhow::bail!("Ref {} not found", ref_name)
    }
}

// Convert a ref name to a file path
pub fn resolve_ref_path<P: AsRef<Path>>(git_dir: P, ref_name: &str) -> PathBuf {
    let git_dir = git_dir.as_ref();
    
    if ref_name.starts_with("refs/") {
        return git_dir.join(ref_name);
    }
    
    if ref_name == "HEAD" {
        return git_dir.join("HEAD");
    }
    
    // Try to resolve common ref names
    let candidates = [
        format!("refs/heads/{}", ref_name),
        format!("refs/tags/{}", ref_name),
        format!("refs/remotes/{}", ref_name),
    ];
    
    for candidate in &candidates {
        let path = git_dir.join(candidate);
        if path.exists() {
            return path;
        }
    }
    
    // Default to assuming it's a branch
    git_dir.join(format!("refs/heads/{}", ref_name))
}

// Update a ref to point to a commit
pub fn update_ref<P: AsRef<Path>>(git_dir: P, ref_name: &str, commit_id: &str) -> Result<()> {
    let git_dir = git_dir.as_ref();
    let ref_path = resolve_ref_path(git_dir, ref_name);
    
    // Ensure parent directory exists
    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(&ref_path, format!("{}\n", commit_id))?;
    
    Ok(())
}

// Get the current HEAD commit
pub fn get_head_commit<P: AsRef<Path>>(git_dir: P) -> Result<String> {
    let git_dir = git_dir.as_ref();
    let head_content = fs::read_to_string(git_dir.join("HEAD"))?;
    
    if head_content.starts_with("ref: ") {
        let ref_name = head_content.trim_start_matches("ref: ").trim();
        read_ref(git_dir, ref_name)
    } else {
        Ok(head_content.trim().to_string())
    }
}

// List all branches
pub fn list_branches<P: AsRef<Path>>(git_dir: P) -> Result<Vec<String>> {
    let heads_dir = git_dir.as_ref().join("refs/heads");
    if !heads_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut branches = Vec::new();
    
    // In a real implementation, we would recursively walk the directory
    // For simplicity, we'll just look at the top-level files
    for entry in fs::read_dir(heads_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    branches.push(name_str.to_string());
                }
            }
        }
    }
    
    branches.sort();
    Ok(branches)
}

// Create a new branch
pub fn create_branch<P: AsRef<Path>>(git_dir: P, branch_name: &str, commit_id: &str) -> Result<()> {
    update_ref(git_dir, &format!("refs/heads/{}", branch_name), commit_id)
}

// Delete a branch
pub fn delete_branch<P: AsRef<Path>>(git_dir: P, branch_name: &str) -> Result<()> {
    let ref_path = resolve_ref_path(git_dir, &format!("refs/heads/{}", branch_name));
    
    if !ref_path.exists() {
        anyhow::bail!("Branch {} not found", branch_name);
    }
    
    fs::remove_file(ref_path)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    fn setup_test_git_dir() -> Result<tempfile::TempDir> {
        let temp_dir = tempdir()?;
        let git_dir = temp_dir.path();
        
        // Create directory structure
        fs::create_dir_all(git_dir.join("refs/heads"))?;
        fs::create_dir_all(git_dir.join("refs/tags"))?;
        
        // Create initial HEAD file
        fs::write(
            git_dir.join("HEAD"),
            "ref: refs/heads/master\n",
        )?;
        
        Ok(temp_dir)
    }
    
    #[test]
    fn test_resolve_ref_path() -> Result<()> {
        let temp_dir = setup_test_git_dir()?;
        let git_dir = temp_dir.path();
        
        // Test full ref path
        let full_ref_path = resolve_ref_path(git_dir, "refs/heads/master");
        assert_eq!(full_ref_path, git_dir.join("refs/heads/master"));
        
        // Test HEAD
        let head_ref_path = resolve_ref_path(git_dir, "HEAD");
        assert_eq!(head_ref_path, git_dir.join("HEAD"));
        
        // Test branch name (should resolve to refs/heads/branch)
        let branch_ref_path = resolve_ref_path(git_dir, "feature");
        assert_eq!(branch_ref_path, git_dir.join("refs/heads/feature"));
        
        Ok(())
    }
    
    #[test]
    fn test_update_and_read_ref() -> Result<()> {
        let temp_dir = setup_test_git_dir()?;
        let git_dir = temp_dir.path();
        
        let commit_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Update a ref
        update_ref(git_dir, "refs/heads/master", commit_id)?;
        
        // Read it back
        let read_commit_id = read_ref(git_dir, "refs/heads/master")?;
        assert_eq!(read_commit_id, commit_id);
        
        // Should also be able to read it with just the branch name
        let read_commit_id2 = read_ref(git_dir, "master")?;
        assert_eq!(read_commit_id2, commit_id);
        
        Ok(())
    }
    
    #[test]
    fn test_get_head_commit_symbolic() -> Result<()> {
        let temp_dir = setup_test_git_dir()?;
        let git_dir = temp_dir.path();
        
        let commit_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Update a ref
        update_ref(git_dir, "refs/heads/master", commit_id)?;
        
        // HEAD should now resolve to this commit
        let head_commit = get_head_commit(git_dir)?;
        assert_eq!(head_commit, commit_id);
        
        Ok(())
    }
    
    #[test]
    fn test_get_head_commit_detached() -> Result<()> {
        let temp_dir = setup_test_git_dir()?;
        let git_dir = temp_dir.path();
        
        let commit_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Set HEAD to a direct commit hash (detached)
        fs::write(git_dir.join("HEAD"), format!("{}\n", commit_id))?;
        
        // HEAD should directly be this commit
        let head_commit = get_head_commit(git_dir)?;
        assert_eq!(head_commit, commit_id);
        
        Ok(())
    }
    
    #[test]
    fn test_branch_operations() -> Result<()> {
        let temp_dir = setup_test_git_dir()?;
        let git_dir = temp_dir.path();
        
        let commit_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Initially no branches exist (except master which isn't created yet)
        let branches = list_branches(git_dir)?;
        assert_eq!(branches.len(), 0);
        
        // Create a branch
        create_branch(git_dir, "feature", commit_id)?;
        
        // Now should have one branch
        let branches = list_branches(git_dir)?;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0], "feature");
        
        // Delete the branch
        delete_branch(git_dir, "feature")?;
        
        // Should be back to zero branches
        let branches = list_branches(git_dir)?;
        assert_eq!(branches.len(), 0);
        
        Ok(())
    }
} 