use anyhow::Result;
use std::env;
use crate::repository::Repository;

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Initialize a new repository
    let repo = Repository::init(&current_dir)?;
    
    #[cfg(not(feature = "online_judge"))]
    println!("Initialized empty Git repository in {}", repo.git_dir.display());
    
    Ok(())
} 