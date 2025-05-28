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
            // git typically tries to print it as UTF-8, and might warn or error if it's not valid.
            // For simplicity, we'll use from_utf8_lossy which will replace invalid UTF-8 sequences.
            print!("{}", String::from_utf8_lossy(&data));
        }
        "tree" => {
            let mut g_cursor = 0;
            while g_cursor < data.len() {
                // Find the space separating mode and name
                let g_space_idx = match data[g_cursor..].iter().position(|&b| b == b' ') {
                    Some(idx) => idx + g_cursor,
                    None => anyhow::bail!("Invalid tree object: missing space after mode"),
                };
                let g_mode_str = str::from_utf8(&data[g_cursor..g_space_idx])?;

                // Find the null byte terminating the name
                let g_nul_idx = match data[g_space_idx + 1..].iter().position(|&b| b == 0) {
                    Some(idx) => idx + g_space_idx + 1,
                    None => anyhow::bail!("Invalid tree object: missing null terminator after name"),
                };
                let g_name_str = str::from_utf8(&data[g_space_idx + 1..g_nul_idx])?;

                // The SHA-1 hash is the next 20 bytes
                let g_sha1_start = g_nul_idx + 1;
                let g_sha1_end = g_sha1_start + 20;
                if g_sha1_end > data.len() {
                    anyhow::bail!("Invalid tree object: insufficient data for SHA-1 hash");
                }
                let g_sha1_bytes = &data[g_sha1_start..g_sha1_end];
                let g_sha1_hex = hex::encode(g_sha1_bytes);

                // Determine object type from mode (simplified)
                let g_entry_type = if g_mode_str == "040000" {
                    "tree"
                } else {
                    "blob"
                };

                println!("{:06} {} {}\t{}", g_mode_str, g_entry_type, g_sha1_hex, g_name_str);

                g_cursor = g_sha1_end;
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