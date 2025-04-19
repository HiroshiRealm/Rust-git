use anyhow::Result;
use std::env;
use std::str;
use crate::repository::Repository;
use hex;

pub fn execute(object_hash: &str) -> Result<()> {
    let current_dir = env::current_dir()?;
    let repo = Repository::open(&current_dir)?;

    let (object_type, data) = crate::repository::objects::read_object(&repo.git_dir.join("objects"), object_hash)?;

    match object_type.as_str() {
        "blob" => {
            // For blobs, just print the content as a string.
            // Git typically tries to print it as UTF-8, and might warn or error if it's not valid.
            // For simplicity, we'll use from_utf8_lossy which will replace invalid UTF-8 sequences.
            print!("{}", String::from_utf8_lossy(&data));
        }
        "tree" => {
            let mut G_cursor = 0;
            while G_cursor < data.len() {
                // Find the space separating mode and name
                let G_space_idx = match data[G_cursor..].iter().position(|&b| b == b' ') {
                    Some(idx) => idx + G_cursor,
                    None => anyhow::bail!("Invalid tree object: missing space after mode"),
                };
                let G_mode_str = str::from_utf8(&data[G_cursor..G_space_idx])?;

                // Find the null byte terminating the name
                let G_nul_idx = match data[G_space_idx + 1..].iter().position(|&b| b == 0) {
                    Some(idx) => idx + G_space_idx + 1,
                    None => anyhow::bail!("Invalid tree object: missing null terminator after name"),
                };
                let G_name_str = str::from_utf8(&data[G_space_idx + 1..G_nul_idx])?;

                // The SHA-1 hash is the next 20 bytes
                let G_sha1_start = G_nul_idx + 1;
                let G_sha1_end = G_sha1_start + 20;
                if G_sha1_end > data.len() {
                    anyhow::bail!("Invalid tree object: insufficient data for SHA-1 hash");
                }
                let G_sha1_bytes = &data[G_sha1_start..G_sha1_end];
                let G_sha1_hex = hex::encode(G_sha1_bytes);

                // Determine object type from mode (simplified)
                let G_entry_type = if G_mode_str == "040000" {
                    "tree"
                } else {
                    "blob"
                };

                println!("{:06} {} {}\t{}", G_mode_str, G_entry_type, G_sha1_hex, G_name_str);

                G_cursor = G_sha1_end;
            }
        }
        "commit" => {
            // For commits, print the commit message and other information.
            // Commit objects are plain text.
            print!("{}", String::from_utf8_lossy(&data));
        }
        _ => {
            anyhow::bail!("Unknown object type: {}", object_type);
        }
    }

    Ok(())
} 