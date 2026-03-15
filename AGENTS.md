- ALWAYS USE PARALLEL TOOLS WHEN APPLICABLE.
- The default branch in this repo is `main`.
- This is a Rust project (edition 2024, nightly). Build with `cargo build`, test with `cargo test`.
- Prefer automation: execute requested actions without confirmation unless blocked by missing info or safety/irreversibility.

## Style Guide

### General Principles

- Keep things in one function unless composable or reusable
- Use `anyhow::Result` with `.context()` for all fallible operations
- Avoid `.unwrap()` and `.expect()` outside of tests
- Prefer single word variable names where possible
- Rely on type inference; avoid explicit type annotations unless necessary for trait objects or clarity
- Prefer iterator chains (`filter`, `map`, `flat_map`, `collect`) over `for` loops when building collections
- Use `tracing` macros (`tracing::info!`, `tracing::warn!`) for logging, never `println!` in library code

### Naming

Prefer single word names for variables and functions. Only use multiple words if necessary.

```rust
// Good
let config = Config::load()?;
fn providers(config: &Config) -> Vec<Box<dyn Provider>> {}

// Bad
let loaded_config = Config::load()?;
fn build_provider_list(config: &Config) -> Vec<Box<dyn Provider>> {}
```

Reduce total variable count by inlining when a value is only used once.

```rust
// Good
let content = std::fs::read_to_string(Config::config_path())?;

// Bad
let path = Config::config_path();
let content = std::fs::read_to_string(&path)?;
```

### Error Handling

Use `anyhow` with context chains. Never swallow errors.

```rust
// Good
std::fs::read_to_string(&path)
    .with_context(|| format!("reading config from {}", path.display()))?;

// Bad
std::fs::read_to_string(&path).unwrap();
std::fs::read_to_string(&path).map_err(|_| anyhow!("failed"))?;
```

Use `bail!` for early failure, not `return Err(anyhow!(...))`.

```rust
// Good
if providers.is_empty() {
    bail!("No credentials found");
}

// Bad
if providers.is_empty() {
    return Err(anyhow!("No credentials found"));
}
```

### Control Flow

Avoid `else` statements. Prefer early returns and `if let` chains.

```rust
// Good
fn resolve(key: &str) -> Option<String> {
    if let Ok(val) = std::env::var(key)
        && !val.is_empty()
    {
        return Some(val);
    }
    None
}

// Bad
fn resolve(key: &str) -> Option<String> {
    if let Ok(val) = std::env::var(key) {
        if !val.is_empty() {
            return Some(val);
        } else {
            None
        }
    } else {
        None
    }
}
```

### Destructuring

Avoid unnecessary destructuring. Use field access to preserve context.

```rust
// Good
config.default_provider
config.default_model

// Bad
let Config { default_provider, default_model, .. } = config;
```

### Structs and Derives

Use `#[serde(default)]` for optional collection fields. Use helper functions for non-trivial defaults.

```rust
// Good
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

// Bad
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub command: Option<Vec<String>>,
    pub enabled: Option<bool>,
}
```

### Trait Objects

Use `Box<dyn Trait>` for runtime polymorphism (providers, tools). Keep trait surfaces minimal.

```rust
// Good
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, msgs: &[Message]) -> Result<Response>;
}

// Bad
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> String { format!("Provider: {}", self.name()) }
    async fn send(&self, msgs: &[Message]) -> Result<Response>;
    async fn send_with_retry(&self, msgs: &[Message]) -> Result<Response> { ... }
}
```

## Project Layout

```
src/
  main.rs          CLI entry point and provider/tool wiring
  lib.rs           Public module declarations
  cli.rs           Clap argument parsing
  config.rs        TOML configuration loading (~/.config/dot/config.toml)
  context.rs       AGENTS.md discovery and system prompt injection
  mcp.rs           MCP client (stdio transport, JSON-RPC)
  skills.rs        Skill discovery and loading
  agent/           Conversation loop, profiles, event types
  provider/        Provider trait + Anthropic and OpenAI implementations
  tools/           Tool trait, file operations, shell execution
  tui/             Ratatui-based interface, input handling, markdown rendering
  auth/            OAuth and API key credential management
  db/              SQLite session and message persistence
```

## TUI Scrolling

The message area uses virtual scrolling — only a window of lines around the viewport is passed to ratatui's `Paragraph`. Lines are pre-wrapped via `pre_wrap_lines()` in `tui/ui.rs` before caching, so every cached line fits within `wrap_width` and equals exactly 1 visual row. The `Paragraph` is rendered **without** `Wrap` to avoid any mismatch between our scroll math (`total_visual = line_count`) and ratatui's internal word wrapping, which can produce more rows than `div_ceil(width)` estimates. Never re-add `Wrap { trim: false }` to the message Paragraph — it will break scroll-to-bottom.

## Testing

- Avoid mocks as much as possible
- Test actual implementation, do not duplicate logic into tests
- Use `#[tokio::test]` for async tests
- Keep test functions short; one assertion per behavior
