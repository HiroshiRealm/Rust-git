use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub type ConfigSection = HashMap<String, String>;
pub type ConfigData = HashMap<String, ConfigSection>;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub data: ConfigData,
}

impl Config {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let data = Self::parse(&content);
        Ok(Self { data })
    }

    fn parse(content: &str) -> ConfigData {
        let mut data = ConfigData::new();
        let mut current_section_name = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let section_name = line.trim_matches(|c| c == '[' || c == ']').to_string();
                current_section_name = section_name;
                data.entry(current_section_name.clone()).or_insert_with(HashMap::new);
            } else if let Some((key, value)) = line.split_once('=') {
                if !current_section_name.is_empty() {
                    if let Some(section) = data.get_mut(&current_section_name) {
                        section.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
            }
        }
        data
    }

    pub fn get_remote_url(&self, remote_name: &str) -> Option<&String> {
        let section_name = format!("remote \"{}\"", remote_name);
        self.data.get(&section_name)?.get("url")
    }
} 