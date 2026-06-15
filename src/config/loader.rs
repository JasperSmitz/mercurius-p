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
        let result = load_tools_from_file("tools.json");

        match result {
            Ok(tools) => {
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].name, "echo");
                assert_eq!(tools[0].parameters.len(), 1);
            }
            Err(error) => {
                panic!("Expected tools.json to load successfully, but got: {error}");
            }
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
