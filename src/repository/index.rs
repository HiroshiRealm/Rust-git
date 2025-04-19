use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Index {
    entries: HashMap<PathBuf, IndexEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexEntry {
    pub mtime: u64,
    pub object_id: String,
    pub mode: u32,
}

impl Index {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
    
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        
        let mut file = fs::File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        if data.is_empty() {
            return Ok(Self::new());
        }
        
        let index: Index = bincode::deserialize(&data)?;
        Ok(index)
    }
    
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let data = bincode::serialize(&self)?;
        fs::write(path, data)?;
        
        Ok(())
    }
    
    pub fn add_file<P1: AsRef<Path>, P2: AsRef<Path>>(&mut self, repo_path: P1, file_path: P2, object_id: &str) -> Result<()> {
        let repo_path = repo_path.as_ref();
        let file_path = file_path.as_ref();
        
        let relative_path = if file_path.starts_with(repo_path) {
            file_path.strip_prefix(repo_path)?
        } else {
            file_path
        };
        
        let metadata = fs::metadata(file_path)?;
        
        self.entries.insert(
            relative_path.to_path_buf(),
            IndexEntry {
                mtime: metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                object_id: object_id.to_string(),
                mode: 0o100644, // regular file
            },
        );
        
        Ok(())
    }
    
    pub fn add_directory<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(&mut self, repo_path: P1, dir_path: P2, objects_dir: P3) -> Result<Vec<String>> {
        let repo_path = repo_path.as_ref();
        let dir_path = dir_path.as_ref();
        let objects_dir = objects_dir.as_ref();
        
        let mut added_files = Vec::new();
        
        for entry in WalkDir::new(dir_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            
            // Skip .git directory
            if path.to_string_lossy().contains("/.git/") {
                continue;
            }
            
            // Create blob object
            let content = fs::read(path)?;
            let object_id = super::objects::write_blob(objects_dir, &content)?;
            
            // Add to index
            self.add_file(repo_path, path, &object_id)?;
            
            let relative_path = if path.starts_with(repo_path) {
                path.strip_prefix(repo_path)?.to_string_lossy().to_string()
            } else {
                path.to_string_lossy().to_string()
            };
            
            added_files.push(relative_path);
        }
        
        Ok(added_files)
    }
    
    pub fn remove_path<P1: AsRef<Path>, P2: AsRef<Path>>(&mut self, repo_path: P1, path: P2) -> Result<Vec<String>> {
        let repo_path = repo_path.as_ref();
        let path = path.as_ref();
        
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            repo_path.join(path)
        };
        
        let mut removed_files = Vec::new();
        
        if abs_path.is_dir() {
            // Remove all files in the directory
            let entries: Vec<_> = self.entries.keys().cloned().collect();
            for entry_path in entries {
                let full_path = repo_path.join(&entry_path);
                if full_path.starts_with(&abs_path) {
                    self.entries.remove(&entry_path);
                    removed_files.push(entry_path.to_string_lossy().to_string());
                }
            }
        } else {
            // Remove a single file
            let rel_path = if abs_path.starts_with(repo_path) {
                abs_path.strip_prefix(repo_path)?
            } else {
                path
            };
            
            if self.entries.remove(&rel_path.to_path_buf()).is_some() {
                removed_files.push(rel_path.to_string_lossy().to_string());
            }
        }
        
        Ok(removed_files)
    }
    
    pub fn get_entries(&self) -> &HashMap<PathBuf, IndexEntry> {
        &self.entries
    }
    
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{tempdir, NamedTempFile};
    
    #[test]
    fn test_index_new() {
        let index = Index::new();
        assert!(index.is_empty());
        assert_eq!(index.get_entries().len(), 0);
    }
    
    #[test]
    fn test_index_load_nonexistent() -> Result<()> {
        let temp_dir = tempdir()?;
        let nonexistent_path = temp_dir.path().join("nonexistent");
        
        let index = Index::load(nonexistent_path)?;
        assert!(index.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_index_save_load() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_owned();
        
        let index = Index::new();
        index.save(&path)?;
        
        // Load it back
        let loaded_index = Index::load(&path)?;
        assert!(loaded_index.is_empty());
        
        Ok(())
    }
    
    #[test]
    fn test_add_file() -> Result<()> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path();
        
        // Create a test file
        let file_path = repo_path.join("test.txt");
        let mut file = fs::File::create(&file_path)?;
        writeln!(file, "Test content")?;
        
        let mut index = Index::new();
        let object_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Add the file to the index
        index.add_file(repo_path, &file_path, object_id)?;
        
        // Check that it was added
        assert!(!index.is_empty());
        assert_eq!(index.get_entries().len(), 1);
        
        // The relative path should be just the filename
        let relative_path = PathBuf::from("test.txt");
        assert!(index.get_entries().contains_key(&relative_path));
        
        // Check the object ID
        assert_eq!(
            index.get_entries().get(&relative_path).unwrap().object_id,
            object_id
        );
        
        Ok(())
    }
    
    #[test]
    fn test_remove_file() -> Result<()> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path();
        
        // Create a test file
        let file_path = repo_path.join("test.txt");
        let mut file = fs::File::create(&file_path)?;
        writeln!(file, "Test content")?;
        
        let mut index = Index::new();
        let object_id = "abcdef0123456789abcdef0123456789abcdef01";
        
        // Add the file to the index
        index.add_file(repo_path, &file_path, object_id)?;
        assert!(!index.is_empty());
        
        // Remove the file from the index
        let removed = index.remove_path(repo_path, &file_path)?;
        
        // Check that it was removed
        assert!(index.is_empty());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "test.txt");
        
        Ok(())
    }
    
    #[test]
    fn test_add_and_remove_directory() -> Result<()> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path();
        let objects_dir = repo_path.join("objects");
        fs::create_dir_all(&objects_dir)?;
        
        // Create a subdirectory with files
        let subdir = repo_path.join("subdir");
        fs::create_dir_all(&subdir)?;
        
        let file1_path = subdir.join("file1.txt");
        let mut file1 = fs::File::create(&file1_path)?;
        writeln!(file1, "File 1 content")?;
        
        let file2_path = subdir.join("file2.txt");
        let mut file2 = fs::File::create(&file2_path)?;
        writeln!(file2, "File 2 content")?;
        
        // Add the directory to the index
        let mut index = Index::new();
        index.add_directory(repo_path, &subdir, &objects_dir)?;
        
        // Check that files were added
        assert!(!index.is_empty());
        assert_eq!(index.get_entries().len(), 2);
        
        // Remove the directory from the index
        let removed = index.remove_path(repo_path, &subdir)?;
        
        // Check that all files were removed
        assert!(index.is_empty());
        assert_eq!(removed.len(), 2);
        
        Ok(())
    }
} 