use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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
    InvalidInput(String),
    MissingParameter(String),
    UnknownParameter(String),
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
    },
    InvalidEnumValue {
        parameter: String,
        value: String,
        allowed_values: Vec<String>,
    },
    InvalidEnumConfiguration(String),
    PathNotAllowed(String),
    UnknownPlaceholder(String),
}

pub fn validate_tool_call(
    tool: &ToolDefinition,
    input: &serde_json::Value,
    policy: &SecurityPolicy,
) -> Result<ResolvedToolCall, ToolValidationError> {
    let input = input
        .as_object()
        .ok_or_else(|| ToolValidationError::InvalidInput("input must be an object".to_string()))?;

    validate_known_parameters(tool, input)?;

    let resolved_parameters = resolve_parameters(tool, input)?;
    validate_resolved_path_parameters(tool, &resolved_parameters, policy)?;
    let resolved_arguments = tool
        .arguments
        .iter()
        .map(|argument| resolve_placeholders(argument, &resolved_parameters))
        .collect::<Result<Vec<_>, _>>()?;

    let resolved_working_directory = tool
        .working_directory
        .as_ref()
        .map(|working_directory| resolve_placeholders(working_directory, &resolved_parameters))
        .transpose()?;

    if let Some(working_directory) = resolved_working_directory.as_ref() {
        validate_path_allowed(working_directory, policy)?;
    }

    Ok(ResolvedToolCall {
        command: tool.command.clone(),
        arguments: resolved_arguments,
        working_directory: resolved_working_directory.map(PathBuf::from),
        timeout_ms: tool.timeout_ms.unwrap_or(5000),
    })
}

fn validate_known_parameters(
    tool: &ToolDefinition,
    input: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ToolValidationError> {
    let declared_parameters = tool
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<HashSet<_>>();

    for parameter_name in input.keys() {
        if !declared_parameters.contains(parameter_name.as_str()) {
            return Err(ToolValidationError::UnknownParameter(
                parameter_name.to_string(),
            ));
        }
    }

    Ok(())
}

fn resolve_parameters(
    tool: &ToolDefinition,
    input: &serde_json::Map<String, serde_json::Value>,
) -> Result<HashMap<String, String>, ToolValidationError> {
    let mut resolved_parameters = HashMap::new();

    for parameter in &tool.parameters {
        let value = if let Some(value) = input.get(&parameter.name).or(parameter.default.as_ref()) {
            value
        } else if parameter.required {
            return Err(ToolValidationError::MissingParameter(
                parameter.name.clone(),
            ));
        } else {
            continue;
        };

        let resolved_value = validate_parameter_value(parameter, value)?;
        resolved_parameters.insert(parameter.name.clone(), resolved_value);
    }

    Ok(resolved_parameters)
}

fn validate_parameter_value(
    parameter: &ToolParameter,
    value: &serde_json::Value,
) -> Result<String, ToolValidationError> {
    match &parameter.parameter_type {
        ParameterType::String | ParameterType::Path => value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| invalid_parameter_type(parameter)),
        ParameterType::Integer => match value {
            serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => {
                Ok(number.to_string())
            }
            _ => Err(invalid_parameter_type(parameter)),
        },
        ParameterType::Boolean => value
            .as_bool()
            .map(|value| value.to_string())
            .ok_or_else(|| invalid_parameter_type(parameter)),
        ParameterType::Enum => {
            let value = value
                .as_str()
                .ok_or_else(|| invalid_parameter_type(parameter))?;
            let allowed_values = parameter
                .allowed_values
                .as_ref()
                .filter(|values| !values.is_empty());

            match allowed_values {
                Some(allowed_values) if allowed_values.iter().any(|allowed| allowed == value) => {
                    Ok(value.to_string())
                }
                Some(allowed_values) => Err(ToolValidationError::InvalidEnumValue {
                    parameter: parameter.name.clone(),
                    value: value.to_string(),
                    allowed_values: allowed_values.clone(),
                }),
                None => Err(ToolValidationError::InvalidEnumConfiguration(
                    parameter.name.clone(),
                )),
            }
        }
    }
}

fn validate_resolved_path_parameters(
    tool: &ToolDefinition,
    resolved_parameters: &HashMap<String, String>,
    policy: &SecurityPolicy,
) -> Result<(), ToolValidationError> {
    for parameter in &tool.parameters {
        if parameter.parameter_type == ParameterType::Path
            && let Some(path) = resolved_parameters.get(&parameter.name)
        {
            validate_path_allowed(path, policy)?;
        }
    }

    Ok(())
}

fn validate_path_allowed(path: &str, policy: &SecurityPolicy) -> Result<(), ToolValidationError> {
    if path.is_empty() || path.contains('\0') {
        return Err(ToolValidationError::PathNotAllowed(path.to_string()));
    }

    let requested_path = PathBuf::from(path);
    if !requested_path.is_absolute() {
        return Err(ToolValidationError::PathNotAllowed(path.to_string()));
    }

    let Some(canonical_path) = canonicalize_path_or_existing_parent(&requested_path) else {
        return Err(ToolValidationError::PathNotAllowed(path.to_string()));
    };

    let blocked_roots = canonicalize_existing_roots(&policy.blocked_paths);
    if blocked_roots
        .iter()
        .any(|blocked_root| canonical_path.starts_with(blocked_root))
    {
        return Err(ToolValidationError::PathNotAllowed(path.to_string()));
    }

    if policy.allowed_paths.is_empty() {
        return Ok(());
    }

    let allowed_roots = canonicalize_existing_roots(&policy.allowed_paths);
    if allowed_roots
        .iter()
        .any(|allowed_root| canonical_path.starts_with(allowed_root))
    {
        Ok(())
    } else {
        Err(ToolValidationError::PathNotAllowed(path.to_string()))
    }
}

fn canonicalize_existing_roots(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|path| fs::canonicalize(path).ok())
        .collect()
}

fn canonicalize_path_or_existing_parent(requested_path: &Path) -> Option<PathBuf> {
    let mut candidate = Some(requested_path);

    while let Some(path) = candidate {
        if let Ok(canonical_path) = fs::canonicalize(path) {
            if path != requested_path && path.parent().is_none() {
                return None;
            }

            return Some(canonical_path);
        }

        candidate = path.parent();
    }

    None
}

fn invalid_parameter_type(parameter: &ToolParameter) -> ToolValidationError {
    ToolValidationError::InvalidParameterType {
        parameter: parameter.name.clone(),
        expected: parameter.parameter_type.clone(),
    }
}

fn resolve_placeholders(
    template: &str,
    resolved_parameters: &HashMap<String, String>,
) -> Result<String, ToolValidationError> {
    let mut output = String::new();
    let mut remaining = template;

    while let Some(open_index) = remaining.find('{') {
        output.push_str(&remaining[..open_index]);

        let after_open = &remaining[open_index + 1..];

        let Some(close_index) = after_open.find('}') else {
            output.push_str(&remaining[open_index..]);
            return Ok(output);
        };

        let parameter_name = &after_open[..close_index];
        let value = resolved_parameters
            .get(parameter_name)
            .ok_or_else(|| ToolValidationError::UnknownPlaceholder(parameter_name.to_string()))?;

        output.push_str(value);
        remaining = &after_open[close_index + 1..];
    }

    output.push_str(remaining);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn parameter(name: &str, parameter_type: ParameterType) -> ToolParameter {
        ToolParameter {
            name: name.to_string(),
            parameter_type,
            required: false,
            default: None,
            allowed_values: None,
        }
    }

    fn required_parameter(name: &str, parameter_type: ParameterType) -> ToolParameter {
        ToolParameter {
            required: true,
            ..parameter(name, parameter_type)
        }
    }

    fn tool_with_parameters(
        parameters: Vec<ToolParameter>,
        arguments: Vec<&str>,
    ) -> ToolDefinition {
        ToolDefinition {
            name: "test-tool".to_string(),
            description: "Test tool".to_string(),
            command: "test-command".to_string(),
            arguments: arguments.into_iter().map(str::to_string).collect(),
            parameters,
            timeout_ms: Some(3000),
            read_only: true,
            category: Some("test".to_string()),
            working_directory: None,
        }
    }

    fn empty_policy() -> SecurityPolicy {
        SecurityPolicy {
            allowed_paths: vec![],
            blocked_paths: vec![],
        }
    }

    fn policy(allowed_paths: Vec<PathBuf>, blocked_paths: Vec<PathBuf>) -> SecurityPolicy {
        SecurityPolicy {
            allowed_paths,
            blocked_paths,
        }
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "mercurius-p-tool-model-{}-{nanos}-{name}",
            std::process::id()
        ))
    }

    fn create_temp_dir(name: &str) -> PathBuf {
        let path = unique_temp_path(name);
        std::fs::create_dir_all(&path).expect("temporary test directory should be created");
        path
    }

    fn path_tool(arguments: Vec<&str>) -> ToolDefinition {
        tool_with_parameters(
            vec![required_parameter("path", ParameterType::Path)],
            arguments,
        )
    }

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
            arguments: vec!["--pretty".to_string()],
            parameters: vec![],
            timeout_ms: Some(3000),
            read_only: true,
            category: Some("system".to_string()),
            working_directory: Some("/tmp".to_string()),
        };

        let policy = SecurityPolicy {
            allowed_paths: vec![],
            blocked_paths: vec![],
        };

        let input = serde_json::json!({});

        let resolved = validate_tool_call(&tool, &input, &policy).unwrap();

        assert_eq!(resolved.command, "uptime");
        assert_eq!(resolved.arguments, vec!["--pretty"]);
        assert_eq!(resolved.timeout_ms, 3000);
        assert_eq!(resolved.working_directory, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn validate_tool_call_uses_default_timeout_when_missing() {
        let tool = ToolDefinition {
            name: "system-load".to_string(),
            description: "Show system load".to_string(),
            command: "uptime".to_string(),
            arguments: vec![],
            parameters: vec![],
            timeout_ms: None,
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

        assert_eq!(resolved.timeout_ms, 5000);
    }

    #[test]
    fn validate_tool_call_rejects_non_object_input() {
        let tool = tool_with_parameters(vec![], vec![]);
        let input = serde_json::json!("not an object");

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::InvalidInput("input must be an object".to_string())
        );
    }

    #[test]
    fn missing_required_parameter_returns_error() {
        let tool = tool_with_parameters(
            vec![required_parameter("message", ParameterType::String)],
            vec!["{message}"],
        );
        let input = serde_json::json!({});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::MissingParameter("message".to_string())
        );
    }

    #[test]
    fn optional_missing_parameter_without_default_is_allowed() {
        let tool = tool_with_parameters(
            vec![parameter("message", ParameterType::String)],
            vec!["literal"],
        );
        let input = serde_json::json!({});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(resolved.arguments, vec!["literal"]);
    }

    #[test]
    fn missing_parameter_with_default_is_resolved_and_substituted() {
        let mut message = parameter("message", ParameterType::String);
        message.default = Some(serde_json::json!("hello"));
        let tool = tool_with_parameters(vec![message], vec!["{message}"]);
        let input = serde_json::json!({});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(resolved.arguments, vec!["hello"]);
    }

    #[test]
    fn string_parameter_accepts_strings_and_rejects_non_strings() {
        let tool = tool_with_parameters(
            vec![required_parameter("message", ParameterType::String)],
            vec!["{message}"],
        );

        let resolved = validate_tool_call(
            &tool,
            &serde_json::json!({"message": "hello"}),
            &empty_policy(),
        )
        .unwrap();
        assert_eq!(resolved.arguments, vec!["hello"]);

        let error = validate_tool_call(&tool, &serde_json::json!({"message": 1}), &empty_policy())
            .unwrap_err();
        assert_eq!(
            error,
            ToolValidationError::InvalidParameterType {
                parameter: "message".to_string(),
                expected: ParameterType::String,
            }
        );
    }

    #[test]
    fn integer_parameter_accepts_integers_and_rejects_floats_and_strings() {
        let tool = tool_with_parameters(
            vec![required_parameter("min_size_mb", ParameterType::Integer)],
            vec!["+{min_size_mb}M"],
        );

        let resolved = validate_tool_call(
            &tool,
            &serde_json::json!({"min_size_mb": 500}),
            &empty_policy(),
        )
        .unwrap();
        assert_eq!(resolved.arguments, vec!["+500M"]);

        for value in [serde_json::json!(1.5), serde_json::json!("500")] {
            let error = validate_tool_call(
                &tool,
                &serde_json::json!({"min_size_mb": value}),
                &empty_policy(),
            )
            .unwrap_err();
            assert_eq!(
                error,
                ToolValidationError::InvalidParameterType {
                    parameter: "min_size_mb".to_string(),
                    expected: ParameterType::Integer,
                }
            );
        }
    }

    #[test]
    fn boolean_parameter_accepts_booleans_and_rejects_strings() {
        let tool = tool_with_parameters(
            vec![required_parameter("enabled", ParameterType::Boolean)],
            vec!["--flag={enabled}"],
        );

        let resolved = validate_tool_call(
            &tool,
            &serde_json::json!({"enabled": true}),
            &empty_policy(),
        )
        .unwrap();
        assert_eq!(resolved.arguments, vec!["--flag=true"]);

        let error = validate_tool_call(
            &tool,
            &serde_json::json!({"enabled": "true"}),
            &empty_policy(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ToolValidationError::InvalidParameterType {
                parameter: "enabled".to_string(),
                expected: ParameterType::Boolean,
            }
        );
    }

    #[test]
    fn path_parameter_accepts_strings_for_now() {
        let tool = tool_with_parameters(
            vec![required_parameter("path", ParameterType::Path)],
            vec!["{path}"],
        );

        let resolved =
            validate_tool_call(&tool, &serde_json::json!({"path": "/tmp"}), &empty_policy())
                .unwrap();

        assert_eq!(resolved.arguments, vec!["/tmp"]);
    }

    #[test]
    fn enum_parameter_accepts_allowed_value() {
        let mut format = required_parameter("format", ParameterType::Enum);
        format.allowed_values = Some(vec!["json".to_string(), "text".to_string()]);
        let tool = tool_with_parameters(vec![format], vec!["--format={format}"]);

        let resolved = validate_tool_call(
            &tool,
            &serde_json::json!({"format": "json"}),
            &empty_policy(),
        )
        .unwrap();

        assert_eq!(resolved.arguments, vec!["--format=json"]);
    }

    #[test]
    fn enum_parameter_rejects_disallowed_value() {
        let mut format = required_parameter("format", ParameterType::Enum);
        format.allowed_values = Some(vec!["json".to_string(), "text".to_string()]);
        let tool = tool_with_parameters(vec![format], vec!["--format={format}"]);

        let error = validate_tool_call(
            &tool,
            &serde_json::json!({"format": "xml"}),
            &empty_policy(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::InvalidEnumValue {
                parameter: "format".to_string(),
                value: "xml".to_string(),
                allowed_values: vec!["json".to_string(), "text".to_string()],
            }
        );
    }

    #[test]
    fn enum_parameter_without_allowed_values_returns_error() {
        let tool = tool_with_parameters(
            vec![required_parameter("format", ParameterType::Enum)],
            vec!["--format={format}"],
        );

        let error = validate_tool_call(
            &tool,
            &serde_json::json!({"format": "json"}),
            &empty_policy(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::InvalidEnumConfiguration("format".to_string())
        );
    }

    #[test]
    fn unknown_input_parameter_returns_error() {
        let tool = tool_with_parameters(vec![], vec![]);
        let input = serde_json::json!({"unexpected": "value"});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::UnknownParameter("unexpected".to_string())
        );
    }

    #[test]
    fn unknown_placeholder_returns_error() {
        let tool = tool_with_parameters(vec![], vec!["{missing}"]);
        let input = serde_json::json!({});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::UnknownPlaceholder("missing".to_string())
        );
    }

    #[test]
    fn multiple_placeholders_can_be_resolved_in_one_argument() {
        let tool = tool_with_parameters(
            vec![
                required_parameter("name", ParameterType::String),
                required_parameter("message", ParameterType::String),
            ],
            vec!["Hello, {name}! Message: {message}"],
        );
        let input = serde_json::json!({
            "name": "Jasper",
            "message": "welcome"
        });

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(resolved.arguments, vec!["Hello, Jasper! Message: welcome"]);
    }

    #[test]
    fn placeholder_inside_larger_string_is_resolved() {
        let tool = tool_with_parameters(
            vec![required_parameter("min_size_mb", ParameterType::Integer)],
            vec!["+{min_size_mb}M"],
        );
        let input = serde_json::json!({"min_size_mb": 500});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(resolved.arguments, vec!["+500M"]);
    }

    #[test]
    fn working_directory_placeholder_is_resolved_to_path_buf() {
        let mut tool = tool_with_parameters(
            vec![required_parameter("path", ParameterType::Path)],
            vec!["status"],
        );
        tool.working_directory = Some("{path}".to_string());
        let input = serde_json::json!({"path": "/tmp/project"});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(
            resolved.working_directory,
            Some(PathBuf::from("/tmp/project"))
        );
    }

    #[test]
    fn path_under_allowed_root_is_accepted() {
        let allowed_root = create_temp_dir("allowed-root");
        let child = allowed_root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": child});

        let resolved =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root.clone()], vec![])).unwrap();

        assert_eq!(
            resolved.arguments,
            vec![child.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn path_outside_allowed_root_is_rejected() {
        let allowed_root = create_temp_dir("allowed-root");
        let outside_root = create_temp_dir("outside-root");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": outside_root});

        let error =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root], vec![])).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(outside_root.to_string_lossy().to_string())
        );
    }

    #[test]
    fn empty_allowed_paths_allows_normal_paths_unless_blocked() {
        let allowed_by_default = create_temp_dir("allowed-by-default");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": allowed_by_default});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(
            resolved.arguments,
            vec![allowed_by_default.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn blocked_path_is_rejected() {
        let blocked_root = create_temp_dir("blocked-root");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": blocked_root});

        let error = validate_tool_call(&tool, &input, &policy(vec![], vec![blocked_root.clone()]))
            .unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(blocked_root.to_string_lossy().to_string())
        );
    }

    #[test]
    fn blocked_path_takes_precedence_over_allowed_path() {
        let allowed_root = create_temp_dir("allowed-root");
        let blocked_root = allowed_root.join("blocked");
        std::fs::create_dir_all(&blocked_root).unwrap();
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": blocked_root});

        let error = validate_tool_call(
            &tool,
            &input,
            &policy(vec![allowed_root], vec![blocked_root.clone()]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(blocked_root.to_string_lossy().to_string())
        );
    }

    #[test]
    fn path_with_dot_dot_escaping_allowed_root_is_rejected() {
        let base = create_temp_dir("traversal-base");
        let allowed_root = base.join("allowed");
        let outside_root = base.join("outside");
        std::fs::create_dir_all(&allowed_root).unwrap();
        std::fs::create_dir_all(&outside_root).unwrap();
        let escaping_path = allowed_root.join("..").join("outside");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": escaping_path});

        let error =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root], vec![])).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(escaping_path.to_string_lossy().to_string())
        );
    }

    #[test]
    fn path_with_dot_dot_staying_inside_allowed_root_is_accepted() {
        let allowed_root = create_temp_dir("traversal-allowed-root");
        let child = allowed_root.join("child");
        let sibling = allowed_root.join("sibling");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let staying_inside_path = child.join("..").join("sibling");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": staying_inside_path});

        let resolved =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root], vec![])).unwrap();

        assert_eq!(
            resolved.arguments,
            vec![staying_inside_path.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn nonexistent_file_under_existing_allowed_parent_is_accepted() {
        let allowed_root = create_temp_dir("allowed-root");
        let missing_file = allowed_root.join("new-file.txt");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": missing_file});

        let resolved =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root], vec![])).unwrap();

        assert_eq!(
            resolved.arguments,
            vec![missing_file.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn nonexistent_path_with_no_existing_parent_is_rejected() {
        let missing_path = PathBuf::from(format!(
            "/mercurius-p-missing-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after Unix epoch")
                .as_nanos()
        ))
        .join("file.txt");
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": missing_path});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(missing_path.to_string_lossy().to_string())
        );
    }

    #[test]
    fn relative_path_is_rejected() {
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": "relative/path"});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed("relative/path".to_string())
        );
    }

    #[test]
    fn empty_path_is_rejected() {
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": ""});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(error, ToolValidationError::PathNotAllowed(String::new()));
    }

    #[test]
    fn path_containing_null_byte_is_rejected() {
        let tool = path_tool(vec!["{path}"]);
        let input = serde_json::json!({"path": "/tmp/path\u{0}suffix"});

        let error = validate_tool_call(&tool, &input, &empty_policy()).unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed("/tmp/path\u{0}suffix".to_string())
        );
    }

    #[test]
    fn literal_working_directory_under_allowed_root_is_accepted() {
        let allowed_root = create_temp_dir("workdir-allowed-root");
        let workdir = allowed_root.join("workdir");
        std::fs::create_dir_all(&workdir).unwrap();
        let mut tool = tool_with_parameters(vec![], vec!["status"]);
        tool.working_directory = Some(workdir.to_string_lossy().to_string());

        let resolved = validate_tool_call(
            &tool,
            &serde_json::json!({}),
            &policy(vec![allowed_root], vec![]),
        )
        .unwrap();

        assert_eq!(resolved.working_directory, Some(workdir));
    }

    #[test]
    fn literal_working_directory_outside_allowed_root_is_rejected() {
        let allowed_root = create_temp_dir("workdir-allowed-root");
        let outside_root = create_temp_dir("workdir-outside-root");
        let mut tool = tool_with_parameters(vec![], vec!["status"]);
        tool.working_directory = Some(outside_root.to_string_lossy().to_string());

        let error = validate_tool_call(
            &tool,
            &serde_json::json!({}),
            &policy(vec![allowed_root], vec![]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ToolValidationError::PathNotAllowed(outside_root.to_string_lossy().to_string())
        );
    }

    #[test]
    fn resolved_working_directory_from_path_placeholder_is_validated() {
        let allowed_root = create_temp_dir("workdir-placeholder-allowed-root");
        let workdir = allowed_root.join("workdir");
        std::fs::create_dir_all(&workdir).unwrap();
        let mut tool = path_tool(vec!["status"]);
        tool.working_directory = Some("{path}".to_string());
        let input = serde_json::json!({"path": workdir});

        let resolved =
            validate_tool_call(&tool, &input, &policy(vec![allowed_root], vec![])).unwrap();

        assert_eq!(resolved.working_directory, Some(workdir));
    }

    #[test]
    fn non_path_parameters_still_work() {
        let tool = tool_with_parameters(
            vec![required_parameter("message", ParameterType::String)],
            vec!["{message}"],
        );
        let input = serde_json::json!({"message": "hello"});

        let resolved = validate_tool_call(&tool, &input, &empty_policy()).unwrap();

        assert_eq!(resolved.arguments, vec!["hello"]);
    }
}
