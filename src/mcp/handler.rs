use serde_json::{Value, json};

use crate::mcp::{JsonRpcRequest, JsonRpcResponse};
use crate::registry::ToolRegistry;

#[derive(Debug, Clone)]
pub struct McpHandler {
    registry: ToolRegistry,
}

impl McpHandler {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub fn handle_request(&self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        match request.id {
            Some(id) => Some(self.handle_method(id, &request.method)),
            None => None,
        }
    }

    fn handle_method(&self, id: Value, method: &str) -> JsonRpcResponse {
        match method {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            _ => JsonRpcResponse::error(Some(id), -32601, format!("Method not found: {method}")),
        }
    }

    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            Some(id),
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mercurius-p",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = self
            .registry
            .list_tools()
            .into_iter()
            .map(tool_to_mcp_json)
            .collect::<Vec<Value>>();

        JsonRpcResponse::success(
            Some(id),
            json!({
                "tools": tools
            }),
        )
    }
}

fn tool_to_mcp_json(tool: &crate::model::ToolDefinition) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for parameter in &tool.parameters {
        properties.insert(
            parameter.name.clone(),
            json!({
                "type": parameter.parameter_type,
                "description": parameter.name
            }),
        );

        if parameter.required {
            required.push(Value::String(parameter.name.clone()));
        }
    }

    json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolDefinition, ToolParameter};
    use serde_json::json;

    fn echo_tool() -> ToolDefinition {
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo a message".to_string(),
            command: "echo".to_string(),
            arguments: vec!["{message}".to_string()],
            parameters: vec![ToolParameter {
                name: "message".to_string(),
                parameter_type: "string".to_string(),
                required: true,
            }],
            timeout_ms: 5000,
        }
    }

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

    fn handler_with_tools(tools: Vec<ToolDefinition>) -> McpHandler {
        let registry = ToolRegistry::new(tools);
        McpHandler::new(registry)
    }

    fn initialize_request() -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2025-06-18"
            })),
        }
    }

    #[test]
    fn handles_initialize_request() {
        let handler = handler_with_tools(vec![]);
        let request = initialize_request();

        match handler.handle_request(request) {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
                assert_eq!(response.id, Some(json!(1)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["protocolVersion"], "2025-06-18");
                        assert_eq!(result["serverInfo"]["name"], "mercurius-p");
                        assert!(result["capabilities"]["tools"].is_object());
                    }
                    None => {
                        panic!("Expected initialize response to contain result");
                    }
                }
            }
            None => {
                panic!("Expected initialize request to produce a response");
            }
        }
    }

    #[test]
    fn handles_tools_list_with_required_parameter() {
        let handler = handler_with_tools(vec![echo_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        match handler.handle_request(request) {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
                assert_eq!(response.id, Some(json!(2)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        let tools = &result["tools"];

                        assert!(tools.is_array());
                        assert_eq!(tools[0]["name"], "echo");
                        assert_eq!(tools[0]["description"], "Echo a message");
                        assert_eq!(tools[0]["inputSchema"]["type"], "object");
                        assert_eq!(
                            tools[0]["inputSchema"]["properties"]["message"]["type"],
                            "string"
                        );
                        assert_eq!(tools[0]["inputSchema"]["required"][0], "message");
                    }
                    None => {
                        panic!("Expected tools/list response to contain result");
                    }
                }
            }
            None => {
                panic!("Expected tools/list request to produce a response");
            }
        }
    }

    #[test]
    fn handles_tools_list_with_no_parameter_tool() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/list".to_string(),
            params: None,
        };

        match handler.handle_request(request) {
            Some(response) => match response.result {
                Some(result) => {
                    assert_eq!(result["tools"][0]["name"], "rustc-version");
                    assert!(result["tools"][0]["inputSchema"]["properties"].is_object());
                    assert!(result["tools"][0]["inputSchema"]["required"].is_array());
                    assert_eq!(
                        result["tools"][0]["inputSchema"]["required"]
                            .as_array()
                            .map(|required| required.len()),
                        Some(0)
                    );
                }
                None => {
                    panic!("Expected tools/list response to contain result");
                }
            },
            None => {
                panic!("Expected tools/list request to produce a response");
            }
        }
    }

    #[test]
    fn returns_method_not_found_for_unknown_method() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(99)),
            method: "unknown/method".to_string(),
            params: None,
        };

        match handler.handle_request(request) {
            Some(response) => {
                assert_eq!(response.id, Some(json!(99)));
                assert!(response.result.is_none());

                match response.error {
                    Some(error) => {
                        assert_eq!(error.code, -32601);
                        assert!(error.message.contains("unknown/method"));
                    }
                    None => {
                        panic!("Expected unknown method response to contain error");
                    }
                }
            }
            None => {
                panic!("Expected unknown method request to produce an error response");
            }
        }
    }

    #[test]
    fn ignores_notification_without_id() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };

        match handler.handle_request(request) {
            Some(response) => {
                panic!("Expected notification to be ignored, but got response: {response:?}");
            }
            None => {}
        }
    }
}
