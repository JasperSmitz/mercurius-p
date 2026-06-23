use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<String>,
    pub parameters: Vec<ToolParameter>,

    #[serde(default)]
    pub timeout_ms: Option<u64>,

    #[serde(default)]
    pub read_only: bool,

    #[serde(default)]
    pub category: Option<String>,

    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolParameter {
    pub name: String,

    #[serde(rename = "type")]
    pub parameter_type: ParameterType,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub default: Option<serde_json::Value>,

    #[serde(default)]
    pub allowed_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Integer,
    Boolean,
    Path,
    Enum,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub allowed_paths: Vec<PathBuf>,
    pub blocked_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToolCall {
    pub command: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolValidationError {
    MissingParameter(String),
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
    },
    InvalidEnumValue {
        parameter: String,
        value: String,
        allowed_values: Vec<String>,
    },
    PathNotAllowed(String),
    UnknownPlaceholder(String),
}

pub fn validate_tool_call(
    tool: &ToolDefinition,
    _input: &serde_json::Value,
    _policy: &SecurityPolicy,
) -> Result<ResolvedToolCall, ToolValidationError> {
    Ok(ResolvedToolCall {
        command: tool.command.clone(),
        arguments: tool.arguments.clone(),
        working_directory: tool.working_directory.as_ref().map(PathBuf::from),
        timeout_ms: tool.timeout_ms.unwrap_or(5000),
    })
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
                assert_eq!(tool.parameters[0].parameter_type, ParameterType::String);
                assert!(tool.parameters[0].required);
                assert_eq!(tool.timeout_ms, Some(5000));
            }
            Err(error) => {
                panic!("Expected tool definition to deserialize successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn deserializes_default_metadata_fields() {
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
            ]
        }
        "#;

        let tool: ToolDefinition = serde_json::from_str(json).unwrap();

        assert_eq!(tool.timeout_ms, None);
        assert!(!tool.read_only);
        assert_eq!(tool.category, None);
        assert_eq!(tool.working_directory, None);
    }

    #[test]
    fn deserializes_extended_metadata_fields() {
        let json = r#"
        {
            "name": "git-status",
            "description": "Show git status",
            "command": "git",
            "arguments": ["-C", "{path}", "status", "--short"],
            "parameters": [
                {
                    "name": "path",
                    "type": "path",
                    "required": true
                }
            ],
            "timeout_ms": 5000,
            "read_only": true,
            "category": "git",
            "working_directory": "{path}"
        }
        "#;

        let tool: ToolDefinition = serde_json::from_str(json).unwrap();

        assert_eq!(tool.name, "git-status");
        assert_eq!(tool.parameters[0].parameter_type, ParameterType::Path);
        assert_eq!(tool.timeout_ms, Some(5000));
        assert!(tool.read_only);
        assert_eq!(tool.category, Some("git".to_string()));
        assert_eq!(tool.working_directory, Some("{path}".to_string()));
    }

    #[test]
    fn validate_tool_call_returns_basic_resolved_call_for_now() {
        let tool = ToolDefinition {
            name: "system-load".to_string(),
            description: "Show system load".to_string(),
            command: "uptime".to_string(),
            arguments: vec![],
            parameters: vec![],
            timeout_ms: Some(3000),
            read_only: true,
            category: Some("system".to_string()),
            working_directory: None,
        };

        let policy = SecurityPolicy {
            allowed_paths: vec![],
            blocked_paths: vec![],
        };

        let input = serde_json::json!({});

        let resolved = validate_tool_call(&tool, &input, &policy).unwrap();

        assert_eq!(resolved.command, "uptime");
        assert_eq!(resolved.arguments, Vec::<String>::new());
        assert_eq!(resolved.timeout_ms, 3000);
        assert_eq!(resolved.working_directory, None);
    }
}