mod messaging;
mod recovery;
mod session;

use anyhow::Result;
use matrix_sdk::Client;
use std::path::Path;

/// A logged-in, cross-signed Matrix client.
///
/// This is the entry point for anything that needs to talk to Matrix -
/// today the CLI binary, later e.g. an MCP server exposing `send_message`
/// as a tool.
pub struct Bot {
    client: Client,
}

impl Bot {
    /// Logs in (or restores a persisted session), then cross-signs the
    /// device using `recovery_key` so other clients trust it. See
    /// `session::login` and `recovery::recover_device` for details.
    pub async fn connect(
        homeserver: &str,
        devicename: &str,
        username: &str,
        password: &str,
        recovery_key: &str,
        state_dir: &Path,
    ) -> Result<Self> {
        let client = session::login(homeserver, devicename, username, password, state_dir).await?;
        recovery::recover_device(&client, recovery_key).await?;
        Ok(Self { client })
    }

    /// Sends `text` (Markdown) to the given room ID or alias.
    pub async fn send_message(&self, room_id_or_alias: &str, text: &str) -> Result<()> {
        messaging::send_message(&self.client, room_id_or_alias, text).await
    }
}
