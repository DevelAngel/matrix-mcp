mod cli;

use crate::cli::Cli;
use matrix_bot::Bot;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::{AuthClient, ClientCredentialsConfig, OAuthState};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use secrecy::ExposeSecret;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(std::io::stderr)
        .init();

    let text = generate_message(
        &cli.generate_url,
        &cli.generate_tool,
        &cli.generate_client_id,
        cli.generate_client_secret.expose_secret(),
    )
    .await?;

    let bot = Bot::connect(
        &cli.homeserver,
        &cli.devicename,
        &cli.username,
        cli.password.expose_secret(),
        cli.recovery_key.expose_secret(),
        &cli.state_dir,
    )
    .await?;

    bot.send_message(&cli.room_id, &text).await?;
    tracing::info!("done, exiting");
    Ok(())
}

/// Connects as an MCP client to the "generate message" server at
/// `generate_url` (Streamable HTTP), authenticating via the OAuth 2.1
/// client credentials grant, calls its `generate_tool` tool, and returns the
/// generated message text.
async fn generate_message(
    generate_url: &str,
    generate_tool: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String> {
    let mut oauth_state = OAuthState::new(generate_url, None)
        .await
        .with_context(|| format!("failed to initialize OAuth state for {generate_url}"))?;
    oauth_state
        .authenticate_client_credentials(ClientCredentialsConfig::ClientSecret {
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
            scopes: vec![],
            resource: Some(generate_url.to_owned()),
        })
        .await
        .with_context(|| {
            format!("OAuth client credentials authentication failed for {generate_url}")
        })?;

    let auth_manager = oauth_state
        .into_authorization_manager()
        .context("failed to get OAuth authorization manager")?;
    let auth_client = AuthClient::new(reqwest::Client::default(), auth_manager);
    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(generate_url),
    );

    let client_info = rmcp::model::ClientInfo::new(
        rmcp::model::ClientCapabilities::default(),
        rmcp::model::Implementation::new("matrix-relay", env!("CARGO_PKG_VERSION")),
    );
    let client = client_info
        .serve(transport)
        .await
        .with_context(|| format!("failed to connect to generate server at {generate_url}"))?;

    let result = client
        .call_tool(
            CallToolRequestParams::new(generate_tool.to_owned()).with_arguments(
                serde_json::json!({})
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
        )
        .await
        .with_context(|| format!("failed to call {generate_tool}"))?;

    let _ = client.cancel().await;

    extract_text(&result).with_context(|| format!("{generate_tool} returned no text content"))
}

/// Pulls the generated message text out of a tool result, preferring
/// structured content (`{"text": "..."}`, matching how `#[tool]`-generated
/// servers serialize typed outputs) and falling back to concatenating any
/// plain text content blocks.
fn extract_text(result: &rmcp::model::CallToolResult) -> Option<String> {
    if let Some(text) = result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
    {
        return Some(text.to_owned());
    }

    let text: String = result
        .content
        .iter()
        .filter_map(|block| block.as_text())
        .map(|text_content| text_content.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() { None } else { Some(text) }
}
