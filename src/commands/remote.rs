use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::Write;

use crate::repository::Repository;

pub fn execute(subcommand: &str, name: &str, url: &str) -> Result<()> {
    match subcommand {
        "add" => add_remote(name, url),
        _ => anyhow::bail!("Unsupported remote subcommand: {}", subcommand),
    }
}

fn add_remote(name: &str, url: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;
    let config_path = repo.git_dir.join("config");

    let new_remote_entry = format!("\n[remote \"{}\"]\n\turl = {}\n", name, url);
    
    fs::OpenOptions::new()
        .append(true)
        .open(&config_path)
        .with_context(|| format!("Failed to open config file at {:?}", &config_path))?
        .write_all(new_remote_entry.as_bytes())
        .with_context(|| "Failed to write to config file")?;
    
    println!("Added remote '{}' with URL '{}'", name, url);
    
    Ok(())
} 