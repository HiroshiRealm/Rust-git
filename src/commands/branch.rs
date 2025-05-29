use anyhow::Result;
use std::env;
use crate::repository::{Repository, refs};

pub fn execute(name: Option<&str>, delete: bool) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    if let Some(name) = name {
        if delete {
            // Delete branch
            refs::delete_branch(&repo.git_dir, name)?;
            #[cfg(not(feature = "online_judge"))]
            println!("Deleted branch {}", name);
        } else {
            // Create branch
            let head_commit = refs::get_head_commit(&repo.git_dir)?;
            refs::create_branch(&repo.git_dir, name, &head_commit)?;
            #[cfg(not(feature = "online_judge"))]
            println!("Created branch {}", name);
        }
    } else {
        // List branches
        let _branches = refs::list_branches(&repo.git_dir)?;
        let _current_branch = repo.current_branch()?;
        
        #[cfg(not(feature = "online_judge"))] {
            if _branches.is_empty() {
                println!("No branches");
            } else {
                for branch in _branches {
                    if branch == _current_branch {
                        println!("* {}", branch);
                    } else {
                        println!("  {}", branch);
                    }
                }
            }
        }
    }
    
    Ok(())
} 