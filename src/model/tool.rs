#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<String>,
    pub parameters: Vec<ToolParameter>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolParameter {
    pub name: String,

    #[serde(rename = "type")]
    pub parameter_type: String,

    pub required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_tool_definition() {
        let json = r#"
        {
            "name": "echo",
            "description": "Echo a message",
            "command": "echo",
            "arguments": ["{message}"],
            "parameters": [
                {
                    "name": "message",
                    "type": "string",
                    "required": true
                }
            ],
            "timeout_ms": 5000
        }
        "#;

        let tool_result: Result<ToolDefinition, serde_json::Error> = serde_json::from_str(json);

        match tool_result {
            Ok(tool) => {
                assert_eq!(tool.name, "echo");
                assert_eq!(tool.parameters.len(), 1);
                assert_eq!(tool.parameters[0].parameter_type, "string");
                assert!(tool.parameters[0].required);
                assert_eq!(tool.timeout_ms, 5000);
            }
            Err(error) => {
                panic!("Expected tool definition to deserialize successfully, but got: {error}");
            }
        }
    }
}
