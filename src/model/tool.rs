#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<String>,
    pub parameters: Vec<ToolParameter>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolParameter {
    pub name: String,

    #[serde(rename = "type")]
    pub parameter_type: String,
    
    pub required: bool,
}