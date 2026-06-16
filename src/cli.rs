use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mercurius-p")]
#[command(version)]
#[command(about = "A configurable MCP tool server for local command execution.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the MCP stdio server.
    Mcp {
        /// Path to the tool configuration file.
        #[arg(short, long, default_value = "tools.json")]
        config: PathBuf,
    },

    /// Validate a tool configuration file.
    Validate {
        /// Path to the tool configuration file.
        #[arg(short, long, default_value = "tools.json")]
        config: PathBuf,
    },

    /// List configured tools.
    ListTools {
        /// Path to the tool configuration file.
        #[arg(short, long, default_value = "tools.json")]
        config: PathBuf,
    },
}
