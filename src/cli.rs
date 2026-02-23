use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dot", about = "minimal ai agent")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(
        short = 's',
        long = "session",
        help = "resume a previous session by id"
    )]
    pub session: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Login,
    Config,
    /// List configured MCP servers and their tools
    Mcp,
}
