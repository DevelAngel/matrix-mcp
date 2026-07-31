pub use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use secrecy::SecretString;
use std::path::PathBuf;

/// One-shot MCP client: fetches a message from a "generate message" MCP
/// server, logs in as a Matrix client (or restores a persisted session),
/// sends the message, then exits. Meant to be started by a systemd oneshot
/// service on a timer, rather than run continuously.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub(crate) struct Cli {
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
    /// session across restarts, so this client keeps reusing the same device
    /// instead of accumulating a new one on every run.
    #[arg(long, env = "MATRIX_STATE_DIR", default_value = "./matrix-state")]
    pub state_dir: PathBuf,

    /// Room to send to: either a room ID (e.g. "!abcdef:example.com") or a
    /// room alias (e.g. "#room:example.com").
    #[arg(long, env = "MATRIX_ROOM_ID")]
    pub room_id: String,

    /// URL of the "generate message" MCP server's Streamable HTTP endpoint
    /// (e.g. "http://127.0.0.1:8001/mcp"), used to fetch the message text
    /// that gets sent to `room_id`.
    #[arg(long, env = "MATRIX_GENERATE_URL")]
    pub generate_url: String,

    /// Name of the tool to call on the "generate message" MCP server, e.g.
    /// "daily_report". Not fixed yet, hence configurable rather than
    /// hardcoded.
    #[arg(long, env = "MATRIX_GENERATE_TOOL")]
    pub generate_tool: String,

    /// OAuth 2.1 client ID used to authenticate with the "generate message"
    /// MCP server via the client credentials grant.
    #[arg(long, env = "MATRIX_GENERATE_CLIENT_ID")]
    pub generate_client_id: String,

    /// OAuth 2.1 client secret used to authenticate with the "generate
    /// message" MCP server via the client credentials grant.
    #[arg(long, env = "MATRIX_GENERATE_CLIENT_SECRET", hide_env_values(true))]
    pub generate_client_secret: SecretString,

    // verbose and quiet flag handling
    #[command(flatten)]
    pub verbosity: Verbosity<WarnLevel>,
}
