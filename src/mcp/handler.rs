use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::mcp::{JsonRpcRequest, JsonRpcResponse};
use crate::model::ExecutionResult;
use crate::service::ToolExecutionService;

#[derive(Debug, Clone)]
pub struct McpHandler {
    service: ToolExecutionService,
    config_path: PathBuf,
}

impl McpHandler {
    pub fn new(service: ToolExecutionService, config_path: impl Into<PathBuf>) -> Self {
        Self {
            service,
            config_path: config_path.into(),
        }
    }

    pub async fn handle_request(&self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        match request.id {
            Some(id) => Some(
                self.handle_method(id, &request.method, request.params)
                    .await,
            ),
            None => None,
        }
    }

    async fn handle_method(
        &self,
        id: Value,
        method: &str,
        params: Option<Value>,
    ) -> JsonRpcResponse {
        match method {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, params).await,
            "resources/list" => self.handle_resources_list(id),
            "resources/read" => self.handle_resources_read(id, params),
            _ => JsonRpcResponse::error(Some(id), -32601, format!("Method not found: {method}")),
        }
    }

    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            Some(id),
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "mercurius-p",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_resources_list(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(
            Some(id),
            json!({
                "resources": [
                    {
                        "uri": "mercurius-p://tools/config",
                        "name": "Tool configuration",
                        "description": "The active mercurius-p tool configuration file",
                        "mimeType": "application/json"
                    }
                ]
            }),
        )
    }

    fn handle_resources_read(&self, id: Value, params: Option<Value>) -> JsonRpcResponse {
        let uri = match parse_resource_uri(params) {
            Ok(uri) => uri,
            Err(error) => {
                return JsonRpcResponse::error(Some(id), -32602, error);
            }
        };

        match uri.as_str() {
            "mercurius-p://tools/config" => self.read_tools_config_resource(id),
            _ => JsonRpcResponse::error(Some(id), -32602, format!("Unknown resource URI: {uri}")),
        }
    }

    fn read_tools_config_resource(&self, id: Value) -> JsonRpcResponse {
        let contents = match std::fs::read_to_string(&self.config_path) {
            Ok(contents) => contents,
            Err(error) => {
                return JsonRpcResponse::error(
                    Some(id),
                    -32603,
                    format!("Failed to read tool configuration resource: {error}"),
                );
            }
        };

        JsonRpcResponse::success(
            Some(id),
            json!({
                "contents": [
                    {
                        "uri": "mercurius-p://tools/config",
                        "mimeType": "application/json",
                        "text": contents
                    }
                ]
            }),
        )
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = self
            .service
            .registry()
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

    async fn handle_tools_call(&self, id: Value, params: Option<Value>) -> JsonRpcResponse {
        let call_params = match parse_tools_call_params(params) {
            Ok(call_params) => call_params,
            Err(error) => {
                return JsonRpcResponse::error(Some(id), -32602, error);
            }
        };

        match self
            .service
            .execute_tool(&call_params.name, &call_params.arguments)
            .await
        {
            Ok(result) => JsonRpcResponse::success(Some(id), execution_result_to_mcp_json(result)),
            Err(error) => JsonRpcResponse::error(Some(id), -32603, error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolsCallParams {
    name: String,
    arguments: HashMap<String, String>,
}

fn parse_resource_uri(params: Option<Value>) -> Result<String, String> {
    let params = match params {
        Some(params) => params,
        None => {
            return Err("resources/read requires params".to_string());
        }
    };

    match params.get("uri") {
        Some(Value::String(uri)) if !uri.trim().is_empty() => Ok(uri.clone()),
        Some(_) => Err("resources/read param 'uri' must be a non-empty string".to_string()),
        None => Err("resources/read missing required param 'uri'".to_string()),
    }
}

fn parse_tools_call_params(params: Option<Value>) -> Result<ToolsCallParams, String> {
    let params = match params {
        Some(params) => params,
        None => {
            return Err("tools/call requires params".to_string());
        }
    };

    let name = match params.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => name.clone(),
        Some(_) => {
            return Err("tools/call param 'name' must be a non-empty string".to_string());
        }
        None => {
            return Err("tools/call missing required param 'name'".to_string());
        }
    };

    let arguments = match params.get("arguments") {
        Some(Value::Object(arguments_object)) => value_object_to_string_map(arguments_object)?,
        Some(_) => {
            return Err("tools/call param 'arguments' must be an object".to_string());
        }
        None => HashMap::new(),
    };

    Ok(ToolsCallParams { name, arguments })
}

fn value_object_to_string_map(
    object: &serde_json::Map<String, Value>,
) -> Result<HashMap<String, String>, String> {
    let mut arguments = HashMap::new();

    for (key, value) in object {
        match value {
            Value::String(string_value) => {
                arguments.insert(key.clone(), string_value.clone());
            }
            Value::Number(number_value) => {
                arguments.insert(key.clone(), number_value.to_string());
            }
            Value::Bool(bool_value) => {
                arguments.insert(key.clone(), bool_value.to_string());
            }
            Value::Null => {
                return Err(format!("Argument '{key}' cannot be null"));
            }
            Value::Array(_) | Value::Object(_) => {
                return Err(format!(
                    "Argument '{key}' must be a string, number, or boolean"
                ));
            }
        }
    }

    Ok(arguments)
}

fn execution_result_to_mcp_json(result: ExecutionResult) -> Value {
    let is_error = result.timed_out || result.exit_code != Some(0);

    let mut text = String::new();

    text.push_str(&format!("exit_code: {:?}\n", result.exit_code));
    text.push_str(&format!("timed_out: {}\n", result.timed_out));
    text.push_str(&format!("duration_ms: {}\n", result.duration_ms));

    if !result.stdout.trim().is_empty() {
        text.push_str("\nstdout:\n");
        text.push_str(&result.stdout);
    }

    if !result.stderr.trim().is_empty() {
        text.push_str("\nstderr:\n");
        text.push_str(&result.stderr);
    }

    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": is_error
    })
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
    use crate::registry::ToolRegistry;
    use crate::service::ToolExecutionService;
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

    fn echo_like_required_parameter_tool() -> ToolDefinition {
        ToolDefinition {
            name: "echo-like".to_string(),
            description: "Requires a message parameter".to_string(),
            command: "rustc".to_string(),
            arguments: vec!["--version".to_string()],
            parameters: vec![ToolParameter {
                name: "message".to_string(),
                parameter_type: "string".to_string(),
                required: true,
            }],
            timeout_ms: 5000,
        }
    }

    fn handler_with_tools(tools: Vec<ToolDefinition>) -> McpHandler {
        let registry = ToolRegistry::new(tools);
        let service = ToolExecutionService::new(registry);

        McpHandler::new(service, "tools.json")
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

    #[tokio::test]
    async fn handles_initialize_request() {
        let handler = handler_with_tools(vec![]);
        let request = initialize_request();

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
                assert_eq!(response.id, Some(json!(1)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["protocolVersion"], "2025-06-18");
                        assert_eq!(result["serverInfo"]["name"], "mercurius-p");
                        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
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

    #[tokio::test]
    async fn handles_tools_list_with_required_parameter() {
        let handler = handler_with_tools(vec![echo_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        match handler.handle_request(request).await {
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

    #[tokio::test]
    async fn handles_tools_list_with_no_parameter_tool() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/list".to_string(),
            params: None,
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
                assert_eq!(response.id, Some(json!(3)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["tools"][0]["name"], "rustc-version");
                        assert!(result["tools"][0]["inputSchema"]["properties"].is_object());
                        assert!(result["tools"][0]["inputSchema"]["required"].is_array());

                        match result["tools"][0]["inputSchema"]["required"].as_array() {
                            Some(required) => {
                                assert_eq!(required.len(), 0);
                            }
                            None => {
                                panic!("Expected required field to be an array");
                            }
                        }
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

    #[tokio::test]
    async fn handles_tools_call_for_no_parameter_tool() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "rustc-version",
                "arguments": {}
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
                assert_eq!(response.id, Some(json!(4)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["isError"], false);
                        assert_eq!(result["content"][0]["type"], "text");

                        match result["content"][0]["text"].as_str() {
                            Some(text) => {
                                assert!(text.contains("rustc"));
                            }
                            None => {
                                panic!("Expected MCP content text to be a string");
                            }
                        }
                    }
                    None => {
                        panic!("Expected tools/call response to contain result");
                    }
                }
            }
            None => {
                panic!("Expected tools/call request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn tools_call_returns_invalid_params_when_params_missing() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(5)),
            method: "tools/call".to_string(),
            params: None,
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert!(response.result.is_none());

                match response.error {
                    Some(error) => {
                        assert_eq!(error.code, -32602);
                        assert!(error.message.contains("requires params"));
                    }
                    None => {
                        panic!("Expected tools/call without params to return error");
                    }
                }
            }
            None => {
                panic!("Expected tools/call request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn tools_call_returns_invalid_params_when_name_missing() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(6)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "arguments": {}
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert!(response.result.is_none());

                match response.error {
                    Some(error) => {
                        assert_eq!(error.code, -32602);
                        assert!(error.message.contains("name"));
                    }
                    None => {
                        panic!("Expected tools/call without name to return error");
                    }
                }
            }
            None => {
                panic!("Expected tools/call request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn tools_call_returns_execution_error_for_unknown_tool() {
        let handler = handler_with_tools(vec![no_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(7)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "missing-tool",
                "arguments": {}
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert!(response.result.is_none());

                match response.error {
                    Some(error) => {
                        assert_eq!(error.code, -32603);
                        assert!(error.message.contains("missing-tool"));
                    }
                    None => {
                        panic!("Expected unknown tool call to return error");
                    }
                }
            }
            None => {
                panic!("Expected tools/call request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn tools_call_returns_execution_error_for_missing_required_parameter() {
        let handler = handler_with_tools(vec![echo_like_required_parameter_tool()]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(8)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "echo-like",
                "arguments": {}
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert!(response.result.is_none());

                match response.error {
                    Some(error) => {
                        assert_eq!(error.code, -32603);
                        assert!(error.message.contains("Missing required parameter"));
                    }
                    None => {
                        panic!("Expected missing required parameter to return error");
                    }
                }
            }
            None => {
                panic!("Expected tools/call request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn returns_method_not_found_for_unknown_method() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(99)),
            method: "unknown/method".to_string(),
            params: None,
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.jsonrpc, "2.0");
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

    #[tokio::test]
    async fn ignores_notification_without_id() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };

        if let Some(response) = handler.handle_request(request).await {
            panic!("Expected notification to be ignored, but got response: {response:?}");
        }
    }

    #[tokio::test]
    async fn handles_resources_list() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(9)),
            method: "resources/list".to_string(),
            params: None,
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.id, Some(json!(9)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["resources"][0]["uri"], "mercurius-p://tools/config");
                        assert_eq!(result["resources"][0]["mimeType"], "application/json");
                    }
                    None => {
                        panic!("Expected resources/list response to contain result");
                    }
                }
            }
            None => {
                panic!("Expected resources/list request to produce a response");
            }
        }
    }
    #[tokio::test]
    async fn handles_resources_read_for_tools_config() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(10)),
            method: "resources/read".to_string(),
            params: Some(json!({
                "uri": "mercurius-p://tools/config"
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => {
                assert_eq!(response.id, Some(json!(10)));
                assert!(response.error.is_none());

                match response.result {
                    Some(result) => {
                        assert_eq!(result["contents"][0]["uri"], "mercurius-p://tools/config");
                        assert_eq!(result["contents"][0]["mimeType"], "application/json");

                        match result["contents"][0]["text"].as_str() {
                            Some(text) => {
                                assert!(text.contains("echo"));
                            }
                            None => {
                                panic!("Expected resource text to be a string");
                            }
                        }
                    }
                    None => {
                        panic!("Expected resources/read response to contain result");
                    }
                }
            }
            None => {
                panic!("Expected resources/read request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn resources_read_returns_invalid_params_when_uri_missing() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(11)),
            method: "resources/read".to_string(),
            params: Some(json!({})),
        };

        match handler.handle_request(request).await {
            Some(response) => match response.error {
                Some(error) => {
                    assert_eq!(error.code, -32602);
                    assert!(error.message.contains("uri"));
                }
                None => {
                    panic!("Expected missing resource URI to return error");
                }
            },
            None => {
                panic!("Expected resources/read request to produce a response");
            }
        }
    }

    #[tokio::test]
    async fn resources_read_returns_invalid_params_for_unknown_uri() {
        let handler = handler_with_tools(vec![]);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(12)),
            method: "resources/read".to_string(),
            params: Some(json!({
                "uri": "mercurius-p://unknown"
            })),
        };

        match handler.handle_request(request).await {
            Some(response) => match response.error {
                Some(error) => {
                    assert_eq!(error.code, -32602);
                    assert!(error.message.contains("Unknown resource URI"));
                }
                None => {
                    panic!("Expected unknown resource URI to return error");
                }
            },
            None => {
                panic!("Expected resources/read request to produce a response");
            }
        }
    }
}
