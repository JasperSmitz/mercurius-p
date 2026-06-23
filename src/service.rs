use std::path::PathBuf;

use crate::executor::ProcessExecutor;
use crate::model::{ExecutionResult, SecurityPolicy, validate_tool_call};
use crate::registry::ToolRegistry;

#[derive(Debug, Clone)]
pub struct ToolExecutionService {
    registry: ToolRegistry,
    security_policy: SecurityPolicy,
}

impl ToolExecutionService {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            security_policy: default_security_policy(),
        }
    }

    pub fn new_with_security_policy(
        registry: ToolRegistry,
        security_policy: SecurityPolicy,
    ) -> Self {
        Self {
            registry,
            security_policy,
        }
    }

    pub async fn execute_tool(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Result<ExecutionResult, String> {
        let tool = self.registry.find_tool(tool_name)?;
        let resolved = validate_tool_call(tool, input, &self.security_policy)
            .map_err(|error| format!("Tool validation failed: {error}"))?;

        ProcessExecutor::execute(
            &resolved.command,
            &resolved.arguments,
            resolved.working_directory.as_deref(),
            resolved.timeout_ms,
        )
        .await
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

pub fn default_security_policy() -> SecurityPolicy {
    SecurityPolicy {
        allowed_paths: vec![],
        blocked_paths: vec![
            PathBuf::from("/etc"),
            PathBuf::from("/boot"),
            PathBuf::from("/usr"),
            PathBuf::from("/bin"),
            PathBuf::from("/sbin"),
            PathBuf::from("/root"),
        ],
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

    fn echo_tool() -> ToolDefinition {
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

    fn path_echo_tool() -> ToolDefinition {
        ToolDefinition {
            name: "path-echo".to_string(),
            description: "Echo a path".to_string(),
            command: "echo".to_string(),
            arguments: vec!["{path}".to_string()],
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                parameter_type: ParameterType::Path,
                required: true,
                default: None,
                allowed_values: None,
            }],
            timeout_ms: Some(5000),
            read_only: true,
            category: None,
            working_directory: None,
        }
    }

    fn policy(allowed_paths: Vec<PathBuf>, blocked_paths: Vec<PathBuf>) -> SecurityPolicy {
        SecurityPolicy {
            allowed_paths,
            blocked_paths,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("mercurius-p-service-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn executes_known_tool_successfully() {
        let registry = ToolRegistry::new(vec![rustc_version_tool()]);
        let service = ToolExecutionService::new(registry);
        let input = serde_json::json!({});

        match service.execute_tool("rustc-version", &input).await {
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
        let input = serde_json::json!({});

        match service.execute_tool("missing-tool", &input).await {
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
        let input = serde_json::json!({});

        match service
            .execute_tool("rustc-version-with-param", &input)
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

        let input = serde_json::json!({"message": "hello"});

        match service
            .execute_tool("rustc-version-with-param", &input)
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

    #[tokio::test]
    async fn returns_validation_error_for_unknown_parameter_before_execution() {
        let registry = ToolRegistry::new(vec![ToolDefinition {
            command: "definitely-not-a-real-command".to_string(),
            ..echo_tool()
        }]);
        let service = ToolExecutionService::new(registry);
        let input = serde_json::json!({
            "message": "hello",
            "unexpected": "value"
        });

        let error = service.execute_tool("echo", &input).await.unwrap_err();

        assert!(error.contains("Tool validation failed"));
        assert!(error.contains("Unknown parameter"));
        assert!(error.contains("unexpected"));
    }

    #[tokio::test]
    async fn resolves_placeholder_before_execution() {
        let registry = ToolRegistry::new(vec![echo_tool()]);
        let service = ToolExecutionService::new(registry);
        let input = serde_json::json!({"message": "hello"});

        let result = service.execute_tool("echo", &input).await.unwrap();

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn uses_default_timeout_when_tool_timeout_is_missing() {
        let mut tool = rustc_version_tool();
        tool.timeout_ms = None;
        let registry = ToolRegistry::new(vec![tool]);
        let service = ToolExecutionService::new(registry);

        let result = service
            .execute_tool("rustc-version", &serde_json::json!({}))
            .await
            .unwrap();

        assert!(!result.timed_out);
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn rejects_blocked_path_before_execution() {
        let blocked_root = temp_dir("blocked-root");
        let registry = ToolRegistry::new(vec![ToolDefinition {
            command: "definitely-not-a-real-command".to_string(),
            ..path_echo_tool()
        }]);
        let service = ToolExecutionService::new_with_security_policy(
            registry,
            policy(vec![], vec![blocked_root.clone()]),
        );
        let input = serde_json::json!({"path": blocked_root});

        let error = service.execute_tool("path-echo", &input).await.unwrap_err();

        assert!(error.contains("Path not allowed"));
        assert!(error.contains(&blocked_root.to_string_lossy().to_string()));
    }

    #[tokio::test]
    async fn valid_path_parameter_is_passed_to_execution() {
        let allowed_root = temp_dir("allowed-root");
        let child = allowed_root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let registry = ToolRegistry::new(vec![path_echo_tool()]);
        let service = ToolExecutionService::new_with_security_policy(
            registry,
            policy(vec![allowed_root], vec![]),
        );
        let input = serde_json::json!({"path": child});

        let result = service.execute_tool("path-echo", &input).await.unwrap();

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), child.to_string_lossy());
    }

    #[tokio::test]
    async fn rejects_blocked_working_directory_before_execution() {
        let blocked_root = temp_dir("blocked-workdir");
        let mut tool = rustc_version_tool();
        tool.command = "definitely-not-a-real-command".to_string();
        tool.working_directory = Some(blocked_root.to_string_lossy().to_string());
        let registry = ToolRegistry::new(vec![tool]);
        let service = ToolExecutionService::new_with_security_policy(
            registry,
            policy(vec![], vec![blocked_root.clone()]),
        );

        let error = service
            .execute_tool("rustc-version", &serde_json::json!({}))
            .await
            .unwrap_err();

        assert!(error.contains("Path not allowed"));
        assert!(error.contains(&blocked_root.to_string_lossy().to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn valid_working_directory_is_passed_to_executor() {
        let workdir = temp_dir("valid-workdir");
        let tool = ToolDefinition {
            name: "pwd-tool".to_string(),
            description: "Print working directory".to_string(),
            command: "pwd".to_string(),
            arguments: vec![],
            parameters: vec![],
            timeout_ms: Some(5000),
            read_only: true,
            category: None,
            working_directory: Some(workdir.to_string_lossy().to_string()),
        };
        let registry = ToolRegistry::new(vec![tool]);
        let service = ToolExecutionService::new(registry);

        let result = service
            .execute_tool("pwd-tool", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), workdir.to_string_lossy());
    }
}
