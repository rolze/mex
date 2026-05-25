use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, BufRead};
use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub target_root: Option<PathBuf>,
    pub views_root: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
    pub image_protocol: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path();
        let mut config = Config {
            target_root: None,
            views_root: None,
            db_path: None,
            image_protocol: "halfblocks".to_string(), // Default as requested
        };

        if !config_path.exists() {
            return Ok(config);
        }

        let file = fs::File::open(&config_path)
            .with_context(|| format!("Failed to open config file: {:?}", config_path))?;
        let reader = io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                
                match key {
                    "target_root" => config.target_root = Some(PathBuf::from(value)),
                    "views_root" => config.views_root = Some(PathBuf::from(value)),
                    "db_path" => {
                        let mut p = PathBuf::from(value);
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name == "mex.db" {
                                p.set_file_name("mex_v1.db");
                            }
                        }
                        config.db_path = Some(p);
                    },
                    "image_protocol" => config.image_protocol = value.to_string(),
                    _ => {} // Ignore unknown keys
                }
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut contents = String::new();
        if let Some(ref p) = self.target_root {
            contents.push_str(&format!("target_root = {}\n", p.display()));
        }
        if let Some(ref p) = self.views_root {
            contents.push_str(&format!("views_root = {}\n", p.display()));
        }
        if let Some(ref p) = self.db_path {
            contents.push_str(&format!("db_path = {}\n", p.display()));
        }
        contents.push_str(&format!("image_protocol = {}\n", self.image_protocol));

        fs::write(&config_path, contents)?;
        Ok(())
    }

    pub fn config_file_path() -> PathBuf {
        let home = std::env::var("HOME").expect("HOME environment variable not set");
        Path::new(&home).join(".config").join("mex").join("config.toml")
    }
}
