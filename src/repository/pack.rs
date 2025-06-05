use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Sha1, Digest};
use hex;

const PACK_SIGNATURE: [u8; 4] = [b'P', b'A', b'C', b'K'];
const PACK_VERSION: u32 = 2;

const OBJ_COMMIT: u8 = 1;
const OBJ_TREE: u8 = 2;
const OBJ_BLOB: u8 = 3;
const OBJ_TAG: u8 = 4;
const OBJ_OFS_DELTA: u8 = 6;
const OBJ_REF_DELTA: u8 = 7;

#[derive(Debug)]
pub struct PackFile {
    pub version: u32,
    pub object_count: u32,
    pub objects: HashMap<String, PackObject>,
}

#[derive(Debug)]
pub struct PackObject {
    pub object_type: String,
    pub data: Vec<u8>,
    pub offset: u64,
    pub size: u64,
    pub base_object: Option<String>,    // 基础对象的ID
    pub delta_data: Option<Vec<u8>>,    // 增量数据
}

impl PackFile {
    pub fn new() -> Self {
        Self {
            version: PACK_VERSION,
            object_count: 0,
            objects: HashMap::new(),
        }
    }

    fn find_best_base(&self, object_type: &str, data: &[u8]) -> Option<String> {
        // 简单的启发式算法：找到相同类型且大小相近的对象作为基础
        let mut best_base = None;
        let mut best_size_diff = usize::MAX;

        for (id, obj) in &self.objects {
            if obj.object_type == object_type && obj.base_object.is_none() {
                let size_diff = if obj.data.len() > data.len() {
                    obj.data.len() - data.len()
                } else {
                    data.len() - obj.data.len()
                };
                if size_diff < best_size_diff {
                    best_size_diff = size_diff;
                    best_base = Some(id.clone());
                }
            }
        }

        best_base
    }

    fn compute_delta(base: &[u8], target: &[u8]) -> Vec<u8> {
        // 简单的增量算法：使用滑动窗口比较
        let mut delta = Vec::new();
        let window_size = 16;
        let mut i = 0;

        while i < target.len() {
            let mut best_match = (0, 0);
            
            // 在基础对象中寻找最长匹配
            for j in 0..base.len() {
                let mut match_len = 0;
                while i + match_len < target.len() 
                    && j + match_len < base.len() 
                    && target[i + match_len] == base[j + match_len] 
                    && match_len < window_size {
                    match_len += 1;
                }
                if match_len > best_match.1 {
                    best_match = (j, match_len);
                }
            }

            if best_match.1 >= 4 { // 只存储长度大于等于4的匹配
                delta.push(0); // 复制标记
                delta.extend_from_slice(&(best_match.0 as u32).to_be_bytes());
                delta.extend_from_slice(&(best_match.1 as u32).to_be_bytes());
                i += best_match.1;
            } else {
                delta.push(1); // 插入标记
                delta.push(target[i]);
                i += 1;
            }
        }

        delta
    }

    pub fn add_object(&mut self, object_type: &str, data: &[u8]) -> Result<String> {
        let header = format!("{} {}", object_type, data.len());
        let mut content = Vec::new();
        content.extend_from_slice(header.as_bytes());
        content.push(0);
        content.extend_from_slice(data);

        let mut hasher = Sha1::new();
        hasher.update(&content);
        let object_id = hex::encode(hasher.finalize());

        // 尝试找到合适的基础对象
        if let Some(base_id) = self.find_best_base(object_type, data) {
            let base_obj = self.objects.get(&base_id).unwrap();
            let delta = Self::compute_delta(&base_obj.data, data);
            
            // 如果增量数据比原始数据小，使用增量存储
            if delta.len() < data.len() {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&delta)?;
                let compressed = encoder.finish()?;
                let size = compressed.len() as u64;

                self.objects.insert(object_id.clone(), PackObject {
                    object_type: object_type.to_string(),
                    data: compressed,
                    offset: 0,
                    size,
                    base_object: Some(base_id),
                    delta_data: Some(delta),
                });
            } else {
                // 如果增量数据不比原始数据小，使用完整存储
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&content)?;
                let compressed = encoder.finish()?;
                let size = compressed.len() as u64;

                self.objects.insert(object_id.clone(), PackObject {
                    object_type: object_type.to_string(),
                    data: compressed,
                    offset: 0,
                    size,
                    base_object: None,
                    delta_data: None,
                });
            }
        } else {
            // 没有找到合适的基础对象，使用完整存储
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&content)?;
            let compressed = encoder.finish()?;
            let size = compressed.len() as u64;

            self.objects.insert(object_id.clone(), PackObject {
                object_type: object_type.to_string(),
                data: compressed,
                offset: 0,
                size,
                base_object: None,
                delta_data: None,
            });
        }

        self.object_count += 1;
        Ok(object_id)
    }

    pub fn write<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let mut file = File::create(&path)?;
        
        // Write pack header
        file.write_all(&PACK_SIGNATURE)?;
        file.write_all(&self.version.to_be_bytes())?;
        file.write_all(&self.object_count.to_be_bytes())?;

        // Calculate offsets and write objects
        let mut offset = 12; // Header size
        for (_, obj) in self.objects.iter_mut() {
            obj.offset = offset;
            offset += obj.size;
        }

        // Write objects
        for (_, obj) in &self.objects {
            if let Some(base_id) = &obj.base_object {
                // Write delta object
                let type_byte = OBJ_REF_DELTA;
                file.write_all(&[type_byte])?;
                file.write_all(&hex::decode(base_id)?)?;
            } else {
                // Write full object
                let type_byte = match obj.object_type.as_str() {
                    "commit" => OBJ_COMMIT,
                    "tree" => OBJ_TREE,
                    "blob" => OBJ_BLOB,
                    "tag" => OBJ_TAG,
                    _ => anyhow::bail!("Unknown object type: {}", obj.object_type),
                };
                file.write_all(&[type_byte])?;
            }
            file.write_all(&obj.data)?;
        }

        // Write pack index
        let mut index = Vec::new();
        for (object_id, obj) in &self.objects {
            let object_id_bytes = hex::decode(object_id)?;
            index.extend_from_slice(&object_id_bytes);
            index.extend_from_slice(&obj.offset.to_be_bytes());
        }

        // Write index file
        let index_path = path.as_ref().with_extension("idx");
        fs::write(index_path, index)?;

        Ok(())
    }

    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(&path)?;
        
        // Read and verify pack header
        let mut signature = [0u8; 4];
        file.read_exact(&mut signature)?;
        if signature != PACK_SIGNATURE {
            anyhow::bail!("Invalid pack file signature");
        }

        let mut version_bytes = [0u8; 4];
        file.read_exact(&mut version_bytes)?;
        let version = u32::from_be_bytes(version_bytes);

        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes)?;
        let object_count = u32::from_be_bytes(count_bytes);

        // Read index file
        let index_path = path.as_ref().with_extension("idx");
        let index_data = fs::read(index_path)?;
        
        let mut objects = HashMap::new();
        let mut offset = 12; // Header size

        for i in 0..object_count {
            let idx = i as usize;
            let object_id = hex::encode(&index_data[idx * 24..idx * 24 + 20]);
            let obj_offset = u64::from_be_bytes(index_data[idx * 24 + 20..idx * 24 + 28].try_into()?);

            // Read object data
            file.seek(SeekFrom::Start(obj_offset))?;
            
            // Read object type
            let mut type_byte = [0u8; 1];
            file.read_exact(&mut type_byte)?;
            
            let (object_type, base_object) = match type_byte[0] {
                OBJ_COMMIT => ("commit".to_string(), None),
                OBJ_TREE => ("tree".to_string(), None),
                OBJ_BLOB => ("blob".to_string(), None),
                OBJ_TAG => ("tag".to_string(), None),
                OBJ_REF_DELTA => {
                    // Read base object ID
                    let mut base_id_bytes = [0u8; 20];
                    file.read_exact(&mut base_id_bytes)?;
                    let base_id = hex::encode(base_id_bytes);
                    ("delta".to_string(), Some(base_id))
                },
                OBJ_OFS_DELTA => {
                    // Read offset to base object
                    let mut offset_bytes = [0u8; 8];
                    file.read_exact(&mut offset_bytes)?;
                    let base_offset = u64::from_be_bytes(offset_bytes);
                    ("delta".to_string(), Some(format!("offset:{}", base_offset)))
                },
                _ => anyhow::bail!("Unknown object type: {}", type_byte[0]),
            };

            let mut decoder = ZlibDecoder::new(&mut file);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;

            // Parse object header
            let null_pos = decompressed
                .iter()
                .position(|&b| b == 0)
                .context("Invalid git object: no null byte")?;

            let header = std::str::from_utf8(&decompressed[0..null_pos])?;
            let parts: Vec<&str> = header.split(' ').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid git object header");
            }

            let data = decompressed[null_pos + 1..].to_vec();

            objects.insert(object_id, PackObject {
                object_type,
                data,
                offset: obj_offset,
                size: 0, // Not needed for reading
                base_object,
                delta_data: None,
            });
        }

        Ok(Self {
            version,
            object_count,
            objects,
        })
    }
}

pub fn create_pack<P: AsRef<Path>>(objects_dir: P) -> Result<()> {
    let objects_dir = objects_dir.as_ref();
    let mut pack = PackFile::new();

    // Read all loose objects
    for entry in fs::read_dir(objects_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let dir_name = entry.file_name();
            if dir_name.len() == 2 {
                for file_entry in fs::read_dir(entry.path())? {
                    let file_entry = file_entry?;
                    if file_entry.file_type()?.is_file() {
                        let file_name = file_entry.file_name();
                        let object_id = format!("{}{}", dir_name.to_string_lossy(), file_name.to_string_lossy());
                        
                        let content = fs::read(file_entry.path())?;
                        let mut decoder = ZlibDecoder::new(&content[..]);
                        let mut decompressed = Vec::new();
                        decoder.read_to_end(&mut decompressed)?;

                        let null_pos = decompressed
                            .iter()
                            .position(|&b| b == 0)
                            .context("Invalid git object: no null byte")?;

                        let header = std::str::from_utf8(&decompressed[0..null_pos])?;
                        let parts: Vec<&str> = header.split(' ').collect();
                        if parts.len() != 2 {
                            continue;
                        }

                        let object_type = parts[0];
                        let data = decompressed[null_pos + 1..].to_vec();

                        pack.add_object(object_type, &data)?;
                    }
                }
            }
        }
    }

    // Write pack file
    let pack_path = objects_dir.join("pack").join("pack-1.pack");
    if let Some(parent) = pack_path.parent() {
        fs::create_dir_all(parent)?;
    }
    pack.write(pack_path)?;

    Ok(())
}

pub fn read_pack_object<P: AsRef<Path>>(objects_dir: P, object_id: &str) -> Result<(String, Vec<u8>)> {
    let objects_dir = objects_dir.as_ref();
    let pack_dir = objects_dir.join("pack");

    // Try to find the object in pack files
    if pack_dir.exists() {
        for entry in fs::read_dir(pack_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("pack") {
                    if let Ok(pack) = PackFile::read(&path) {
                        if let Some(obj) = pack.objects.get(object_id) {
                            return Ok((obj.object_type.clone(), obj.data.clone()));
                        }
                    }
                }
            }
        }
    }

    // If not found in pack files, try loose objects
    let dir_name = &object_id[0..2];
    let file_name = &object_id[2..];
    let object_path = objects_dir.join(dir_name).join(file_name);
    
    if object_path.exists() {
        let compressed = fs::read(object_path)?;
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        let null_pos = decompressed
            .iter()
            .position(|&b| b == 0)
            .context("Invalid git object: no null byte")?;

        let header = std::str::from_utf8(&decompressed[0..null_pos])?;
        let parts: Vec<&str> = header.split(' ').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid git object header");
        }

        let object_type = parts[0].to_string();
        let data = decompressed[null_pos + 1..].to_vec();

        Ok((object_type, data))
    } else {
        anyhow::bail!("Object not found: {}", object_id)
    }
} 