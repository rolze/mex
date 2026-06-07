use anyhow::{Context, Result};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
#[allow(dead_code)]
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

        let Some(config_path) = config_path else {
            return Ok(config);
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
                    }
                    "image_protocol" => config.image_protocol = value.to_string(),
                    _ => {} // Ignore unknown keys
                }
            }
        }

        Ok(config)
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let Some(config_path) = Self::config_file_path() else {
            return Ok(()); // Nowhere to save
        };
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

    pub fn config_file_path() -> Option<PathBuf> {
        if let Ok(env_path) = std::env::var("MEX_CONFIG") {
            return Some(PathBuf::from(env_path));
        }
        if let Ok(home) = std::env::var("HOME") {
            Some(
                Path::new(&home)
                    .join(".config")
                    .join("mex")
                    .join("config.toml"),
            )
        } else {
            None
        }
    }
}
