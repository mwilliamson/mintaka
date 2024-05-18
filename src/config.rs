use std::{fs::OpenOptions, io::Read, path::Path};

use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct MintakaConfig {
    pub(crate) processes: Vec<ProcessConfig>,
}

#[derive(Deserialize)]
pub(crate) struct ProcessConfig {
    pub(crate) command: Vec<String>,

    pub(crate) name: Option<String>,

    #[serde(rename = "type")]
    pub(crate) process_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ConfigError {
    FileOpenFailed(std::io::Error),

    FileReadFailed(std::io::Error),

    DeserializationFailed(toml::de::Error),
}

pub(crate) fn load_config(path: &Path) -> Result<MintakaConfig, ConfigError> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(ConfigError::FileOpenFailed)?;
    let mut config_str = String::new();
    file.read_to_string(&mut config_str).map_err(ConfigError::FileReadFailed)?;

    let config: MintakaConfig = toml::from_str(&config_str)
        .map_err(ConfigError::DeserializationFailed)?;

    Ok(config)
}
