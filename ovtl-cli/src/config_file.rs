use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliConfig {
    pub url: Option<String>,
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ovtl")
        .join("config.toml")
}

pub fn load() -> CliConfig {
    let path = config_path();
    if !path.exists() {
        return CliConfig::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

