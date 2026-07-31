mod cli;

use crate::cli::Cli;
use matrix_bot::Bot;

use anyhow::{Context, Result};
use clap::Parser;
use reqwest::Client;
use reqwest::header::HeaderMap;
use rmcp::ServiceExt;
use rmcp::model::{
    ClientCapabilities, ClientInfo, Implementation, ReadResourceRequestParams, ReadResourceResult,
    ResourceContents,
};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::{AuthClient, ClientCredentialsConfig, OAuthState};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use secrecy::ExposeSecret;

use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(io::stderr)
        .init();

    let text = generate_message(
        &cli.generate_url,
        &cli.generate_resource,
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
/// client credentials grant, reads its `generate_resource` resource, and
/// returns the generated message text.
async fn generate_message(
    generate_url: &str,
    generate_resource: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String> {
    let oauth_http_client = Client::builder()
        .timeout(Duration::from_secs(60))
        .default_headers(HeaderMap::new())
        .build()
        .context("failed to create http client")?;
    let mut oauth_state = OAuthState::new(generate_url, Some(oauth_http_client))
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
    let auth_client = AuthClient::new(Client::default(), auth_manager);
    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(generate_url),
    );

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("matrix-relay", env!("CARGO_PKG_VERSION")),
    );
    let client = client_info
        .serve(transport)
        .await
        .with_context(|| format!("failed to connect to generate server at {generate_url}"))?;

    let result = client
        .read_resource(ReadResourceRequestParams::new(generate_resource.to_owned()))
        .await
        .with_context(|| format!("failed to read resource {generate_resource}"))?;

    let _ = client.cancel().await;

    extract_text(&result).with_context(|| format!("{generate_resource} has no text contents"))
}

/// Concatenates all text contents of a resource read result. Resources are
/// meant to return exactly one content item, but joining multiple (e.g. one
/// per sub-resource) is harmless and doesn't need special-casing.
fn extract_text(result: &ReadResourceResult) -> Option<String> {
    let text: String = result
        .contents
        .iter()
        .filter_map(|content| match content {
            ResourceContents::TextResourceContents { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() { None } else { Some(text) }
}
