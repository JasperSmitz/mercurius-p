use std::collections::HashMap;

use crate::command_builder::build_command;
use crate::executor::ProcessExecutor;
use crate::model::ExecutionResult;
use crate::registry::ToolRegistry;

#[derive(Debug, Clone)]
pub struct ToolExecutionService {
    registry: ToolRegistry,
}

impl ToolExecutionService {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: &HashMap<String, String>,
    ) -> Result<ExecutionResult, String> {
        let tool = self.registry.find_tool(tool_name)?;
        let built_command = build_command(tool, parameters)?;

        ProcessExecutor::execute(
            &built_command.command,
            &built_command.arguments,
            tool.timeout_ms.unwrap_or(5000),
        )
        .await
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParameterType, ToolDefinition, ToolParameter};

    fn tool_with_required_parameter() -> ToolDefinition {
        ToolDefinition {
            name: "rustc-version-with-param".to_string(),
            description: "Print rustc version with an unused parameter".to_string(),
            command: "rustc".to_string(),
            arguments: vec!["--version".to_string()],
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

    fn rustc_version_tool() -> ToolDefinition {
        ToolDefinition {
            name: "rustc-version".to_string(),
            description: "Print rustc version".to_string(),
            command: "rustc".to_string(),
            arguments: vec!["--version".to_string()],
            parameters: vec![],
            timeout_ms: Some(5000),
            read_only: false,
            category: None,
            working_directory: None,
        }
    }

    #[tokio::test]
    async fn executes_known_tool_successfully() {
        let registry = ToolRegistry::new(vec![rustc_version_tool()]);
        let service = ToolExecutionService::new(registry);
        let parameters = HashMap::new();

        match service.execute_tool("rustc-version", &parameters).await {
            Ok(result) => {
                assert!(!result.timed_out);
                assert_eq!(result.exit_code, Some(0));
                assert!(result.stdout.contains("rustc"));
            }
            Err(error) => {
                panic!("Expected tool to execute successfully, but got: {error}");
            }
        }
    }

    #[tokio::test]
    async fn returns_error_for_unknown_tool() {
        let registry = ToolRegistry::new(vec![rustc_version_tool()]);
        let service = ToolExecutionService::new(registry);
        let parameters = HashMap::new();

        match service.execute_tool("missing-tool", &parameters).await {
            Ok(result) => {
                panic!("Expected unknown tool to fail, but got: {result:?}");
            }
            Err(error) => {
                assert!(error.contains("missing-tool"));
            }
        }
    }

    #[tokio::test]
    async fn returns_error_for_missing_required_parameter() {
        let registry = ToolRegistry::new(vec![tool_with_required_parameter()]);
        let service = ToolExecutionService::new(registry);
        let parameters = HashMap::new();

        match service
            .execute_tool("rustc-version-with-param", &parameters)
            .await
        {
            Ok(result) => {
                panic!("Expected missing required parameter to fail, but got: {result:?}");
            }
            Err(error) => {
                assert!(error.contains("Missing required parameter"));
                assert!(error.contains("message"));
            }
        }
    }

    #[tokio::test]
    async fn executes_tool_when_required_parameter_is_present() {
        let registry = ToolRegistry::new(vec![tool_with_required_parameter()]);
        let service = ToolExecutionService::new(registry);

        let mut parameters = HashMap::new();
        parameters.insert("message".to_string(), "hello".to_string());

        match service
            .execute_tool("rustc-version-with-param", &parameters)
            .await
        {
            Ok(result) => {
                assert!(!result.timed_out);
                assert_eq!(result.exit_code, Some(0));
                assert!(result.stdout.contains("rustc"));
            }
            Err(error) => {
                panic!("Expected tool to execute successfully, but got: {error}");
            }
        }
    }
}
