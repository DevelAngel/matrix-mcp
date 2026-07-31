mod cli;

use crate::cli::{Cli, Command};
use matrix_bot::Bot;
use matrix_mcp::mcp::MatrixServer;

use anyhow::Result;
use clap::Parser;
use secrecy::ExposeSecret;

// io transport
use rmcp::ServiceExt;
use rmcp::transport;

// streamable HTTP transport
use axum::Router;
use matrix_sdk::reqwest::Url;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    // stdout is the MCP protocol channel for the `io` transport; logs must
    // never share it, or they corrupt the JSON-RPC stream. stderr always.
    tracing_subscriber::fmt()
        .with_max_level(cli.verbosity)
        .with_writer(std::io::stderr)
        .init();

    let bot = Bot::connect(
        &cli.homeserver,
        &cli.devicename,
        &cli.username,
        cli.password.expose_secret(),
        cli.recovery_key.expose_secret(),
        &cli.state_dir,
    )
    .await?;
    let server = MatrixServer::new(bot);

    match cli.command.unwrap_or_default() {
        Command::Io => run_mcp_io_server(server).await,
        Command::Http {
            addr,
            allowed_origins,
        } => run_mcp_http_server(server, addr, &allowed_origins).await,
    }
}

async fn run_mcp_io_server(server: MatrixServer) -> Result<()> {
    tracing::info!("Start stdio server");
    let service = server.serve(transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_mcp_http_server(
    server: MatrixServer,
    addr: SocketAddr,
    allowed_origins: &[Url],
) -> Result<()> {
    tracing::info!("Start streamable http server: {}", addr);
    if allowed_origins.is_empty() {
        tracing::warn!("No allowed origins");
    } else {
        let allowed_origins: Vec<_> = allowed_origins.iter().map(|url| url.to_string()).collect();
        tracing::info!("Allowed origins: {}", allowed_origins.join(", "));
    }
    let allowed_hosts: Vec<_> = allowed_origins
        .iter()
        .map(|url| url.host_str().expect("url have no host"))
        .collect();
    if !allowed_hosts.is_empty() {
        tracing::info!("Allowed hosts: {}", allowed_hosts.join(", "));
    }

    let ct = CancellationToken::new();

    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_allowed_origins(allowed_origins.iter().map(|url| url.to_string()))
            .with_allowed_hosts(allowed_hosts)
            .with_cancellation_token(ct.child_token()),
    );

    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());
    let tcp_listener = TcpListener::bind(addr).await?;
    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            signal::ctrl_c().await.unwrap();
            ct.cancel();
        })
        .await;
    Ok(())
}
