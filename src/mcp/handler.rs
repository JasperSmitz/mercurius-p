use serde_json::json;

use crate::mcp::{JsonRpcRequest, JsonRpcResponse};

pub fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    match request.id {
        Some(id) => Some(handle_method(id, &request.method)),
        None => None,
    }
}

fn handle_method(id: serde_json::Value, method: &str) -> JsonRpcResponse {
    match method {
        "initialize" => handle_initialize(id),
        _ => JsonRpcResponse::error(Some(id), -32601, format!("Method not found: {method}")),
    }
}

fn handle_initialize(id: serde_json::Value) -> JsonRpcResponse {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let request = initialize_request();

        match handle_request(request) {
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

    #[test]
    fn returns_method_not_found_for_unknown_method() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(99)),
            method: "unknown/method".to_string(),
            params: None,
        };

        match handle_request(request) {
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

    #[test]
    fn ignores_notification_without_id() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };

        if let Some(response) = handle_request(request) {
            panic!("Expected notification to be ignored, but got response: {response:?}");
        }
    }
}
