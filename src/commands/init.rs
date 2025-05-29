use anyhow::Result;
use std::env;
use crate::repository::Repository;

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open or initialize the repository
    let _repo = Repository::init(&current_dir)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Initialized empty Git repository in {}", _repo.git_dir.display());
    
    Ok(())
} 