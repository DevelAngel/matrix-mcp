use anyhow::{Context, Result};
use matrix_sdk::Client;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::events::room::message::{
    FormattedBody, MessageType, RoomMessageEventContent, TextMessageEventContent,
};
use matrix_sdk::ruma::{OwnedRoomId, RoomId, RoomOrAliasId};

/// Sends `text` (interpreted as Markdown) as a single message to
/// `room_id_or_alias`, then returns. Callers that only need to send one-off
/// messages don't need a continuous sync loop or event handler - just enough
/// of a sync to have the room and device state needed to send (encrypted)
/// messages.
pub async fn send_message(client: &Client, room_id_or_alias: &str, text: &str) -> Result<()> {
    let room_or_alias_id = RoomOrAliasId::parse(room_id_or_alias)
        .with_context(|| format!("'{room_id_or_alias}' is neither a valid room ID nor alias"))?;

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

    let mut text_content = TextMessageEventContent::plain(text);
    text_content.formatted = FormattedBody::markdown(text);
    let content = RoomMessageEventContent::new(MessageType::Text(text_content));

    room.send(content).await.context("failed to send message")?;
    tracing::info!("message sent to {room_id}");

    Ok(())
}
