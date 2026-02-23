use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use dot::auth::ProviderCredential;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install().map_err(|e| anyhow!("{e}"))?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = dot::cli::Cli::parse();
    match cli.command {
        Some(dot::cli::Commands::Login) => {
            dot::config::Config::ensure_dirs()?;
            dot::auth::login_flow().await?;
        }
        Some(dot::cli::Commands::Config) => {
            let config_path = dot::config::Config::config_path();
            let data_dir = dot::config::Config::data_dir();
            let creds_path = dot::config::Config::config_dir().join("credentials.json");
            println!("config   {}", config_path.display());
            println!("data     {}", data_dir.display());
            println!("creds    {}", creds_path.display());
            if config_path.exists() {
                let cfg = dot::config::Config::load()?;
                println!("provider {}", cfg.default_provider);
                println!("model    {}", cfg.default_model);
                if !cfg.mcp.is_empty() {
                    println!("\nmcp servers:");
                    for (name, mcfg) in &cfg.mcp {
                        let status = if mcfg.enabled { "on" } else { "off" };
                        println!("  {} [{}] {:?}", name, status, mcfg.command);
                    }
                }
                if !cfg.agents.is_empty() {
                    println!("\nagents:");
                    for (name, acfg) in &cfg.agents {
                        println!("  {} — {}", name, acfg.description);
                    }
                }
            }
        }
        Some(dot::cli::Commands::Mcp) => {
            let config = dot::config::Config::load()?;
            if config.mcp.is_empty() {
                println!("No MCP servers configured.");
                println!("\nAdd servers to ~/.config/dot/config.toml:");
                println!();
                println!("  [mcp.my-server]");
                println!("  command = [\"npx\", \"-y\", \"@modelcontextprotocol/server-filesystem\", \"/tmp\"]");
                println!("  enabled = true");
                return Ok(());
            }

            for (name, cfg) in &config.mcp {
                let status = if cfg.enabled { "enabled" } else { "disabled" };
                println!("{} [{}]", name, status);
                if !cfg.command.is_empty() {
                    println!("  command: {}", cfg.command.join(" "));
                }
                if let Some(url) = &cfg.url {
                    println!("  url: {}", url);
                }

                if cfg.enabled && !cfg.command.is_empty() {
                    match try_list_mcp_tools(name, &cfg.command, &cfg.env) {
                        Ok(tools) => {
                            println!("  tools ({}):", tools.len());
                            for t in &tools {
                                let desc = t.description.as_deref().unwrap_or("");
                                println!("    {} — {}", t.name, desc);
                            }
                        }
                        Err(e) => {
                            println!("  error: {}", e);
                        }
                    }
                }
                println!();
            }
        }
        None => {
            dot::config::Config::ensure_dirs()?;
            let config = dot::config::Config::load()?;
            let creds = dot::auth::Credentials::load()?;
            let db = dot::db::Db::open().context("opening database")?;
            let providers = build_providers(&config, &creds)?;

            let tools = build_tool_registry(&config);
            let profiles = build_agent_profiles(&config);

            dot::tui::run(config, providers, db, tools, profiles).await?;
        }
    }
    Ok(())
}

fn try_list_mcp_tools(
    name: &str,
    command: &[String],
    env: &std::collections::HashMap<String, String>,
) -> Result<Vec<dot::mcp::McpToolDef>> {
    let client = dot::mcp::McpClient::start(name, command, env)?;
    client.initialize()?;
    client.list_tools()
}

fn build_tool_registry(config: &dot::config::Config) -> dot::tools::ToolRegistry {
    let mut registry = dot::tools::ToolRegistry::default_tools();

    for (name, cfg) in &config.mcp {
        if !cfg.enabled || cfg.command.is_empty() {
            continue;
        }
        let mut manager = dot::mcp::McpManager::new();
        match manager.start_server(name, &cfg.command, &cfg.env) {
            Ok(()) => {
                let mcp_tools = manager.discover_tools();
                let count = mcp_tools.len();
                registry.register_many(mcp_tools);
                tracing::info!("Registered {} MCP tools from '{}'", count, name);
            }
            Err(e) => {
                tracing::warn!("Failed to start MCP server '{}': {}", name, e);
                eprintln!("warning: MCP server '{}' failed to start: {}", name, e);
            }
        }
    }

    let skill_registry = dot::skills::SkillRegistry::discover();
    if let Some(skill_tool) = skill_registry.into_tool() {
        registry.register(Box::new(skill_tool));
    }

    tracing::info!("Tool registry: {} tools total", registry.tool_count());
    registry
}

fn build_agent_profiles(config: &dot::config::Config) -> Vec<dot::agent::AgentProfile> {
    let mut profiles = vec![dot::agent::AgentProfile::default_profile()];

    for (name, cfg) in &config.agents {
        if !cfg.enabled {
            continue;
        }
        profiles.push(dot::agent::AgentProfile::from_config(name, cfg));
    }

    profiles
}

fn build_anthropic(
    creds: &dot::auth::Credentials,
    model: String,
) -> Option<Box<dyn dot::provider::Provider>> {
    if let Some(cred) = creds.get("anthropic") {
        return match cred {
            ProviderCredential::OAuth {
                access_token,
                refresh_token,
                expires_at,
                ..
            } => Some(Box::new(
                dot::provider::anthropic::AnthropicProvider::new_with_oauth(
                    access_token.clone(),
                    refresh_token.clone().unwrap_or_default(),
                    expires_at.unwrap_or(0),
                    model,
                ),
            )),
            ProviderCredential::ApiKey { key } => Some(Box::new(
                dot::provider::anthropic::AnthropicProvider::new_with_api_key(key.clone(), model),
            )),
        };
    }
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            return Some(Box::new(
                dot::provider::anthropic::AnthropicProvider::new_with_api_key(key, model),
            ));
        }
    }
    None
}

fn build_openai(
    creds: &dot::auth::Credentials,
    model: String,
) -> Option<Box<dyn dot::provider::Provider>> {
    if let Some(cred) = creds.get("openai") {
        if let Some(api_key) = cred.api_key() {
            let oai_config =
                async_openai::config::OpenAIConfig::new().with_api_key(api_key.to_string());
            return Some(Box::new(
                dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
            ));
        }
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            let oai_config = async_openai::config::OpenAIConfig::new().with_api_key(key);
            return Some(Box::new(
                dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
            ));
        }
    }
    None
}

fn build_providers(
    config: &dot::config::Config,
    creds: &dot::auth::Credentials,
) -> Result<Vec<Box<dyn dot::provider::Provider>>> {
    let model = config.default_model.clone();
    let mut providers: Vec<Box<dyn dot::provider::Provider>> = Vec::new();

    let anthropic = build_anthropic(creds, model.clone());
    let openai = build_openai(creds, "gpt-4o".to_string());

    match config.default_provider.as_str() {
        "anthropic" => {
            if let Some(p) = anthropic {
                providers.push(p);
            }
            if let Some(p) = openai {
                providers.push(p);
            }
        }
        "openai" => {
            if let Some(p) = openai {
                providers.push(p);
            }
            if let Some(p) = anthropic {
                providers.push(p);
            }
        }
        other => bail!("Unknown provider '{other}'. Supported: anthropic, openai"),
    }

    if providers.is_empty() {
        bail!(
            "No credentials found.\n\
             Set ANTHROPIC_API_KEY or OPENAI_API_KEY, or run `dot login`."
        );
    }

    Ok(providers)
}
