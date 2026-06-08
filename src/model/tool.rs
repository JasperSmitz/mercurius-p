#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<String>,
    pub parameters: Vec<String>,
    pub timeout: u64,
}