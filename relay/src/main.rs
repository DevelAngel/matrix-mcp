mod cli;

use crate::cli::Cli;
use matrix_bot::Bot;
use matrix_sampling::{LLM, Message as LLMMessage};

use anyhow::{Context, Result};
use clap::Parser;
use reqwest::Client;
use reqwest::header::HeaderMap;
use rmcp::model::{
    ClientCapabilities, ClientInfo, Implementation, ReadResourceRequestParams, ReadResourceResult,
    ResourceContents, Role, SamplingContent,
};
#[allow(deprecated)] // MCP sampling
use rmcp::model::{
    CreateMessageRequestParams, CreateMessageResult, SamplingMessage, SamplingMessageContentBlock,
};
use rmcp::service::{RequestContext, RoleClient};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::{AuthClient, ClientCredentialsConfig, OAuthState};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{ClientHandler, ErrorData as McpError, ServiceExt};
use secrecy::ExposeSecret;

use std::io;
use std::time::Duration;

struct Agent {
    llm: LLM,
}

impl Agent {
    fn new(api_base_url: &str, api_key: Option<&str>) -> Self {
        let llm = LLM::connect(api_base_url, api_key);
        Self { llm }
    }
}

impl ClientHandler for Agent {
    #[allow(deprecated)] // MCP sampling
    fn get_info(&self) -> ClientInfo {
        ClientInfo::new(
            ClientCapabilities::builder().enable_sampling().build(),
            Implementation::new("matrix-relay", env!("CARGO_PKG_VERSION")),
        )
    }

    #[allow(deprecated)] // MCP sampling
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, McpError> {
        tracing::info!("Received sampling request with {:?}", params);

        let conv_msg = |msg: SamplingContent<SamplingMessageContentBlock>| -> Option<String> {
            match msg {
                SamplingContent::Single(SamplingMessageContentBlock::Text(c)) => Some(c.text),
                SamplingContent::Single(c) => {
                    tracing::warn!("ignore unsupported sampling content: {c:?}");
                    None
                }
                SamplingContent::Multiple(c) => {
                    tracing::warn!("ignore unsupported sampling content: {c:?}");
                    None
                }
            }
        };

        let temperature = params.temperature.unwrap_or(0.5);
        let system_prompt = params.system_prompt;
        let messages: Vec<_> = params
            .messages
            .into_iter()
            .filter_map(|msg| {
                conv_msg(msg.content).map(|content| match msg.role {
                    Role::User => LLMMessage::User(content),
                    Role::Assistant => LLMMessage::Assistant(content),
                })
            })
            .collect();
        let response_text = self
            .llm
            .send(system_prompt.as_deref(), messages, temperature)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "failed to retrieve a motivating sentence from LLM",
                    Some(e.to_string().into()),
                )
            })?;

        Ok(CreateMessageResult::new(
            SamplingMessage::assistant_text(response_text),
            "mock_llm".to_string(),
        )
        .with_stop_reason(CreateMessageResult::STOP_REASON_END_TURN))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(io::stderr)
        .init();

    let api_key = None; // unsupported yet
    let text = generate_message(
        &cli.generate_url,
        &cli.generate_resource,
        &cli.generate_client_id,
        cli.generate_client_secret.expose_secret(),
        &cli.llm_api_base_url,
        api_key,
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
    api_base_url: &str,
    api_key: Option<&str>,
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

    let client = Agent::new(api_base_url, api_key)
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
