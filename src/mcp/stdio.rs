use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::{JsonRpcRequest, JsonRpcResponse, McpHandler};

#[derive(Debug, Clone)]
pub struct McpStdioServer {
    handler: McpHandler,
}

impl McpStdioServer {
    pub fn new(handler: McpHandler) -> Self {
        Self { handler }
    }

    pub async fn run(&self) -> Result<(), String> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        loop {
            let line_result = lines.next_line().await;

            let line = match line_result {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    return Err(format!("Failed to read from stdin: {error}"));
                }
            };

            if let Some(response_line) = self.handle_line(&line).await {
                if let Err(error) = stdout.write_all(response_line.as_bytes()).await {
                    return Err(format!("Failed to write response to stdout: {error}"));
                }

                if let Err(error) = stdout.write_all(b"\n").await {
                    return Err(format!("Failed to write newline to stdout: {error}"));
                }

                if let Err(error) = stdout.flush().await {
                    return Err(format!("Failed to flush stdout: {error}"));
                }
            }
        }

        Ok(())
    }

    pub async fn handle_line(&self, line: &str) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }

        let request_result: Result<JsonRpcRequest, serde_json::Error> = serde_json::from_str(line);

        let response = match request_result {
            Ok(request) => self.handler.handle_request(request).await,
            Err(error) => Some(JsonRpcResponse::error(
                None,
                -32700,
                format!("Parse error: {error}"),
            )),
        };

        match response {
            Some(response) => match serde_json::to_string(&response) {
                Ok(serialized) => Some(serialized),
                Err(error) => {
                    let fallback = JsonRpcResponse::error(
                        None,
                        -32603,
                        format!("Failed to serialize response: {error}"),
                    );

                    serde_json::to_string(&fallback).ok()
                }
            },
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ToolDefinition;
    use crate::registry::ToolRegistry;
    use crate::service::ToolExecutionService;
    use serde_json::Value;

    fn no_parameter_tool() -> ToolDefinition {
        ToolDefinition {
            name: "rustc-version".to_string(),
            description: "Print rustc version".to_string(),
            command: "rustc".to_string(),
            arguments: vec!["--version".to_string()],
            parameters: vec![],
            timeout_ms: 5000,
        }
    }

    fn test_server() -> McpStdioServer {
        let registry = ToolRegistry::new(vec![no_parameter_tool()]);
        let service = ToolExecutionService::new(registry);
        let handler = McpHandler::new(service, "tools.json");

        McpStdioServer::new(handler)
    }

    #[tokio::test]
    async fn handles_initialize_line() {
        let server = test_server();

        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;

        match server.handle_line(line).await {
            Some(response_line) => {
                let parsed_result: Result<Value, serde_json::Error> =
                    serde_json::from_str(&response_line);

                match parsed_result {
                    Ok(value) => {
                        assert_eq!(value["jsonrpc"], "2.0");
                        assert_eq!(value["id"], 1);
                        assert_eq!(value["result"]["serverInfo"]["name"], "mercurius-p");
                    }
                    Err(error) => {
                        panic!("Expected response line to be valid JSON, but got: {error}");
                    }
                }
            }
            None => {
                panic!("Expected initialize line to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn handles_tools_list_line() {
        let server = test_server();

        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

        match server.handle_line(line).await {
            Some(response_line) => {
                let parsed_result: Result<Value, serde_json::Error> =
                    serde_json::from_str(&response_line);

                match parsed_result {
                    Ok(value) => {
                        assert_eq!(value["jsonrpc"], "2.0");
                        assert_eq!(value["id"], 2);
                        assert_eq!(value["result"]["tools"][0]["name"], "rustc-version");
                    }
                    Err(error) => {
                        panic!("Expected response line to be valid JSON, but got: {error}");
                    }
                }
            }
            None => {
                panic!("Expected tools/list line to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn ignores_notification_line() {
        let server = test_server();

        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

        if let Some(response_line) = server.handle_line(line).await {
            panic!("Expected notification to be ignored, but got: {response_line}");
        }
    }

    #[tokio::test]
    async fn ignores_blank_line() {
        let server = test_server();

        if let Some(response_line) = server.handle_line("   ").await {
            panic!("Expected blank line to be ignored, but got: {response_line}");
        }
    }

    #[tokio::test]
    async fn returns_parse_error_for_invalid_json() {
        let server = test_server();

        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize""#;

        match server.handle_line(line).await {
            Some(response_line) => {
                let parsed_result: Result<Value, serde_json::Error> =
                    serde_json::from_str(&response_line);

                match parsed_result {
                    Ok(value) => {
                        assert_eq!(value["jsonrpc"], "2.0");
                        assert_eq!(value["error"]["code"], -32700);
                        assert!(
                            value["error"]["message"]
                                .as_str()
                                .map(|message| message.contains("Parse error"))
                                .unwrap_or(false)
                        );
                    }
                    Err(error) => {
                        panic!("Expected parse error response to be valid JSON, but got: {error}");
                    }
                }
            }
            None => {
                panic!("Expected invalid JSON to produce a parse error response");
            }
        }
    }
}
