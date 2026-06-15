use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,

    #[serde(default)]
    pub id: Option<Value>,

    pub method: String,

    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_json_rpc_request_with_params() {
        let raw_json = r#"
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18"
            }
        }
        "#;

        let request_result: Result<JsonRpcRequest, serde_json::Error> =
            serde_json::from_str(raw_json);

        match request_result {
            Ok(request) => {
                assert_eq!(request.jsonrpc, "2.0");
                assert_eq!(request.id, Some(json!(1)));
                assert_eq!(request.method, "initialize");

                match request.params {
                    Some(params) => {
                        assert_eq!(params["protocolVersion"], "2025-06-18");
                    }
                    None => {
                        panic!("Expected params to be present");
                    }
                }
            }
            Err(error) => {
                panic!("Expected request to deserialize successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn deserializes_json_rpc_notification_without_id() {
        let raw_json = r#"
        {
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }
        "#;

        let request_result: Result<JsonRpcRequest, serde_json::Error> =
            serde_json::from_str(raw_json);

        match request_result {
            Ok(request) => {
                assert_eq!(request.jsonrpc, "2.0");
                assert_eq!(request.id, None);
                assert_eq!(request.method, "notifications/initialized");
                assert_eq!(request.params, None);
            }
            Err(error) => {
                panic!("Expected notification to deserialize successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn serializes_success_response() {
        let response = JsonRpcResponse::success(
            Some(json!(1)),
            json!({
                "serverInfo": {
                    "name": "mercurius-p",
                    "version": "0.1.0"
                }
            }),
        );

        let serialized_result = serde_json::to_value(response);

        match serialized_result {
            Ok(value) => {
                assert_eq!(value["jsonrpc"], "2.0");
                assert_eq!(value["id"], 1);
                assert!(value.get("result").is_some());
                assert!(value.get("error").is_none());
                assert_eq!(value["result"]["serverInfo"]["name"], "mercurius-p");
            }
            Err(error) => {
                panic!("Expected response to serialize successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn serializes_error_response() {
        let response = JsonRpcResponse::error(Some(json!(1)), -32601, "Method not found");

        let serialized_result = serde_json::to_value(response);

        match serialized_result {
            Ok(value) => {
                assert_eq!(value["jsonrpc"], "2.0");
                assert_eq!(value["id"], 1);
                assert!(value.get("result").is_none());
                assert!(value.get("error").is_some());
                assert_eq!(value["error"]["code"], -32601);
                assert_eq!(value["error"]["message"], "Method not found");
            }
            Err(error) => {
                panic!("Expected error response to serialize successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn success_response_can_omit_id_for_notification_like_cases() {
        let response = JsonRpcResponse::success(None, json!({ "ok": true }));

        let serialized_result = serde_json::to_value(response);

        match serialized_result {
            Ok(value) => {
                assert!(value.get("id").is_none());
                assert_eq!(value["result"]["ok"], true);
            }
            Err(error) => {
                panic!("Expected response to serialize successfully, but got: {error}");
            }
        }
    }
}
