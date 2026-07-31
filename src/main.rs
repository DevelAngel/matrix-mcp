mod cli;

use crate::cli::{Cli, Parser};
use anyhow::{Context, Result};
use matrix_mcp::matrix::Bot;
use secrecy::ExposeSecret;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .init();

    let bot = Bot::connect(
        &cli.homeserver,
        &cli.devicename,
        &cli.username,
        cli.password.expose_secret(),
        cli.recovery_key.expose_secret(),
        &cli.state_dir,
    )
    .await?;

    let text = std::fs::read_to_string(&cli.message_file)
        .with_context(|| format!("failed to read message file {}", cli.message_file.display()))?;

    bot.send_message(&cli.room_id, &text).await?;
    Ok(())
}
