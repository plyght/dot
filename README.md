<div align="center">
    <br/>
    <br/>
    <h3>dot</h3>
    <p>A minimal AI agent that lives in your terminal</p>
    <br/>
    <br/>
</div>

A terminal-native AI coding agent with tool execution, MCP support, and persistent conversations. Built in Rust with a TUI interface designed to stay out of your way.

## Features

- **Multi-Provider**: Supports Anthropic Claude and OpenAI with automatic fallback
- **Tool Execution**: Built-in file operations, shell commands, and pattern search
- **MCP Integration**: Connect any Model Context Protocol server for extensible tooling
- **Persistent Sessions**: SQLite-backed conversation history, resumable across restarts
- **Agent Profiles**: Define custom agents with specific models, prompts, and tool sets
- **Skills System**: Discovers and loads skill definitions from configurable directories
- **Context-Aware**: Auto-loads project-level and global `AGENTS.md` instructions
- **Vim Keybindings**: Modal editing with full vim-style navigation

## Install

```bash
cargo install dot-ai
```

Or from source:

```bash
git clone https://github.com/plyght/dot.git
cd dot
cargo install --path .
```

## Setup

```bash
# Authenticate with a provider
dot login

# Or set environment variables
export ANTHROPIC_API_KEY="..."
export OPENAI_API_KEY="..."
```

## Usage

```bash
# Launch the TUI
dot

# Resume a previous session
dot -s <session-id>

# Show config paths and current settings
dot config

# List MCP servers and discovered tools
dot mcp
```

Inside the TUI: `i` to enter insert mode, `Enter` to send, `Esc` to return to normal mode, `Ctrl+C` to cancel a stream or quit.

## Configuration

All configuration lives in `~/.config/dot/config.toml`:

```toml
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[theme]
name = "dark"

[tui]
vim_mode = true

[context]
auto_load_global = true
auto_load_project = true
```

### MCP Servers

```toml
[mcp.filesystem]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
enabled = true
env = {}
timeout = 30
```

### Agent Profiles

```toml
[agents.reviewer]
description = "Code review agent"
model = "claude-sonnet-4-20250514"
system_prompt = "You are a thorough code reviewer."
enabled = true

[agents.reviewer.tools]
run_command = false
```

## Architecture

```
src/
  main.rs          CLI entry point and provider/tool wiring
  config.rs        TOML configuration loading
  context.rs       AGENTS.md discovery and injection
  mcp.rs           MCP client (stdio transport, JSON-RPC)
  skills.rs        Skill discovery and loading
  agent/           Conversation loop, profiles, event types
  provider/        Provider trait + Anthropic and OpenAI implementations
  tools/           Tool trait, file operations, shell execution
  tui/             Ratatui-based interface, input handling, markdown rendering
  auth/            OAuth and API key credential management
  db/              SQLite session and message persistence
```

## Development

```bash
cargo build
cargo test
```

Requires Rust nightly (edition 2024). Key dependencies: ratatui, crossterm, tokio, clap, reqwest, rusqlite, async-openai, syntect.

## License

MIT
