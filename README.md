<div align="center">
    <h3>dot</h3>
    <p>A minimal AI agent that lives in your terminal</p>
    <br/>
    <br/>
</div>

A terminal-native AI coding agent with tool execution, memory, subagents, MCP support, extensible hooks, and persistent conversations. Built in Rust with a TUI interface designed to stay out of your way.

## Features

- **Multi-Provider**: Supports Anthropic Claude (with OAuth), OpenAI, and any OpenAI-compatible endpoint
- **Tool Execution**: Built-in file operations, shell commands, pattern search, glob, grep, web fetch, and patch
- **Batch & MultiEdit**: Parallel tool execution and multi-edit within a single file
- **Subagents**: Delegate tasks to focused subagents (blocking or background) with optional profiles and tool filters
- **Memory**: Long-term memory across conversations with core blocks, archival search (FTS5), and automatic LLM extraction
- **Snapshot & Revert**: Tracks file changes for per-message revert, checkpoints, and full restore
- **Custom Tools**: Define shell-backed tools in config.toml
- **MCP Integration**: Connect any Model Context Protocol server for extensible tooling
- **Lifecycle Hooks**: 22 events with blocking/modifying support via shell commands
- **Slash Commands**: Built-in and custom `/commands` accessible from the TUI
- **Extension Packages**: Install extensions from git with `dot install <url>`
- **Persistent Sessions**: SQLite-backed conversation history, resumable across restarts
- **Agent Profiles**: Define custom agents with specific models, prompts, and tool sets
- **Skills System**: Discovers and loads skill definitions from configurable directories
- **Context-Aware**: Auto-loads project-level and global `AGENTS.md` instructions
- **Thinking Levels**: Configurable thinking budget (Off / Low / Medium / High)
- **Vim Keybindings**: Modal editing with full vim-style navigation
- **Mouse Support**: Click, drag-select, scroll, right-click context menus
- **Image Attachments**: Paste images directly into the input
- **File Picker**: Type `@` to browse and attach files
- **Themes**: Dark, light, terminal, and auto theme detection

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

# List installed extensions
dot extensions

# Install an extension from git
dot install https://github.com/user/my-dot-extension.git

# Uninstall an extension
dot uninstall my-dot-extension
```

### Keyboard Shortcuts

**Normal mode**:
`i` insert mode, `j/k` scroll, `g/G` top/bottom, `Ctrl+D/U` half-page, `t` toggle thinking, `Tab` cycle agent, `q` quit, `Ctrl+R` rename session

**Insert mode**:
`Enter` send, `Ctrl+J` newline, `Ctrl+E` open external editor, `Ctrl+T` cycle thinking, `Ctrl+W` delete word, `Esc` normal mode

**Global**:
`Ctrl+C` cancel stream / clear input / quit, `/` command palette, `@` file picker

**Mouse**:
Left-click to interact, drag to select text, right-click for context menu, scroll wheel to navigate

## Configuration

All configuration lives in `~/.config/dot/config.toml`:

```toml
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[theme]
name = "dark"   # dark | light | terminal | auto

[tui]
vim_mode = true
favorite_models = ["claude-sonnet-4-20250514", "gpt-4o"]

[context]
auto_load_global = true
auto_load_project = true

[subagents]
enabled = true
max_turns = 20

[memory]
enabled = true
auto_extract = true
inject_count = 15
max_memories = 2000
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

### Custom Providers

```toml
[providers.my-proxy]
api = "openai"
base_url = "https://proxy.example.com/v1"
api_key_env = "MY_API_KEY"
models = ["gpt-4o"]
default_model = "gpt-4o"
```

### Custom Tools

```toml
[custom_tools.deploy]
description = "Deploy to production"
command = "scripts/deploy.sh"
schema = { type = "object", properties = { env = { type = "string" } } }
timeout = 60
```

### Slash Commands

```toml
[commands.review]
description = "Run code review"
command = "scripts/review.sh"
timeout = 30
```

Built-in commands: `/model`, `/agent`, `/clear`, `/help`, `/thinking`, `/sessions`, `/new`, `/rename`, `/export`

### Lifecycle Hooks

```toml
[hooks.before_tool_call]
command = "scripts/notify.sh"
timeout = 10

[hooks.after_prompt]
command = "scripts/log.sh"
```

Available events: `session_start`, `session_end`, `before_prompt`, `after_prompt`, `before_tool_call`, `after_tool_call`, `before_compact`, `after_compact`, `model_switch`, `agent_switch`, `on_error`, `on_stream_start`, `on_stream_end`, `on_resume`, `on_user_input`, `on_tool_error`, `before_exit`, `on_thinking_start`, `on_thinking_end`, `on_title_generated`, `before_permission_check`, `on_context_load`.

`before_*` hooks can block (exit non-zero) or modify (stdout replaces input). Other hooks are fire-and-forget.

### Extension Packages

Extensions live in `~/.config/dot/extensions/` with an `extension.toml` manifest:

```toml
name = "my-extension"
description = "An example extension"
version = "0.1.0"

[tools.lint]
description = "Run linter"
command = "scripts/lint.sh"

[commands.deploy]
description = "Deploy"
command = "scripts/deploy.sh"

[hooks.after_tool_call]
command = "scripts/notify.sh"
```

## Architecture

```
src/
  main.rs              CLI entry point and provider/tool wiring
  cli.rs               Clap argument parsing
  config.rs            TOML configuration loading (~/.config/dot/config.toml)
  context.rs           AGENTS.md discovery and system prompt injection
  extension.rs         Extension trait, hooks, lifecycle events, script tools
  command.rs           Slash command registry
  packages.rs          Extension package discovery, install, uninstall
  mcp.rs               MCP client (stdio transport, JSON-RPC)
  skills.rs            Skill discovery and loading
  snapshot.rs          File change tracking and revert
  agent/
    mod.rs             Conversation loop and tool dispatch
    events.rs          Agent event types (stream, tool, subagent)
    profile.rs         Agent profile definitions
    subagent.rs        Blocking and background subagent delegation
  memory/
    mod.rs             MemoryStore: core blocks, archival memory, FTS5 search
    extract.rs         LLM-based memory extraction from conversations
    tools.rs           Memory tools (search, add, update, delete, list)
  provider/
    mod.rs             Provider trait
    openai.rs          OpenAI and compatible implementations
    anthropic/
      mod.rs           Anthropic Claude (API key + OAuth, thinking, compaction)
      auth.rs          OAuth and token refresh
      stream.rs        SSE stream parsing
      types.rs         Request/response types
  tools/
    mod.rs             Tool trait and registry
    file.rs            File read/write
    shell.rs           Shell command execution
    patch.rs           Apply patch / diff
    glob.rs            Glob pattern matching
    grep.rs            Regex search
    web.rs             Web fetch
    batch.rs           Parallel tool execution
    multiedit.rs       Multiple edits in a single file
  tui/
    mod.rs             TUI run loop and event handling
    app.rs             Application state
    ui.rs              Main layout and message rendering
    ui_popups.rs       Popup rendering (model, agent, session, help, etc.)
    ui_tools.rs        Tool call rendering with syntax highlighting
    tools.rs           Tool categories and display extraction
    widgets.rs         Selector widgets and command palette
    markdown.rs        Markdown to styled spans
    theme.rs           Color themes (dark, light, terminal, auto)
    actions.rs         Input action definitions and dispatch
    event.rs           Terminal event stream
    input/
      mod.rs           Input handling entry point
      modes.rs         Normal and insert mode keybindings
      mouse.rs         Mouse click, drag, scroll, context menu
      popups.rs        Popup-specific input handling
  auth/
    mod.rs             Credential management
    login.rs           Login flow
    oauth.rs           OAuth helpers
    ui.rs              Auth UI
  db/
    mod.rs             SQLite session and message persistence
    schema.rs          Database schema (conversations, messages, tool_calls, memories)
```

## Development

```bash
cargo build
cargo test
```

Requires Rust nightly (edition 2024). Key dependencies: ratatui, crossterm, tokio, clap, reqwest, rusqlite, async-openai, syntect.

## License

MIT
