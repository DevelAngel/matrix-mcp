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

    let recovery_key = strip_whitespace(recovery_key);

    recovery
        .recover(&recovery_key)
        .await
        .context("failed to recover secrets with the provided recovery key")?;

    tracing::info!("recovered secrets, device is now cross-signed and trusted");
    Ok(())
}

/// Recovery keys are usually displayed/copied in space-separated groups
/// (e.g. "EsTx A2eq HHZa ..."), but the SDK expects the key without any
/// whitespace, so strip it before using it.
fn strip_whitespace(key: &str) -> String {
    key.chars().filter(|c| !c.is_whitespace()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_whitespace_removes_spaces_between_groups() {
        assert_eq!(strip_whitespace("EsTx A2eq HHZa RVYd"), "EsTxA2eqHHZaRVYd");
    }

    #[test]
    fn strip_whitespace_removes_tabs_and_newlines() {
        assert_eq!(strip_whitespace("EsTx\tA2eq\nHHZa"), "EsTxA2eqHHZa");
    }

    #[test]
    fn strip_whitespace_leaves_key_without_whitespace_untouched() {
        assert_eq!(strip_whitespace("EsTxA2eqHHZaRVYd"), "EsTxA2eqHHZaRVYd");
    }

    #[test]
    fn strip_whitespace_of_empty_string_is_empty() {
        assert_eq!(strip_whitespace(""), "");
    }
}
