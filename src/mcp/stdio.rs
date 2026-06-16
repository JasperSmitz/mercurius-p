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

            match self.handle_line(&line).await {
                Some(response_line) => {
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
                None => {}
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

                    match serde_json::to_string(&fallback) {
                        Ok(serialized_fallback) => Some(serialized_fallback),
                        Err(_) => None,
                    }
                }
            },
            None => None,
        }
    }
}