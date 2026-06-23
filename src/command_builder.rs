use std::collections::HashMap;

use crate::model::ToolDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltCommand {
    pub command: String,
    pub arguments: Vec<String>,
}

pub fn build_command(
    tool: &ToolDefinition,
    parameters: &HashMap<String, String>,
) -> Result<BuiltCommand, String> {
    validate_required_parameters(tool, parameters)?;

    let arguments = tool
        .arguments
        .iter()
        .map(|argument| replace_placeholders(argument, parameters))
        .collect();

    Ok(BuiltCommand {
        command: tool.command.clone(),
        arguments,
    })
}

fn validate_required_parameters(
    tool: &ToolDefinition,
    parameters: &HashMap<String, String>,
) -> Result<(), String> {
    for parameter in &tool.parameters {
        if parameter.required {
            match parameters.get(&parameter.name) {
                Some(value) if !value.trim().is_empty() => {}
                Some(_) => {
                    return Err(format!(
                        "Required parameter '{}' for tool '{}' cannot be blank",
                        parameter.name, tool.name
                    ));
                }
                None => {
                    return Err(format!(
                        "Missing required parameter '{}' for tool '{}'",
                        parameter.name, tool.name
                    ));
                }
            }
        }
    }

    Ok(())
}

fn replace_placeholders(argument: &str, parameters: &HashMap<String, String>) -> String {
    let mut result = argument.to_string();

    for (name, value) in parameters {
        let placeholder = format!("{{{name}}}");
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParameterType, ToolDefinition, ToolParameter};

    fn valid_tool() -> ToolDefinition {
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo a message".to_string(),
            command: "echo".to_string(),
            arguments: vec!["{message}".to_string()],
            parameters: vec![ToolParameter {
                name: "message".to_string(),
                parameter_type: ParameterType::String,
                required: true,
                default: None,
                allowed_values: None,
            }],
            timeout_ms: Some(5000),
            read_only: false,
            category: None,
            working_directory: None,
        }
    }

    #[test]
    fn builds_command_with_replaced_placeholder() {
        let tool = valid_tool();
        let mut parameters = HashMap::new();
        parameters.insert("message".to_string(), "hello".to_string());

        match build_command(&tool, &parameters) {
            Ok(command) => {
                assert_eq!(command.command, "echo");
                assert_eq!(command.arguments, vec!["hello"]);
            }
            Err(error) => {
                panic!("Expected command to build successfully, but got: {error}");
            }
        }
    }

    #[test]
    fn preserves_literal_arguments() {
        let mut tool = valid_tool();
        tool.arguments = vec!["status".to_string(), "--short".to_string()];
        tool.parameters = vec![];

        let parameters = HashMap::new();

        match build_command(&tool, &parameters) {
            Ok(command) => {
                assert_eq!(command.command, "echo");
                assert_eq!(command.arguments, vec!["status", "--short"]);
            }
            Err(error) => {
                panic!("Expected literal arguments to be preserved, but got: {error}");
            }
        }
    }

    #[test]
    fn replaces_multiple_placeholders_in_one_argument() {
        let mut tool = valid_tool();
        tool.arguments = vec!["Hello, {name}! Message: {message}".to_string()];
        tool.parameters = vec![
            ToolParameter {
                name: "name".to_string(),
                parameter_type: ParameterType::String,
                required: true,
                default: None,
                allowed_values: None,
            },
            ToolParameter {
                name: "message".to_string(),
                parameter_type: ParameterType::String,
                required: true,
                default: None,
                allowed_values: None,
            },
        ];

        let mut parameters = HashMap::new();
        parameters.insert("name".to_string(), "Jasper".to_string());
        parameters.insert("message".to_string(), "welcome".to_string());

        match build_command(&tool, &parameters) {
            Ok(command) => {
                assert_eq!(command.arguments, vec!["Hello, Jasper! Message: welcome"]);
            }
            Err(error) => {
                panic!("Expected multiple placeholders to be replaced, but got: {error}");
            }
        }
    }

    #[test]
    fn returns_error_for_missing_required_parameter() {
        let tool = valid_tool();
        let parameters = HashMap::new();

        match build_command(&tool, &parameters) {
            Ok(command) => {
                panic!("Expected missing required parameter to fail, but got: {command:?}");
            }
            Err(error) => {
                assert!(error.contains("Missing required parameter"));
                assert!(error.contains("message"));
            }
        }
    }

    #[test]
    fn returns_error_for_blank_required_parameter() {
        let tool = valid_tool();
        let mut parameters = HashMap::new();
        parameters.insert("message".to_string(), "   ".to_string());

        match build_command(&tool, &parameters) {
            Ok(command) => {
                panic!("Expected blank required parameter to fail, but got: {command:?}");
            }
            Err(error) => {
                assert!(error.contains("cannot be blank"));
                assert!(error.contains("message"));
            }
        }
    }

    #[test]
    fn allows_missing_optional_parameter() {
        let mut tool = valid_tool();
        tool.arguments = vec!["hello".to_string()];
        tool.parameters = vec![ToolParameter {
            name: "message".to_string(),
            parameter_type: ParameterType::String,
            required: false,
            default: None,
            allowed_values: None,
        }];

        let parameters = HashMap::new();

        match build_command(&tool, &parameters) {
            Ok(command) => {
                assert_eq!(command.arguments, vec!["hello"]);
            }
            Err(error) => {
                panic!("Expected missing optional parameter to be allowed, but got: {error}");
            }
        }
    }

    #[test]
    fn leaves_unknown_placeholder_unchanged() {
        let mut tool = valid_tool();
        tool.arguments = vec!["{unknown}".to_string()];
        tool.parameters = vec![];

        let parameters = HashMap::new();

        match build_command(&tool, &parameters) {
            Ok(command) => {
                assert_eq!(command.arguments, vec!["{unknown}"]);
            }
            Err(error) => {
                panic!("Expected unknown placeholder to remain unchanged, but got: {error}");
            }
        }
    }
}
