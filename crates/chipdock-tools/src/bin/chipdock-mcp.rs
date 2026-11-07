use chipdock_control::{ApiError, MAX_PAGE, Request, call};
use chipdock_tools::{ConnectionOptions, FormatOption};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{future::Future, pin::Pin};

type Pending = Pin<Box<dyn Future<Output = Result<Value, ApiError>> + Send>>;
type Client = fn(Request) -> Pending;

struct ChipDock {
    client: Client,
}

fn local(request: Request) -> Pending {
    Box::pin(call(request))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Session {
    session_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Records {
    session_id: u64,
    #[serde(default)]
    cursor: u64,
    #[serde(default = "page_limit")]
    limit: u32,
}

fn page_limit() -> u32 {
    MAX_PAGE
}

#[derive(Deserialize, JsonSchema)]
struct Connect {
    session_id: u64,
    path: String,
    #[serde(flatten)]
    options: ConnectionOptions,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SendArgs {
    session_id: u64,
    #[serde(default)]
    format: FormatOption,
    input: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Select {
    session_id: u64,
    name: Option<String>,
}

#[tool_router]
impl ChipDock {
    #[tool(
        description = "List serial session IDs and connection status in the running ChipDock app."
    )]
    async fn serial_sessions(&self) -> CallToolResult {
        self.invoke(Request::Sessions).await
    }

    #[tool(description = "List available serial ports, including the virtual pair.")]
    async fn serial_ports(&self) -> CallToolResult {
        self.invoke(Request::Ports).await
    }

    #[tool(
        description = "Read connection status, byte counts and the selected template for one session."
    )]
    async fn serial_status(
        &self,
        Parameters(Session { session_id }): Parameters<Session>,
    ) -> CallToolResult {
        self.invoke(Request::Status { session_id }).await
    }

    #[tool(
        description = "Read up to 64 serial records after the given cursor, with the next cursor and any gap."
    )]
    async fn serial_records(
        &self,
        Parameters(Records {
            session_id,
            cursor,
            limit,
        }): Parameters<Records>,
    ) -> CallToolResult {
        self.invoke(Request::Records {
            session_id,
            cursor,
            limit,
        })
        .await
    }

    #[tool(description = "List named send templates configured for a serial session.")]
    async fn serial_protocols(
        &self,
        Parameters(Session { session_id }): Parameters<Session>,
    ) -> CallToolResult {
        self.invoke(Request::Protocols { session_id }).await
    }

    #[tool(
        description = "Connect the specified session to a serial port using optional serial settings. Requires control permission."
    )]
    async fn serial_connect(
        &self,
        Parameters(Connect {
            session_id,
            path,
            options,
        }): Parameters<Connect>,
    ) -> CallToolResult {
        let reconnect = options.reconnect;
        match options.try_into() {
            Ok(config) => {
                self.invoke(Request::Connect {
                    session_id,
                    path,
                    config,
                    reconnect,
                })
                .await
            }
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Disconnect one serial session and cancel its reconnect and send tasks. Requires control permission."
    )]
    async fn serial_disconnect(
        &self,
        Parameters(Session { session_id }): Parameters<Session>,
    ) -> CallToolResult {
        self.invoke(Request::Disconnect { session_id }).await
    }

    #[tool(
        description = "Send UTF-8 text, HEX bytes or arguments to the selected send template. Requires control permission."
    )]
    async fn serial_send(
        &self,
        Parameters(SendArgs {
            session_id,
            format,
            input,
        }): Parameters<SendArgs>,
    ) -> CallToolResult {
        self.invoke(Request::Send {
            session_id,
            format: format.into(),
            input,
        })
        .await
    }

    #[tool(
        description = "Select a named send template for command-mode sends, or clear selection with null. Requires control permission."
    )]
    async fn serial_select_protocol(
        &self,
        Parameters(Select { session_id, name }): Parameters<Select>,
    ) -> CallToolResult {
        self.invoke(Request::SelectProtocol { session_id, name })
            .await
    }

    #[tool(
        description = "Open a realtime plot window for the specified session. Requires control permission."
    )]
    async fn serial_open_plot(
        &self,
        Parameters(Session { session_id }): Parameters<Session>,
    ) -> CallToolResult {
        self.invoke(Request::OpenPlot { session_id }).await
    }
}

impl ChipDock {
    async fn invoke(&self, request: Request) -> CallToolResult {
        match (self.client)(request).await {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => tool_error(error),
        }
    }
}

fn tool_error(error: ApiError) -> CallToolResult {
    CallToolResult::structured_error(json!({"error": error}))
}

#[tool_handler]
impl ServerHandler for ChipDock {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("chipdock-mcp", env!("CARGO_PKG_VERSION"))).with_instructions("Uses the current user's running ChipDock app. Enable local control in ChipDock before calling serial tools.")
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let server = ChipDock { client: local }
        .serve(rmcp::transport::stdio())
        .await?;
    server.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tool_forwards_explicit_session_and_returns_structured_json() {
        let server = ChipDock {
            client: |request| {
                Box::pin(async move {
                    match request {
                        Request::Status { session_id } => Ok(json!({"sessionId": session_id})),
                        _ => unreachable!(),
                    }
                })
            },
        };
        let reply = server
            .serial_status(Parameters(Session { session_id: 42 }))
            .await;
        assert_eq!(reply.structured_content, Some(json!({"sessionId": 42})));
    }
}
