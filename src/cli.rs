pub use clap::Parser;
use clap::Subcommand;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use matrix_sdk::reqwest::Url;
use secrecy::SecretString;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(after_help = "If no subcommand is given, io is used by default.")]
pub(crate) struct Cli {
    // transport mode
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Matrix Homeserver, e.g. "matrix.example.com".
    #[arg(long, env = "MATRIX_HOMESERVER")]
    pub homeserver: String,

    /// Device name
    #[arg(long, env = "MATRIX_DEVICE_NAME")]
    pub devicename: String,

    /// User name
    #[arg(long, env = "MATRIX_USERNAME")]
    pub username: String,

    /// Password of user
    #[arg(long, env = "MATRIX_PASSWORD", hide_env_values(true))]
    pub password: SecretString,

    /// Recovery key used to recover secrets (and thereby cross-sign this
    /// device) after login, so the bot's device is trusted without manual
    /// verification.
    #[arg(long, env = "MATRIX_RECOVERY_KEY", hide_env_values(true))]
    pub recovery_key: SecretString,

    /// Directory used to persist the Matrix state/crypto store and the login
    /// session across restarts, so the bot keeps reusing the same device
    /// instead of accumulating a new one on every run.
    #[arg(long, env = "MATRIX_STATE_DIR", default_value = "./matrix-state")]
    pub state_dir: PathBuf,

    // verbose and quiet flag handling
    #[command(flatten)]
    pub verbosity: Verbosity<WarnLevel>,
}

#[derive(Debug, Default, Subcommand)]
pub enum Command {
    /// Runs the MCP server using the stdio transport
    #[default]
    Io,
    /// Runs the MCP server using the Streamable HTTP transport
    Http {
        /// MCP server address
        #[arg(
            long,
            env = "MATRIX_MCP_ADDRESS",
            value_name = "ADDRESS:PORT",
            default_value = "127.0.0.1:8000"
        )]
        addr: SocketAddr,
        /// Allowed Origins.
        /// Can be repeated or comma-separated.
        #[arg(
            long = "allowed-origin",
            env = "MATRIX_MCP_ALLOWED_ORIGINS",
            value_name = "BASE_URL",
            value_delimiter = ','
        )]
        allowed_origins: Vec<Url>,
    },
}
