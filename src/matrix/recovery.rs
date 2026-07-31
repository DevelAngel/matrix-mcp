use anyhow::{Context, Result};
use matrix_sdk::Client;
use matrix_sdk::encryption::recovery::RecoveryState;

/// Uses the account's recovery key to import the cross-signing secrets onto
/// this (freshly logged-in) device. Without this, every login creates a new,
/// unsigned device that other clients (e.g. Element) show as untrusted, even
/// though E2E encryption itself already works.
pub async fn recover_device(client: &Client, recovery_key: &str) -> Result<()> {
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
