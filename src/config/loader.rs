use std::fs;
use std::path::Path;

use crate::model::ToolDefinition;

pub fn load_tools_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<ToolDefinition>, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let tools: Vec<ToolDefinition> = serde_json::from_str(&contents)?;

    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_tools_from_valid_json_file() {
        let path = std::env::temp_dir().join(format!(
            "mercurius_p_test_tools_{}.json",
            std::process::id()
        ));

        let json = r#"
        [
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
        ]
        "#;

        if let Err(error) = std::fs::write(&path, json) {
            panic!("Expected test config file to be written, but got: {error}");
        }

        match load_tools_from_file(&path) {
            Ok(tools) => {
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].name, "echo");
                assert_eq!(tools[0].parameters.len(), 1);
            }
            Err(error) => {
                panic!("Expected test config file to load successfully, but got: {error}");
            }
        }

        if let Err(error) = std::fs::remove_file(&path) {
            panic!("Expected test config file to be removed, but got: {error}");
        }
    }

    #[test]
    fn returns_error_for_missing_file() {
        let result = load_tools_from_file("missing-tools.json");

        match result {
            Ok(_) => {
                panic!("Expected loading missing-tools.json to fail, but it succeeded");
            }
            Err(_) => {
                // This is expected.
            }
        }
    }
}
