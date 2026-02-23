use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dot", about = "minimal ai agent")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Login,
    Config,
    /// List configured MCP servers and their tools
    Mcp,
}
