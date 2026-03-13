// src/config.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub api_url: Option<String>,
    pub default_group: Option<u32>,
    pub caldav_url: Option<String>,
    pub caldav_username: Option<String>,
    #[serde(skip_serializing)] // Don't save password to file in plain text
    pub caldav_password: Option<String>,
    pub college_name: Option<String>,
    pub calendar_name: Option<String>,
    pub timezone: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        // Try to load password from keyring or env if not in config
        if config.caldav_password.is_none() {
            if let Ok(pass) = std::env::var("OSARS_CALDAV_PASSWORD") {
                config.caldav_password = Some(pass);
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Don't save password to file
        let mut saveable = self.clone();
        saveable.caldav_password = None;
        fs::write(path, toml::to_string_pretty(&saveable)?)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("osa2cal");
        Ok(dir.join("config.toml"))
    }
}
