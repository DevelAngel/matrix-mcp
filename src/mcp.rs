use crate::matrix::Bot;

use rmcp::ErrorData;
use rmcp::Json;
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerInfo;
use rmcp::schemars;
use rmcp::{tool, tool_handler, tool_router};

use serde::Serialize;
use serde_json::json;

type RmcpToolResult<T> = std::result::Result<T, ErrorData>;

/// MCP server exposing a single tool: sending a Markdown message to a
/// Matrix room. Just a tool, not resources - unlike reference material
/// (e.g. a data model) there's nothing here worth browsing, only an action
/// to perform.
#[derive(Clone)]
pub struct MatrixServer {
    bot: Bot,
}

/// Arguments for the `send_message` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SendMessageInput {
    /// Room to send to: either a room ID (e.g. "!abcdef:example.com") or a
    /// room alias (e.g. "#room:example.com").
    room: String,
    /// Message body, sent as Markdown (rendered to HTML for clients that
    /// support it, with a plain-text fallback for those that don't).
    text: String,
}

/// Result of the `send_message` tool.
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct SendMessageOutput {
    /// The room the message was sent to, as given in the request.
    room: String,
}

#[tool_router]
impl MatrixServer {
    pub fn new(bot: Bot) -> Self {
        Self { bot }
    }

    #[tool(
        description = "Send a Markdown-formatted message to a Matrix room, \
                        identified by room ID or alias.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn send_message(
        &self,
        Parameters(SendMessageInput { room, text }): Parameters<SendMessageInput>,
    ) -> RmcpToolResult<Json<SendMessageOutput>> {
        self.bot.send_message(&room, &text).await.map_err(|e| {
            ErrorData::internal_error("failed to send message", Some(json!(e.to_string())))
        })?;

        Ok(Json(SendMessageOutput { room }))
    }
}

#[tool_handler]
impl ServerHandler for MatrixServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default().with_instructions(
            "Sends Markdown-formatted messages to Matrix rooms on behalf of \
             a bot account. Use send_message with a room ID or alias and \
             the message text.",
        )
    }
}
