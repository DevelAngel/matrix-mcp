pub use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use secrecy::SecretString;

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

    // verbose and quiet flag handling
    #[command(flatten)]
    pub verbosity: Verbosity<WarnLevel>,
}
