use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dot",
    about = "minimal ai agent",
    version,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(
        short = 's',
        long = "session",
        help = "resume a previous session by id"
    )]
    pub session: Option<String>,

    /// Output format for headless mode
    #[arg(short = 'o', long = "output", default_value = "text", value_parser = ["text", "json", "stream-json"])]
    pub output: String,

    /// Print only the final text response (no tool output)
    #[arg(long = "no-tools", default_value_t = false)]
    pub no_tools: bool,

    /// Multi-turn interactive headless mode (read prompts from stdin line by line)
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Print version
    #[arg(short = 'v', long = "version")]
    pub print_version: bool,

    /// Simulate first run (show welcome screen)
    #[arg(long = "first-run", hide = true)]
    pub first_run: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    Login,
    Config,
    /// List configured MCP servers and their tools
    Mcp,
    /// List installed extensions
    Extensions,
    /// Install an extension from a git URL
    Install {
        /// Git URL or local path to the extension
        source: String,
    },
    /// Uninstall an extension by name
    Uninstall {
        /// Name of the extension to remove
        name: String,
    },
    /// Connect to an ACP agent
    Acp {
        /// Agent name (configured in config.toml)
        name: String,
    },
    /// Run in headless mode (no TUI). Use "bg" as first arg for background mode.
    Run {
        /// The prompt to send (prefix with "bg" for background mode, omit to read from stdin)
        prompt: Vec<String>,

        /// Output format: text, json, stream-json
        #[arg(short = 'o', long = "output", default_value = "text")]
        output: String,

        /// Print only the final text response (no tool output)
        #[arg(long = "no-tools", default_value_t = false)]
        no_tools: bool,

        /// Resume a previous session
        #[arg(short = 's', long = "session")]
        session: Option<String>,

        /// Multi-turn interactive mode (read prompts from stdin line by line)
        #[arg(short = 'i', long = "interactive")]
        interactive: bool,
    },
    /// List background tasks
    Tasks,
    /// View a background task's status and output
    Task {
        /// Task ID to view
        id: String,
    },
    /// Show version information
    Version,
}
