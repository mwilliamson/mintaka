use std::{fs::OpenOptions, io::Read, path::Path};

use regex::Regex;
use serde::Deserialize;

use crate::process_types::ProcessType;

#[derive(Deserialize)]
pub(crate) struct MintakaConfig {
    pub(crate) processes: Vec<ProcessConfig>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct ProcessConfig {
    pub(crate) command: Vec<String>,

    pub(crate) name: Option<String>,

    #[serde(rename = "type")]
    process_type: Option<ProcessTypeConfig>,

    pub(crate) after: Option<String>,

    autostart: Option<bool>,

    success_regex: Option<String>,

    error_regex: Option<String>,
}

impl ProcessConfig {
    pub(crate) fn process_type(&self) -> ProcessType {
        match self.process_type.as_ref() {
            None => ProcessType::Unknown {
                success_regex: self.success_regex.as_ref().map(|regex| Regex::new(regex).unwrap()),
                error_regex: self.error_regex.as_ref().map(|regex| Regex::new(regex).unwrap()),
            },
            Some(process_type) => process_type.to_process_type()
        }
    }

    pub(crate) fn autostart(&self) -> bool {
        match self.autostart {
            None => self.after.is_none(),
            Some(autostart) => autostart,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ProcessTypeConfig {
    TscWatch,
}

impl ProcessTypeConfig {
    fn to_process_type(&self) -> ProcessType {
        match self {
            ProcessTypeConfig::TscWatch => ProcessType::TscWatch,
        }
    }
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
