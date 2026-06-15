use std::collections::HashSet;

use crate::model::ToolDefinition;

pub fn validate_tools(tools: &[ToolDefinition]) -> Result<(), String> {
    let mut tool_names = HashSet::new();

    for tool in tools {
        validate_tool(tool)?;

        let normalized_name = tool.name.to_lowercase();

        if !tool_names.insert(normalized_name) {
            return Err(format!("Duplicate tool name: {}", tool.name));
        }
    }

    Ok(())
}

fn validate_tool(tool: &ToolDefinition) -> Result<(), String> {
    if tool.name.trim().is_empty() {
        return Err("Tool name cannot be empty".to_string());
    }

    if tool.description.trim().is_empty() {
        return Err(format!("Tool '{}' must have a description", tool.name));
    }

    if tool.command.trim().is_empty() {
        return Err(format!("Tool '{}' must have a command", tool.name));
    }

    if tool.timeout_ms == 0 {
        return Err(format!(
            "Tool '{}' must have a timeout greater than 0",
            tool.name
        ));
    }

    validate_parameters(tool)?;

    Ok(())
}

fn validate_parameters(tool: &ToolDefinition) -> Result<(), String> {
    let mut parameter_names = HashSet::new();

    for parameter in &tool.parameters {
        if parameter.name.trim().is_empty() {
            return Err(format!(
                "Tool '{}' has a parameter with an empty name",
                tool.name
            ));
        }

        if parameter.parameter_type.trim().is_empty() {
            return Err(format!(
                "Tool '{}' has parameter '{}' with an empty type",
                tool.name, parameter.name
            ));
        }

        let normalized_name = parameter.name.to_lowercase();

        if !parameter_names.insert(normalized_name) {
            return Err(format!(
                "Tool '{}' has duplicate parameter name: {}",
                tool.name, parameter.name
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ToolDefinition, ToolParameter};

    fn valid_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: "A valid tool".to_string(),
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

    #[test]
    fn accepts_valid_tools() {
        let tools = vec![valid_tool("echo")];

        match validate_tools(&tools) {
            Ok(()) => {}
            Err(error) => panic!("Expected valid tools to pass validation, but got: {error}"),
        }
    }

    #[test]
    fn rejects_empty_tool_name() {
        let tools = vec![valid_tool("")];

        match validate_tools(&tools) {
            Ok(()) => panic!("Expected empty tool name to fail validation"),
            Err(error) => assert!(error.contains("name")),
        }
    }

    #[test]
    fn rejects_duplicate_tool_names() {
        let tools = vec![valid_tool("echo"), valid_tool("ECHO")];

        match validate_tools(&tools) {
            Ok(()) => panic!("Expected duplicate tool names to fail validation"),
            Err(error) => assert!(error.contains("Duplicate tool name")),
        }
    }

    #[test]
    fn rejects_empty_description() {
        let mut tool = valid_tool("echo");
        tool.description = " ".to_string();

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected empty description to fail validation"),
            Err(error) => assert!(error.contains("description")),
        }
    }

    #[test]
    fn rejects_empty_command() {
        let mut tool = valid_tool("echo");
        tool.command = " ".to_string();

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected empty command to fail validation"),
            Err(error) => assert!(error.contains("command")),
        }
    }

    #[test]
    fn rejects_zero_timeout() {
        let mut tool = valid_tool("echo");
        tool.timeout_ms = 0;

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected zero timeout to fail validation"),
            Err(error) => assert!(error.contains("timeout")),
        }
    }

    #[test]
    fn allows_empty_arguments() {
        let mut tool = valid_tool("echo");
        tool.arguments = vec![];

        match validate_tools(&[tool]) {
            Ok(()) => {}
            Err(error) => panic!("Expected empty arguments to be allowed, but got: {error}"),
        }
    }

    #[test]
    fn allows_empty_parameters() {
        let mut tool = valid_tool("git-status");
        tool.parameters = vec![];

        match validate_tools(&[tool]) {
            Ok(()) => {}
            Err(error) => panic!("Expected empty parameters to be allowed, but got: {error}"),
        }
    }

    #[test]
    fn rejects_empty_parameter_name() {
        let mut tool = valid_tool("echo");
        tool.parameters[0].name = " ".to_string();

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected empty parameter name to fail validation"),
            Err(error) => assert!(error.contains("parameter")),
        }
    }

    #[test]
    fn rejects_empty_parameter_type() {
        let mut tool = valid_tool("echo");
        tool.parameters[0].parameter_type = " ".to_string();

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected empty parameter type to fail validation"),
            Err(error) => assert!(error.contains("type")),
        }
    }

    #[test]
    fn rejects_duplicate_parameter_names() {
        let mut tool = valid_tool("echo");
        tool.parameters.push(ToolParameter {
            name: "MESSAGE".to_string(),
            parameter_type: "string".to_string(),
            required: false,
        });

        match validate_tools(&[tool]) {
            Ok(()) => panic!("Expected duplicate parameter names to fail validation"),
            Err(error) => assert!(error.contains("duplicate parameter")),
        }
    }
}
