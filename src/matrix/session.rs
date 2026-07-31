use anyhow::{Context, Result};
use matrix_sdk::Client;
use matrix_sdk::authentication::matrix::MatrixSession;
use std::path::Path;

/// Builds a [`Client`] backed by a persistent SQLite store under
/// `state_dir`, then either restores a previously saved session or logs in
/// with `username`/`password` and persists the resulting session.
///
/// Restoring an existing session lets the bot reuse the same device across
/// restarts instead of accumulating a new, separately-trusted device on the
/// account every time it runs.
pub async fn login(
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
