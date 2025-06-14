use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub mod objects;
pub mod index;
pub mod refs;
pub mod bundle;
pub mod config;
pub mod pack;

// Utility function for consistent path normalization across the entire system
pub fn normalize_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with("./") {
        PathBuf::from(&path_str[2..])
    } else {
        path.to_path_buf()
    }
}

pub struct Repository {
    pub path: PathBuf,
    pub git_dir: PathBuf,
    pub index: index::Index,
    pub config: config::Config,
}

impl Repository {
    /// Open an existing Git repository
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = fs::canonicalize(path)?;
        let git_dir = find_git_dir(&path)?;
        
        let index = index::Index::load(&git_dir.join("index"))?;
        let config = config::Config::open(&git_dir.join("config"))?;
        
        Ok(Self {
            path,
            git_dir,
            index,
            config,
        })
    }
    
    /// Initialize a new Git repository
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = fs::canonicalize(path)?;
        let git_dir = path.join(".git");
        
        // Create directory structure
        fs::create_dir_all(&git_dir)?;
        fs::create_dir_all(git_dir.join("objects"))?;
        fs::create_dir_all(git_dir.join("refs/heads"))?;
        fs::create_dir_all(git_dir.join("refs/tags"))?;
        
        // Create initial HEAD file
        fs::write(
            git_dir.join("HEAD"),
            "ref: refs/heads/master\n",
        )?;
        
        // Create empty config
        fs::write(
            git_dir.join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n",
        )?;
        
        // Create description
        fs::write(
            git_dir.join("description"),
            "Unnamed repository; edit this file 'description' to name the repository.\n",
        )?;
        
        // Ensure the empty tree object exists in the object store
        // The hash for an empty tree is "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        // Data for an empty tree is an empty byte array.
        objects::write_object(
            &git_dir.join("objects"),
            &[], // Empty data for an empty tree
            "tree"
        )?;
        
        // Create initial master branch with a null commit
        let null_commit = objects::write_commit(
            &git_dir.join("objects"),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904", // Empty tree
            &[],
            "Initial commit",
            "Rust-Git <user@example.com>",
        )?;
        
        // Create the master branch reference
        fs::write(
            git_dir.join("refs/heads/master"),
            format!("{}\n", null_commit),
        )?;
        
        let index = index::Index::new();
        let config = config::Config::open(&git_dir.join("config"))?;
        
        Ok(Self {
            path,
            git_dir,
            index,
            config,
        })
    }
    
    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String> {
        let head_content = fs::read_to_string(self.git_dir.join("HEAD"))?;
        if head_content.starts_with("ref: refs/heads/") {
            Ok(head_content
                .trim_start_matches("ref: refs/heads/")
                .trim_end()
                .to_string())
        } else {
            anyhow::bail!("HEAD is detached")
        }
    }

    /// Repack all loose objects into a pack file
    pub fn repack(&self) -> Result<()> {
        let objects_dir = self.git_dir.join("objects");
        pack::create_pack(&objects_dir)
    }

    /// Garbage collect loose objects and pack reachable ones
    pub fn gc(&self) -> Result<()> {
        // In a more complete implementation, gc would first determine which objects are
        // truly unreachable by traversing the commit graph from all refs.
        // For now, we treat all loose objects as reachable and pack them.
        // The cleanup of loose objects is now handled inside create_pack.
        self.repack()
    }
}

/// Find the .git directory by looking up the directory tree
fn find_git_dir(start_path: &Path) -> Result<PathBuf> {
    let mut current = start_path.to_path_buf();
    
    loop {
        let git_dir = current.join(".git");
        if git_dir.is_dir() {
            return Ok(git_dir);
        }
        
        if !current.pop() {
            anyhow::bail!("Not a git repository (or any of the parent directories)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    
    fn setup_test_repo() -> Result<(tempfile::TempDir, Repository)> {
        let temp_dir = tempfile::tempdir()?;
        let repo = Repository::init(&temp_dir)?;
        Ok((temp_dir, repo))
    }
    
    #[test]
    fn test_init() -> Result<()> {
        let (temp_dir, repo) = setup_test_repo()?;
        
        // Check if .git directory exists
        assert!(repo.git_dir.exists());
        
        // Check if necessary subdirectories exist
        assert!(repo.git_dir.join("objects").exists());
        assert!(repo.git_dir.join("refs/heads").exists());
        assert!(repo.git_dir.join("refs/tags").exists());
        
        // Check if HEAD points to master branch
        let head_content = fs::read_to_string(repo.git_dir.join("HEAD"))?;
        assert_eq!(head_content, "ref: refs/heads/master\n");
        
        // Check current branch
        assert_eq!(repo.current_branch()?, "master");
        
        Ok(())
    }
    
    #[test]
    fn test_open() -> Result<()> {
        let (temp_dir, _) = setup_test_repo()?;
        
        // Open existing repository
        let repo = Repository::open(&temp_dir)?;
        
        // Check if .git directory exists
        assert!(repo.git_dir.exists());
        
        // Check current branch
        assert_eq!(repo.current_branch()?, "master");
        
        Ok(())
    }
    
    #[test]
    fn test_find_git_dir() -> Result<()> {
        let (temp_dir, repo) = setup_test_repo()?;
        
        // Create a subdirectory
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir_all(&subdir)?;
        
        // Find .git from subdirectory
        let git_dir = find_git_dir(&subdir)?;
        
        // Normalize paths by converting to absolute paths
        let normalized_git_dir = fs::canonicalize(&git_dir)?;
        let normalized_repo_git_dir = fs::canonicalize(&repo.git_dir)?;
        
        // Check if they match after normalization
        assert_eq!(normalized_git_dir, normalized_repo_git_dir);
        
        Ok(())
    }
} 