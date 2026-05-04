use std::path::PathBuf;

use clap::Parser;

/// ClipSync Enterprise Server — headless daemon for centralized clipboard sync
#[derive(Parser, Debug)]
#[command(name = "enterprise-server", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// Path to TOML configuration file
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// TCP port to listen on (overrides config file)
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Data directory for tokens, TLS certs, and state
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Log level: trace, debug, info, warn, error
    #[arg(long, value_name = "LEVEL", default_value = "info")]
    pub log_level: String,

    /// Bind address
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<String>,
}
