mod cli;

use crate::cli::Cli;
use matrix_bot::Bot;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::StreamableHttpClientTransport;
use secrecy::ExposeSecret;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(std::io::stderr)
        .init();

    let text = generate_message(&cli.generate_url, &cli.generate_tool).await?;

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
/// `generate_url` (Streamable HTTP), calls its `generate_tool` tool, and
/// returns the generated message text.
async fn generate_message(generate_url: &str, generate_tool: &str) -> Result<String> {
    let transport = StreamableHttpClientTransport::from_uri(generate_url);
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
