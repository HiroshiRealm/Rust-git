use anyhow::{Result};
use std::collections::HashMap;
use std::fs;
use std::io::{Write};
use std::path::{Path};
use std::time::{SystemTime, UNIX_EPOCH};
use sha1::{Sha1, Digest};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use hex;
use fossil_delta;

use super::objects;

struct PackedObject {
    oid: String,
    object_type: String,
    data: Vec<u8>, // Raw data without git object header
    offset: u64,
}

pub fn create_pack(objects_dir: &Path) -> Result<()> {
    // 1. Collect all loose objects
    let mut loose_objects = Vec::new();
    for entry in fs::read_dir(objects_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.file_name().and_then(|s| s.to_str()).map_or(false, |s| s.len() == 2) {
            for object_entry in fs::read_dir(path)? {
                let object_entry = object_entry?;
                let object_path = object_entry.path();
                if object_path.is_file() {
                    let dir_name = object_path.parent().unwrap().file_name().unwrap().to_str().unwrap();
                    let file_name = object_path.file_name().unwrap().to_str().unwrap();
                    let oid = format!("{}{}", dir_name, file_name);
                    let (object_type, data) = objects::read_object(objects_dir, &oid)?;
                    loose_objects.push(PackedObject { oid, object_type, data, offset: 0 });
                }
            }
        }
    }

    if loose_objects.is_empty() { return Ok(()); }

    // 2. Sort objects by type and size to improve delta potential
    loose_objects.sort_by(|a, b| a.object_type.cmp(&b.object_type).then(a.data.len().cmp(&b.data.len())));

    // 3. Prepare pack data, finding deltas along the way
    let mut packed_items = Vec::new();
    let mut packed_objects_for_lookup: Vec<&PackedObject> = Vec::new();
    
    for obj in &loose_objects {
        let mut best_base: Option<(&PackedObject, Vec<u8>)> = None;

        let search_window = packed_objects_for_lookup.iter().rev().take(10);
        for base in search_window {
            if obj.object_type == base.object_type {
                let delta = fossil_delta::delta(&base.data, &obj.data);
                if !delta.is_empty() && delta.len() < obj.data.len() {
                    best_base = Some((base, delta));
                    break;
                }
            }
        }

        if let Some((base, delta)) = best_base {
            packed_items.push(PackEntry::Delta { oid: obj.oid.clone(), base_oid: base.oid.clone(), delta });
        } else {
            packed_items.push(PackEntry::Full { oid: obj.oid.clone(), object_type: obj.object_type.clone(), data: obj.data.clone() });
        }
        packed_objects_for_lookup.push(obj);
    }
    
    // 4. Write pack file
    write_pack_file(objects_dir, &mut packed_items)
}

enum PackEntry {
    Full { oid: String, object_type: String, data: Vec<u8> },
    Delta { oid: String, base_oid: String, delta: Vec<u8> },
}

fn write_pack_file(objects_dir: &Path, items: &mut Vec<PackEntry>) -> Result<()> {
    let pack_dir = objects_dir.join("pack");
    fs::create_dir_all(&pack_dir)?;
    
    let pack_name_sha = Sha1::new().chain_update(format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()).as_bytes()).finalize();
    let pack_name = format!("pack-{}", hex::encode(pack_name_sha));
    let pack_file_path = pack_dir.join(format!("{}.pack", &pack_name));
    let idx_file_path = pack_dir.join(format!("{}.idx", &pack_name));

    let mut pack_file = fs::File::create(&pack_file_path)?;
    pack_file.write_all(b"PACK")?;
    pack_file.write_all(&2u32.to_be_bytes())?;
    pack_file.write_all(&(items.len() as u32).to_be_bytes())?;

    let mut current_offset = 12;
    let mut oid_to_offset_map = HashMap::new();
    let mut final_offsets = HashMap::new();

    for item in items.iter_mut() {
        let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
        let header;
        
        match item {
            PackEntry::Full { oid: _, object_type, data } => {
                let mut full_data_with_header = format!("{} {}\0", object_type, data.len()).into_bytes();
                full_data_with_header.extend_from_slice(data);
                compressor.write_all(&full_data_with_header)?;
                header = get_pack_header(full_data_with_header.len(), object_type)?;
            }
            PackEntry::Delta { oid: _, base_oid, delta } => {
                let base_offset = oid_to_offset_map.get(base_oid).unwrap();
                let offset_delta = current_offset - base_offset;
                
                let mut delta_with_offset = Vec::new();
                let mut d = offset_delta;
                loop {
                    let mut byte = (d & 0x7f) as u8;
                    d >>= 7;
                    if d > 0 { byte |= 0x80; }
                    delta_with_offset.push(byte);
                    if d == 0 { break; }
                }
                delta_with_offset.extend_from_slice(delta);
                compressor.write_all(&delta_with_offset)?;
                header = get_pack_header(delta_with_offset.len(), "offset_delta")?;
            }
        }
        
        let compressed_data = compressor.finish()?;
        let oid_str = match item {
            PackEntry::Full { oid, .. } => oid,
            PackEntry::Delta { oid, .. } => oid,
        };
        oid_to_offset_map.insert(oid_str.clone(), current_offset);
        final_offsets.insert(oid_str.clone(), current_offset);

        pack_file.write_all(&header)?;
        pack_file.write_all(&compressed_data)?;
        current_offset += (header.len() + compressed_data.len()) as u64;
    }

    let pack_content = fs::read(&pack_file_path)?;
    let pack_sha = Sha1::new().chain_update(&pack_content).finalize();
    pack_file.write_all(&pack_sha[..])?;
    
    write_idx_file(&idx_file_path, items, &final_offsets, &pack_sha)?;
    
    // Cleanup: Precisely remove only the loose objects that were packed.
    for item in items.iter() {
        let oid = match item {
            PackEntry::Full { oid, .. } => oid,
            PackEntry::Delta { oid, .. } => oid,
        };
        let object_path = objects_dir.join(&oid[0..2]).join(&oid[2..]);
        if object_path.exists() {
            fs::remove_file(&object_path)?;
        }

        let dir_path = objects_dir.join(&oid[0..2]);
        if dir_path.exists() && dir_path.is_dir() {
            if let Ok(mut read_dir) = fs::read_dir(&dir_path) {
                if read_dir.next().is_none() {
                    fs::remove_dir(&dir_path)?;
                }
            }
        }
    }

    Ok(())
}

fn get_pack_header(size: usize, object_type: &str) -> Result<Vec<u8>> {
    let type_id = match object_type {
        "commit" => 1,
        "tree" => 2,
        "blob" => 3,
        "tag" => 4,
        "offset_delta" => 6,
        _ => anyhow::bail!("Unknown object type for packing: {}", object_type),
    };
    let mut header = Vec::new();
    let mut s = size;
    let mut byte = ((type_id << 4) | (s & 0x0f)) as u8;
    s >>= 4;
    while s > 0 {
        header.push(byte | 0x80);
        byte = (s & 0x7f) as u8;
        s >>= 7;
    }
    header.push(byte);
    header.reverse();
    Ok(header)
}

fn write_idx_file(idx_path: &Path, items: &[PackEntry], offsets: &HashMap<String, u64>, pack_sha: &[u8]) -> Result<()> {
    let mut idx_file = fs::File::create(idx_path)?;
    idx_file.write_all(&[0xff, 0x74, 0x4f, 0x63, 0x00, 0x00, 0x00, 0x02])?;

    let mut sorted_oids: Vec<&String> = offsets.keys().collect();
    sorted_oids.sort();
    
    // Fanout table
    let mut fanout = [0u32; 256];
    for (i, oid_str) in sorted_oids.iter().enumerate() {
        let first_byte = hex::decode(&oid_str[0..2])?[0] as usize;
        for j in first_byte..256 {
            fanout[j] = (i + 1) as u32;
        }
    }
    for count in fanout.iter() {
        idx_file.write_all(&count.to_be_bytes())?;
    }

    // OIDs
    for oid in &sorted_oids {
        idx_file.write_all(&hex::decode(oid)?)?;
    }
    // CRCs (dummy)
    for _ in 0..items.len() { idx_file.write_all(&0u32.to_be_bytes())?; }
    // Offsets
    for oid in &sorted_oids {
        idx_file.write_all(&(offsets[oid.as_str()] as u32).to_be_bytes())?;
    }
    
    idx_file.write_all(pack_sha)?;
    let idx_content = fs::read(idx_path)?;
    idx_file.write_all(&Sha1::new().chain_update(&idx_content).finalize()[..])?;

    Ok(())
} 