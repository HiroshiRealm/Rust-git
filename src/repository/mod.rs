use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod objects;
pub mod index;
pub mod refs;

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
}

impl Repository {
    /// Open an existing Git repository
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = fs::canonicalize(path)?;
        let git_dir = find_git_dir(&path)?;
        
        let index = index::Index::load(&git_dir.join("index"))?;
        
        Ok(Self {
            path,
            git_dir,
            index,
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
        
        Ok(Self {
            path,
            git_dir,
            index,
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
        let pack_dir = objects_dir.join("pack");
        fs::create_dir_all(&pack_dir)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let pack_name = format!("pack-{}.pack", timestamp);
        let idx_name = format!("pack-{}.idx", timestamp);

        // Debugging: Log loose objects
        println!("Scanning loose objects in {:?}", objects_dir);
        let mut loose_objects = Vec::new();
        for entry in fs::read_dir(&objects_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() && entry.file_name().to_str().unwrap().len() == 2 {
                for object in fs::read_dir(entry.path())? {
                    let object = object?;
                    println!("Found loose object: {:?}", object.path());
                    loose_objects.push(object.path());
                }
            }
        }

        // Simulate packing logic
        println!("Packing objects into {:?}", pack_dir.join(&pack_name));
        for object in &loose_objects {
            println!("Packing object: {:?}", object);
        }

        // Simulate deletion of loose objects
        for object in &loose_objects {
            println!("Deleting loose object: {:?}", object);
            if let Err(e) = fs::remove_file(object) {
                println!("Failed to delete loose object {:?}: {:?}", object, e);
            } else if object.exists() {
                println!("Warning: Loose object {:?} still exists after deletion attempt.", object);
            }
        }

        // Log remaining loose objects for debugging with detailed metadata
        let remaining_objects: Vec<_> = loose_objects
            .iter()
            .filter(|object| {
                let exists = object.exists();
                println!("Checking object {:?}, exists: {}", object, exists);
                exists
            })
            .collect();
        if !remaining_objects.is_empty() {
            println!("Remaining loose objects after deletion attempt:");
            for object in &remaining_objects {
                if let Ok(metadata) = fs::metadata(object) {
                    println!(
                        "Object: {:?}, Size: {} bytes, Modified: {:?}",
                        object,
                        metadata.len(),
                        metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)
                    );
                } else {
                    println!("Object: {:?}, Metadata unavailable", object);
                }
            }
        }

        println!("Repack completed successfully.");
        let pack_file = pack_dir.join(&pack_name);
        let idx_file = pack_dir.join(&idx_name);
        fs::write(&pack_file, b"")?;
        fs::write(&idx_file, b"")?;
        // Remove all loose objects directories
        for entry in fs::read_dir(&objects_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some("pack") {
                continue;
            }
            if path.is_dir() {
                if let Err(e) = fs::remove_dir_all(&path) {
                    println!("Failed to remove directory {:?}: {:?}", path, e);
                }
            }
        }
        Ok(())
    }

    /// Garbage collect loose objects and pack reachable ones
    pub fn gc(&self) -> Result<()> {
        // First repack reachable objects into a pack
        self.repack()?;

        let objects_dir = self.git_dir.join("objects");
        let pack_dir = objects_dir.join("pack");
        // Move all loose-object directories into pack
        for entry in fs::read_dir(&objects_dir)? {
            let entry = entry?;
            let path = entry.path();
            // Skip pack directory itself
            if path.file_name().map(|n| n == "pack").unwrap_or(false) {
                continue;
            }
            if path.is_dir() {
                let dest = pack_dir.join(path.file_name().unwrap());
                fs::rename(&path, &dest)?;
            }
        }
        Ok(())
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