use anyhow::Result;
use std::env;
use crate::repository::{Repository, objects, refs};

pub fn execute(message: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    
    // Open the repository
    let repo = Repository::open(&current_dir)?;
    
    // Write the current tree from index
    let current_tree_id = objects::write_tree(&repo)?;
    
    // Get the current branch and parent commit
    let branch = repo.current_branch()?;
    let parent_commits = match refs::get_head_commit(&repo.git_dir) {
        Ok(commit) => vec![commit],
        Err(_) => Vec::new(), // No previous commits (initial commit)
    };
    
    // Check if there are changes to commit
    if !parent_commits.is_empty() {
        // Get the tree ID from the previous commit
        let parent_commit_id = &parent_commits[0];
        let commit_obj = objects::read_object(&repo, parent_commit_id)?;
        
        if commit_obj.object_type != "commit" {
            anyhow::bail!("Expected commit object, got {}", commit_obj.object_type);
        }
        
        // Parse the commit to get the tree ID
        let commit_content = String::from_utf8_lossy(&commit_obj.data);
        let lines: Vec<&str> = commit_content.lines().collect();
        if lines.is_empty() || !lines[0].starts_with("tree ") {
            anyhow::bail!("Invalid commit object format");
        }
        
        let previous_tree_id = lines[0].strip_prefix("tree ").unwrap().trim();
        
        // Compare current tree with previous tree
        if current_tree_id == previous_tree_id {
            println!("Nothing to commit, working tree clean");
            return Ok(());
        }
    }
    
    // Create the commit
    let author = "Rust-git <user@example.com>";
    let parent_refs: Vec<&str> = parent_commits.iter().map(|s| s.as_str()).collect();
    
    let commit_id = objects::write_commit(
        &repo.git_dir,
        &current_tree_id,
        &parent_refs,
        message,
        author,
    )?;
    
    // Update the branch reference
    refs::update_ref(
        &repo.git_dir,
        &format!("refs/heads/{}", branch),
        &commit_id,
    )?;
    
    // Save the index to preserve the current state
    repo.index.save(repo.git_dir.join("index"))?;
    
    #[cfg(feature = "online_judge")]
    println!("{}", commit_id);
    #[cfg(not(feature = "online_judge"))]
    println!("[{}] {}", branch, message);
    
    Ok(())
} 