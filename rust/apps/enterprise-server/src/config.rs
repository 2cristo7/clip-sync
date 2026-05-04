use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::Cli;

/// Top-level TOML configuration for the enterprise server.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FileConfig {
    pub server: ServerSection,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ServerSection {
    pub port: u16,
    pub bind: String,
    pub data_dir: Option<PathBuf>,
    pub log_level: String,
}

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            port: clipsync_protocol::config::PORT,
            bind: "0.0.0.0".to_string(),
            data_dir: None,
            log_level: "info".to_string(),
        }
    }
}

/// Resolved runtime configuration after merging TOML + CLI overrides.
#[derive(Debug)]
pub struct AppConfig {
    pub port: u16,
    pub bind: IpAddr,
    pub data_dir: PathBuf,
    pub log_level: String,
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".clipsync-enterprise")
}

impl AppConfig {
    /// Load configuration from an optional TOML file, then apply CLI overrides.
    pub fn load(cli: &Cli) -> Result<Self, ConfigError> {
        let file_cfg = match &cli.config {
            Some(path) => load_toml(path)?,
            None => FileConfig::default(),
        };

        let port = cli.port.unwrap_or(file_cfg.server.port);

        let bind_str = cli
            .bind
            .as_deref()
            .unwrap_or(&file_cfg.server.bind);
        let bind: IpAddr = bind_str
            .parse()
            .map_err(|_| ConfigError::InvalidBind(bind_str.to_string()))?;

        let data_dir = cli
            .data_dir
            .clone()
            .or(file_cfg.server.data_dir)
            .unwrap_or_else(default_data_dir);

        let log_level = if cli.log_level != "info" {
            cli.log_level.clone()
        } else {
            file_cfg.server.log_level
        };

        Ok(Self {
            port,
            bind,
            data_dir,
            log_level,
        })
    }
}

fn load_toml(path: &Path) -> Result<FileConfig, ConfigError> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
    toml::from_str(&contents).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, toml::de::Error),
    InvalidBind(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "failed to read config {}: {e}", p.display()),
            Self::Parse(p, e) => write!(f, "failed to parse config {}: {e}", p.display()),
            Self::InvalidBind(addr) => write!(f, "invalid bind address: {addr}"),
        }
    }
}

impl std::error::Error for ConfigError {}
