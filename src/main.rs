use std::str::FromStr;

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

    if cli.print_version {
        println!("dot {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    match cli.command {
        Some(dot::cli::Commands::Login) => {
            dot::config::Config::ensure_dirs()?;
            dot::auth::login_flow().await?;
        }
        Some(dot::cli::Commands::Config) => {
            let cfg = dot::config::Config::load()?;
            let config_path = dot::config::Config::config_path();
            let data_dir = dot::config::Config::data_dir();
            let creds_path = dot::config::Config::config_dir().join("credentials.json");
            println!("config   {}", config_path.display());
            println!("data     {}", data_dir.display());
            println!("creds    {}", creds_path.display());
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
        Some(dot::cli::Commands::Mcp) => {
            let config = dot::config::Config::load()?;
            if config.mcp.is_empty() {
                println!("No MCP servers configured.");
                println!("\nAdd servers to ~/.config/dot/config.toml:");
                println!();
                println!("  [mcp.my-server]");
                println!(
                    "  command = [\"npx\", \"-y\", \"@modelcontextprotocol/server-filesystem\", \"/tmp\"]"
                );
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
        Some(dot::cli::Commands::Extensions) => {
            let exts = dot::packages::list();
            if exts.is_empty() {
                println!("No extensions installed.");
                println!("\nInstall with: dot install <git-url>");
            } else {
                println!("Installed extensions:\n");
                for (name, desc, path) in &exts {
                    println!("  {} \u{2014} {}", name, desc);
                    println!("    {}", path.display());
                }
            }
        }
        Some(dot::cli::Commands::Install { source }) => {
            dot::config::Config::ensure_dirs()?;
            match dot::packages::install(&source) {
                Ok(msg) => println!("{}", msg),
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(dot::cli::Commands::Uninstall { name }) => match dot::packages::uninstall(&name) {
            Ok(msg) => println!("{}", msg),
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        },
        Some(dot::cli::Commands::Run {
            prompt: args,
            output,
            no_tools,
            session,
            interactive,
        }) => {
            dot::config::Config::ensure_dirs()?;
            let background = args.first().map(|s| s.as_str()) == Some("bg");
            let raw_args = if background { &args[1..] } else { &args[..] };
            let prompt = if !raw_args.is_empty() {
                raw_args.join(" ")
            } else if !interactive {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .context("reading prompt from stdin")?;
                let trimmed = buf.trim().to_string();
                if trimmed.is_empty() {
                    bail!("No prompt provided. Pass a prompt argument or pipe via stdin.");
                }
                trimmed
            } else {
                String::new()
            };

            if background {
                run_background_task(&prompt).await?;
            } else {
                let task_id = std::env::var("DOT_TASK_ID").ok();
                run_headless(prompt, output, no_tools, session, interactive, task_id).await?;
            }
        }
        Some(dot::cli::Commands::Tasks) => {
            dot::config::Config::ensure_dirs()?;
            let db = dot::db::Db::open().context("opening database")?;
            let tasks = db.list_tasks(50)?;
            if tasks.is_empty() {
                println!("No background tasks.");
            } else {
                for t in &tasks {
                    let prompt_preview = if t.prompt.len() > 60 {
                        format!("{}…", &t.prompt[..60])
                    } else {
                        t.prompt.clone()
                    };
                    let icon = match t.status.as_str() {
                        "running" => "◑",
                        "completed" => "●",
                        "failed" => "✗",
                        _ => "○",
                    };
                    println!("{} {} [{}] {}", icon, &t.id[..8], t.status, prompt_preview);
                }
            }
        }
        Some(dot::cli::Commands::Task { id }) => {
            dot::config::Config::ensure_dirs()?;
            let db = dot::db::Db::open().context("opening database")?;
            let tasks = db.list_tasks(100)?;
            let task = tasks
                .iter()
                .find(|t| t.id.starts_with(&id))
                .or_else(|| tasks.iter().find(|t| t.id == id));
            match task {
                Some(t) => {
                    println!("id       {}", t.id);
                    println!("status   {}", t.status);
                    println!("prompt   {}", t.prompt);
                    if let Some(ref sid) = t.session_id {
                        println!("session  {}", sid);
                    }
                    if let Some(ref ts) = t.completed_at {
                        println!("finished {}", ts);
                    }
                    if let Some(ref out) = t.output {
                        println!("\n{}", out);
                    }
                }
                None => {
                    eprintln!("Task not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        Some(dot::cli::Commands::Version) => {
            println!("dot {}", env!("CARGO_PKG_VERSION"));
        }

        Some(dot::cli::Commands::Acp { name }) => {
            dot::config::Config::ensure_dirs()?;
            let config = dot::config::Config::load()?;
            let cwd = std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            run_acp(config, &name, cwd).await?;
        }
        None => {
            dot::config::Config::ensure_dirs()?;
            let mut config = dot::config::Config::load()?;
            dot::packages::merge_into_config(&mut config);
            let cwd = std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            {
                let creds = dot::auth::Credentials::load()?;
                let db = dot::db::Db::open().context("opening database")?;
                let memory = if config.memory.enabled {
                    Some(std::sync::Arc::new(
                        dot::memory::MemoryStore::open().context("opening memory store")?,
                    ))
                } else {
                    None
                };
                let providers = build_providers(&config, &creds)?;
                let (tools, skill_names) = build_tool_registry(&config);
                let profiles = build_agent_profiles(&config);
                let hooks = build_hooks(&config);
                let commands = build_commands(&config);
                let resume_id = cli.session.clone();
                dot::tui::run(
                    config,
                    providers,
                    db,
                    memory,
                    tools,
                    profiles,
                    cwd,
                    resume_id,
                    skill_names,
                    hooks,
                    commands,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn run_background_task(prompt: &str) -> Result<()> {
    let task_id = uuid::Uuid::new_v4().to_string();
    let exe = std::env::current_exe().context("resolving executable path")?;
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let child = std::process::Command::new(&exe)
        .args(["run", prompt, "-o", "json"])
        .env("DOT_TASK_ID", &task_id)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning background task")?;

    let db = dot::db::Db::open().context("opening database")?;
    db.create_task(&task_id, prompt, child.id(), &cwd)?;

    println!("{}", &task_id[..8]);
    Ok(())
}

async fn run_headless(
    prompt: String,
    output: String,
    no_tools: bool,
    session: Option<String>,
    interactive: bool,
    task_id: Option<String>,
) -> Result<()> {
    dot::config::Config::ensure_dirs()?;
    let mut config = dot::config::Config::load()?;
    dot::packages::merge_into_config(&mut config);
    let creds = dot::auth::Credentials::load()?;
    let db = dot::db::Db::open().context("opening database")?;
    let memory = if config.memory.enabled {
        Some(std::sync::Arc::new(
            dot::memory::MemoryStore::open().context("opening memory store")?,
        ))
    } else {
        None
    };
    let providers = build_providers(&config, &creds)?;
    let (tools, skill_names) = build_tool_registry(&config);
    let profiles = build_agent_profiles(&config);
    let hooks = build_hooks(&config);
    let commands = build_commands(&config);
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let opts = dot::headless::HeadlessOptions {
        prompt,
        format: dot::headless::OutputFormat::parse(&output),
        no_tools,
        resume_id: session,
        interactive,
        task_id,
    };
    dot::headless::run(
        config,
        providers,
        db,
        memory,
        tools,
        profiles,
        cwd,
        skill_names,
        hooks,
        commands,
        opts,
    )
    .await
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

fn build_tool_registry(
    config: &dot::config::Config,
) -> (dot::tools::ToolRegistry, Vec<(String, String)>) {
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

    for (name, cfg) in &config.custom_tools {
        let tool = dot::extension::ScriptTool::new(
            name.clone(),
            cfg.description.clone(),
            cfg.schema.clone(),
            cfg.command.clone(),
            cfg.timeout,
        );
        registry.register(Box::new(tool));
        tracing::info!("Registered custom tool: {}", name);
    }

    let skill_registry = dot::skills::SkillRegistry::discover();
    let skill_names: Vec<(String, String)> = skill_registry
        .skills()
        .iter()
        .map(|s| (s.name.clone(), s.description.clone()))
        .collect();
    if let Some(skill_tool) = skill_registry.into_tool() {
        registry.register(Box::new(skill_tool));
    }

    tracing::info!("Tool registry: {} tools total", registry.tool_count());
    (registry, skill_names)
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
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY")
        && !key.is_empty()
    {
        return Some(Box::new(
            dot::provider::anthropic::AnthropicProvider::new_with_api_key(key, model),
        ));
    }
    None
}

fn build_openai(
    creds: &dot::auth::Credentials,
    model: String,
) -> Option<Box<dyn dot::provider::Provider>> {
    if let Some(cred) = creds.get("openai")
        && let Some(api_key) = cred.api_key()
    {
        let oai_config =
            async_openai::config::OpenAIConfig::new().with_api_key(api_key.to_string());
        return Some(Box::new(
            dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
        ));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY")
        && !key.is_empty()
    {
        let oai_config = async_openai::config::OpenAIConfig::new().with_api_key(key);
        return Some(Box::new(
            dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
        ));
    }
    None
}

fn build_copilot(
    creds: &dot::auth::Credentials,
    model: String,
) -> Option<Box<dyn dot::provider::Provider>> {
    if let Some(cred) = creds.get("copilot")
        && let Some(token) = cred.api_key()
    {
        return Some(Box::new(dot::provider::copilot::CopilotProvider::new(
            token.to_string(),
            model,
        )));
    }
    if let Some(token) = dot::auth::copilot::read_existing_token() {
        return Some(Box::new(dot::provider::copilot::CopilotProvider::new(
            token, model,
        )));
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
    let copilot = build_copilot(creds, "gpt-4o".to_string());

    match config.default_provider.as_str() {
        "anthropic" => {
            if let Some(p) = anthropic {
                providers.push(p);
            }
            if let Some(p) = openai {
                providers.push(p);
            }
            if let Some(p) = copilot {
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
            if let Some(p) = copilot {
                providers.push(p);
            }
        }
        "copilot" => {
            if let Some(p) = copilot {
                providers.push(p);
            }
            if let Some(p) = anthropic {
                providers.push(p);
            }
            if let Some(p) = openai {
                providers.push(p);
            }
        }
        other => {
            if let Some(p) = build_custom_provider(other, config) {
                providers.push(p);
            }
            if let Some(p) = anthropic {
                providers.push(p);
            }
            if let Some(p) = openai {
                providers.push(p);
            }
            if let Some(p) = copilot {
                providers.push(p);
            }
        }
    }

    for (name, def) in &config.providers {
        if !def.enabled {
            continue;
        }
        if Some(name.as_str()) == Some(config.default_provider.as_str()) {
            continue;
        }
        if let Some(p) = build_provider_from_def(name, def) {
            providers.push(p);
        }
    }

    if providers.is_empty() {
        bail!(
            "No credentials found.\n\
             Set ANTHROPIC_API_KEY or OPENAI_API_KEY, or run `dot login`."
        );
    }

    Ok(providers)
}

fn build_custom_provider(
    name: &str,
    config: &dot::config::Config,
) -> Option<Box<dyn dot::provider::Provider>> {
    let def = config.providers.get(name)?;
    build_provider_from_def(name, def)
}

fn build_provider_from_def(
    name: &str,
    def: &dot::config::ProviderDefinition,
) -> Option<Box<dyn dot::provider::Provider>> {
    let key = def
        .api_key_env
        .as_deref()
        .and_then(|env| std::env::var(env).ok())
        .filter(|k| !k.is_empty());
    let key = match key {
        Some(k) => k,
        None => {
            tracing::warn!("Provider '{}' has no API key (env var not set)", name);
            return None;
        }
    };
    let model = def
        .default_model
        .clone()
        .or_else(|| def.models.first().cloned())
        .unwrap_or_else(|| "default".to_string());
    match def.api.as_str() {
        "openai" => {
            let mut oai_config = async_openai::config::OpenAIConfig::new().with_api_key(key);
            if let Some(ref url) = def.base_url {
                oai_config = oai_config.with_api_base(url);
            }
            Some(Box::new(
                dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
            ))
        }
        "anthropic" => Some(Box::new(
            dot::provider::anthropic::AnthropicProvider::new_with_api_key(key, model),
        )),
        other => {
            tracing::warn!("Unknown provider API type '{}' for '{}'", other, name);
            None
        }
    }
}

fn build_hooks(config: &dot::config::Config) -> dot::extension::HookRegistry {
    let mut registry = dot::extension::HookRegistry::new();
    for (event_name, cfg) in &config.hooks {
        if let Ok(event) = dot::extension::Event::from_str(event_name) {
            registry.register(dot::extension::Hook {
                event,
                command: cfg.command.clone(),
                timeout: cfg.timeout,
            });
            tracing::info!("Registered hook for event: {}", event_name);
        } else {
            tracing::warn!("Unknown hook event: {}", event_name);
        }
    }
    registry
}

fn build_commands(config: &dot::config::Config) -> dot::command::CommandRegistry {
    let mut registry = dot::command::CommandRegistry::new();
    for (name, cfg) in &config.commands {
        registry.register(dot::command::SlashCommand::from_config(name, cfg));
    }
    registry
}

async fn run_acp(config: dot::config::Config, agent_name: &str, cwd: String) -> Result<()> {
    let agent_config = config
        .acp_agents
        .get(agent_name)
        .with_context(|| format!("ACP agent '{}' not found in config", agent_name))?;
    if agent_config.command.is_empty() {
        bail!("ACP agent '{}' has no command configured", agent_name);
    }
    let command = &agent_config.command[0];
    let args: Vec<String> = agent_config.command[1..].to_vec();
    let env: Vec<(String, String)> = agent_config
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut client = dot::acp::AcpClient::start(command, &args, &env)
        .with_context(|| format!("starting ACP agent '{}'", agent_name))?;

    let init = client.initialize().await.context("ACP initialize")?;
    tracing::info!("Connected to ACP agent: {:?}", init.agent_info);

    for auth in &init.auth_methods {
        client
            .authenticate(&auth.id)
            .await
            .with_context(|| format!("ACP authenticate ({})", auth.id))?;
        tracing::info!("ACP authenticated with method: {}", auth.id);
    }

    let session = client
        .new_session(&cwd, vec![])
        .await
        .context("ACP new session")?;
    tracing::info!("ACP session created: {}", session.session_id);

    dot::tui::run_acp(config, client).await
}
