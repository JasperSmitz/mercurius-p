use std::collections::HashMap;

use crate::model::ToolDefinition;

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    pub fn new(tools: Vec<ToolDefinition>) -> Self {
        let tools = tools
            .into_iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect();

        Self { tools }
    }

    pub fn list_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    pub fn find_tool(&self, name: &str) -> Result<&ToolDefinition, String> {
        match self.tools.get(name) {
            Some(tool) => Ok(tool),
            None => Err(format!("Tool '{name}' was not found")),
        }
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}
