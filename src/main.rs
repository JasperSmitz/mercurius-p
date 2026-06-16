use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use mercurius_p::cli::{Cli, Commands};
use mercurius_p::config::{load_tools_from_file, validate_tools};
use mercurius_p::mcp::{McpHandler, McpStdioServer};
use mercurius_p::model::ToolDefinition;
use mercurius_p::registry::ToolRegistry;
use mercurius_p::service::ToolExecutionService;

fn print_banner() {
    let banner = include_str!("../assets/banner.txt");
    println!("{banner}");
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Mcp { config } => run_mcp(&config).await,
        Commands::Validate { config } => run_validate(&config),
        Commands::ListTools { config } => run_list_tools(&config),
    }
}

async fn run_mcp(config_path: &Path) -> ExitCode {
    let tools = match load_and_validate_tools(config_path) {
        Ok(tools) => tools,
        Err(error) => {
            eprintln!("Failed to start MCP server: {error}");
            return ExitCode::FAILURE;
        }
    };

    let registry = ToolRegistry::new(tools);
    let service = ToolExecutionService::new(registry);
    let handler = McpHandler::new(service, config_path);
    let server = McpStdioServer::new(handler);

    match server.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("MCP server error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_validate(config_path: &Path) -> ExitCode {
    match load_and_validate_tools(config_path) {
        Ok(tools) => {
            print_banner();
            println!("Configuration is valid. Loaded {} tool(s).", tools.len());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Configuration is invalid: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_list_tools(config_path: &Path) -> ExitCode {
    let tools = match load_and_validate_tools(config_path) {
        Ok(tools) => tools,
        Err(error) => {
            eprintln!("Failed to load tools: {error}");
            return ExitCode::FAILURE;
        }
    };

    if tools.is_empty() {
        println!("No tools configured.");
        return ExitCode::SUCCESS;
    }

    for tool in tools {
        println!("{} - {}", tool.name, tool.description);
    }

    ExitCode::SUCCESS
}

fn load_and_validate_tools(config_path: &Path) -> Result<Vec<ToolDefinition>, String> {
    let tools = match load_tools_from_file(config_path) {
        Ok(tools) => tools,
        Err(error) => {
            return Err(format!("Failed to load config file: {error}"));
        }
    };

    match validate_tools(&tools) {
        Ok(()) => Ok(tools),
        Err(error) => Err(format!("Validation failed: {error}")),
    }
}
