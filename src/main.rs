mod cli;

use crate::cli::{Cli, Parser};

use anyhow::{Context, Result};
use matrix_sdk::config::SyncSettings;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::ruma::events::room::message::{
    MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
};
use matrix_sdk::{Client, Room, RoomState};
use secrecy::ExposeSecret;

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
    )
    .await?;

    recover_device(&client, cli.recovery_key.expose_secret()).await?;

    sync_loop(&client).await?;
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
    let recovery_key: String = recovery_key.chars().filter(|c| !c.is_whitespace()).collect();

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
) -> Result<Client> {
    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .build()
        .await
        .context("failed to create matrix client")?;
    client
        .matrix_auth()
        .login_username(username, password)
        .initial_device_display_name(devicename)
        .await
        .with_context(|| format!("failed to login to {username}"))?;
    tracing::info!("logged in as {username}");
    Ok(client)
}

async fn sync_loop(client: &Client) -> Result<()> {
    // An initial sync to set up state and so our bot doesn't respond to old
    // messages. If the `StateStore` finds saved state in the location given the
    // initial sync will be skipped in favor of loading state from the store
    let sync_token = client
        .sync_once(SyncSettings::default())
        .await
        .context("failed to sync")?;

    // now that we've synced, let's attach a handler for incoming room messages, so
    // we can react on it
    client.add_event_handler(on_room_message);

    // since we called `sync_once` before we entered our sync loop we must pass
    // that sync token to `sync`
    let settings = SyncSettings::default().token(sync_token.next_batch);
    // this keeps state from the server streaming in to the bot via the
    // EventHandler trait
    client.sync(settings).await?; // this essentially loops until we kill the bot
    Ok(())
}

// This fn is called whenever we see a new room message event. You notice that
// the difference between this and the other function that we've given to the
// handler lies only in their input parameters. However, that is enough for the
// rust-sdk to figure out which one to call and only do so, when the parameters
// are available.
async fn on_room_message(event: OriginalSyncRoomMessageEvent, room: Room) {
    // First, we need to unpack the message: We only want messages from rooms we are
    // still in and that are regular text messages - ignoring everything else.
    if room.state() != RoomState::Joined {
        return;
    }
    let MessageType::Text(text_content) = event.content.msgtype else {
        return;
    };

    // here comes the actual "logic": when the bot see's a `!party` in the message,
    // it responds
    if text_content.body.contains("!party") {
        let content = RoomMessageEventContent::text_plain("🎉🎊🥳 let's PARTY!! 🥳🎊🎉");
        tracing::warn!("react on command: party");
        // send our message to the room we found the "!party" command in
        room.send(content).await.unwrap();
        tracing::warn!("message sent");
    }
}
