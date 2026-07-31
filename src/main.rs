mod cli;

use crate::cli::{Cli, Parser};

use anyhow::{Context, Result};
use matrix_sdk::Client;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::ruma::events::room::message::{
    FormattedBody, MessageType, RoomMessageEventContent, TextMessageEventContent,
};
use matrix_sdk::ruma::{OwnedRoomId, RoomId, RoomOrAliasId};
use secrecy::ExposeSecret;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .init();

    let client = login(
        &cli.homeserver,
        &cli.devicename,
        &cli.username,
        cli.password.expose_secret(),
        &cli.state_dir,
    )
    .await?;

    recover_device(&client, cli.recovery_key.expose_secret()).await?;

    send_message(&client, &cli.room_id, &cli.message_file).await?;
    Ok(())
}

/// Sends the Markdown content of `message_file` as a single message to
/// `room_id`, then returns. The bot doesn't need to read or react to any
/// messages, so we don't run a continuous sync loop or event handler - just
/// enough of a sync to have the room and device state needed to send
/// (encrypted) messages.
async fn send_message(client: &Client, room_id_or_alias: &str, message_file: &Path) -> Result<()> {
    let room_or_alias_id = RoomOrAliasId::parse(room_id_or_alias)
        .with_context(|| format!("'{room_id_or_alias}' is neither a valid room ID nor alias"))?;

    let text = std::fs::read_to_string(message_file)
        .with_context(|| format!("failed to read message file {}", message_file.display()))?;

    client
        .sync_once(SyncSettings::default())
        .await
        .context("failed to sync")?;

    // A room alias (e.g. "#quests:drossos.de") isn't a room ID and can't be
    // looked up with `get_room` directly - it has to be resolved to the
    // actual room ID via the server first.
    let room_id: OwnedRoomId = match <&RoomId>::try_from(&*room_or_alias_id) {
        Ok(room_id) => room_id.to_owned(),
        Err(alias) => {
            client
                .resolve_room_alias(alias)
                .await
                .with_context(|| format!("failed to resolve room alias {alias}"))?
                .room_id
        }
    };

    let room = client
        .get_room(&room_id)
        .with_context(|| format!("not a member of room {room_id}, or room is unknown"))?;

    let mut text_content = TextMessageEventContent::plain(&text);
    text_content.formatted = FormattedBody::markdown(&text);
    let content = RoomMessageEventContent::new(MessageType::Text(text_content));

    room.send(content).await.context("failed to send message")?;
    tracing::info!("message sent to {room_id}");

    Ok(())
}

/// Uses the account's recovery key to import the cross-signing secrets onto
/// this (freshly logged-in) device. Without this, every login creates a new,
/// unsigned device that other clients (e.g. Element) show as untrusted, even
/// though E2E encryption itself already works.
async fn recover_device(client: &Client, recovery_key: &str) -> Result<()> {
    let recovery = client.encryption().recovery();

    if recovery.state() == RecoveryState::Enabled {
        tracing::info!("recovery already enabled, skipping");
        return Ok(());
    }

    // Recovery keys are usually displayed/copied in space-separated groups
    // (e.g. "EsTx A2eq HHZa ..."), but the SDK expects the key without any
    // whitespace, so strip it before using it.
    let recovery_key: String = recovery_key
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    recovery
        .recover(&recovery_key)
        .await
        .context("failed to recover secrets with the provided recovery key")?;

    tracing::info!("recovered secrets, device is now cross-signed and trusted");
    Ok(())
}

async fn login(
    homeserver: &str,
    devicename: &str,
    username: &str,
    password: &str,
    state_dir: &Path,
) -> Result<Client> {
    std::fs::create_dir_all(state_dir)
        .with_context(|| format!("failed to create state dir {}", state_dir.display()))?;
    let session_file = state_dir.join("session.json");

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(state_dir.join("store"), None)
        .build()
        .await
        .context("failed to create matrix client")?;

    if let Some(session) = load_session(&session_file)? {
        client
            .restore_session(session)
            .await
            .context("failed to restore existing session")?;
        tracing::info!("restored existing session for {username}, reusing device");
    } else {
        client
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name(devicename)
            .await
            .with_context(|| format!("failed to login to {username}"))?;
        tracing::info!("logged in as {username} with a new device");

        let session = client
            .matrix_auth()
            .session()
            .context("client has no session right after login")?;
        save_session(&session_file, &session)?;
    }

    Ok(client)
}

/// Loads a previously persisted login session, if any, so we can reuse the
/// same device on subsequent runs instead of creating a new one every time.
fn load_session(session_file: &Path) -> Result<Option<MatrixSession>> {
    if !session_file.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(session_file)
        .with_context(|| format!("failed to read session file {}", session_file.display()))?;
    let session: MatrixSession = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse session file {}", session_file.display()))?;
    Ok(Some(session))
}

/// Persists the login session to disk so it can be reused (via
/// `load_session`) on the next run.
fn save_session(session_file: &Path, session: &MatrixSession) -> Result<()> {
    let data = serde_json::to_string_pretty(session).context("failed to serialize session")?;
    std::fs::write(session_file, data)
        .with_context(|| format!("failed to write session file {}", session_file.display()))?;
    Ok(())
}
