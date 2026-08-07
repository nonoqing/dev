use anyhow::{anyhow, Context, Result};
use bitfun_agent_runtime::sdk::AgentSessionUsageRequest;
use bitfun_core::agentic::get_agent_registry;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::plugin_runtime::{
    activate_managed_plugin, deactivate_managed_plugin, preview_managed_plugin_activation,
    ManagedPluginActivationView, ManagedPluginDeactivationResult,
};
use bitfun_core::plugin_source::{
    refresh_managed_plugin_sources, set_managed_plugin_trust, ManagedPluginSourceError,
    ManagedPluginSourceIssue, ManagedPluginSourceSnapshot, ManagedPluginTrustDecision,
    ManagedPluginTrustLevel,
};
use bitfun_core::product_assembly::ProductRuntimeParts;
use bitfun_core::runtime_ports::PluginRuntimeAvailability;
use bitfun_core::service::config::initialize_global_config;
use bitfun_core::service::session_usage::render_usage_report_markdown;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

async fn ensure_global_config_service(
) -> Result<std::sync::Arc<bitfun_core::service::config::ConfigService>> {
    initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    bitfun_core::service::config::get_global_config_service()
        .await
        .context("Failed to get global config service")
}

pub(crate) async fn print_agents(workspace: Option<&Path>) -> Result<()> {
    let registry = get_agent_registry();
    if workspace.is_some() {
        if let Err(error) =
            bitfun_core::external_sources::ensure_external_source_workspace_snapshot(workspace)
                .await
        {
            eprintln!(
                "Warning: external agent sources could not be refreshed: {}",
                crate::plugin_diagnostics::escape_terminal_text(&error)
            );
        }
    }
    let modes = registry
        .get_modes_info_for_workspace(workspace, workspace.is_some())
        .await;
    let subagents = registry.get_subagents_info(workspace).await;

    println!("Agent modes");
    println!();
    if modes.is_empty() {
        println!("No agent modes found.");
    } else {
        for agent in modes {
            println!(
                "- {}: {} (tools: {}, readonly: {}, review: {})",
                agent.id, agent.name, agent.tool_count, agent.is_readonly, agent.is_review
            );
            if !agent.description.is_empty() {
                println!("  {}", agent.description);
            }
        }
    }

    println!();
    println!("Subagents");
    println!();
    if subagents.is_empty() {
        println!("No subagents found for the current workspace.");
    } else {
        for agent in subagents {
            println!(
                "- {}: {} (tools: {}, enabled: {}, readonly: {}, review: {})",
                agent.id,
                agent.name,
                agent.tool_count,
                agent.effective_enabled,
                agent.is_readonly,
                agent.is_review
            );
            if !agent.description.is_empty() {
                println!("  {}", agent.description);
            }
        }
    }

    Ok(())
}

pub(crate) async fn print_models() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let models = config_service.get_ai_models().await?;
    let global_config: bitfun_core::service::config::GlobalConfig =
        config_service.get_config(None).await?;

    let primary_model_id = global_config.ai.default_models.primary.clone();
    let mode_model_id = crate::model_selection::resolve_mode_model_id(&global_config.ai);

    println!("AI models");
    println!();
    if models.is_empty() {
        println!("No AI models configured.");
        return Ok(());
    }

    for model in models {
        let is_primary = primary_model_id.as_deref() == Some(model.id.as_str());
        let is_mode_default = mode_model_id.as_deref() == Some(model.id.as_str());

        println!(
            "- {}{} ({})",
            if is_primary { "* " } else { "  " },
            model.id,
            if model.enabled { "enabled" } else { "disabled" }
        );
        println!("  Name: {}", model.name);
        println!("  Provider: {}", model.provider);
        println!("  Model: {}", model.model_name);
        if is_mode_default {
            println!("  Used by modes: all");
        }
    }

    Ok(())
}

pub(crate) async fn print_mcp_servers() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let configs = mcp_service.config_service().load_all_configs().await?;

    println!("MCP servers");
    println!();
    if configs.is_empty() {
        println!("No MCP servers configured.");
        return Ok(());
    }

    for config in configs {
        let status = if config.enabled {
            match tokio::time::timeout(
                Duration::from_millis(30),
                mcp_service.server_manager().get_server_status(&config.id),
            )
            .await
            {
                Ok(Ok(status)) => format!("{:?}", status),
                Ok(Err(_)) => "Unknown".to_string(),
                Err(_) => "Starting".to_string(),
            }
        } else {
            "Disabled".to_string()
        };

        let endpoint = match config.server_type {
            bitfun_core::service::mcp::server::MCPServerType::Local => config
                .command
                .as_ref()
                .map(|cmd| format!("{} {}", cmd, config.args.join(" ")))
                .unwrap_or_else(|| "<missing command>".to_string()),
            bitfun_core::service::mcp::server::MCPServerType::Remote => config
                .url
                .clone()
                .unwrap_or_else(|| "<missing url>".to_string()),
        };

        println!("- {} ({:?})", config.id, config.server_type);
        println!("  Name: {}", config.name);
        println!("  Status: {}", status);
        println!("  Enabled: {}", config.enabled);
        println!("  Endpoint: {}", endpoint);
    }

    Ok(())
}

pub(crate) async fn set_default_model(model_id: &str) -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    config_service
        .set_config("ai.default_models.primary", model_id)
        .await?;
    config_service
        .set_config("ai.agent_model_defaults.mode", model_id)
        .await?;

    println!("Default model set to: {}", model_id);

    // Short-lived management process: the sync loop never runs here, so push
    // the change directly (no-op when logged out).
    crate::account::build_management_account_runtime()
        .push_settings_after_local_change()
        .await;
    Ok(())
}

pub(crate) async fn set_mcp_server_enabled(server_id: &str, enabled: bool) -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let mut config = mcp_service
        .config_service()
        .get_server_config(server_id)
        .await?
        .ok_or_else(|| anyhow!("MCP server not found: {}", server_id))?;
    config.enabled = enabled;
    mcp_service
        .config_service()
        .save_server_config(&config)
        .await?;

    println!(
        "MCP server {} {}.",
        server_id,
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// User-facing input for `bitfun mcp add`.
///
/// All fields are optional so the CLI can pre-fill any subset via flags and
/// resolve the rest through the three-step wizard.
pub(crate) struct McpAddInput {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub command: Option<String>,
    pub url: Option<String>,
    pub non_interactive: bool,
}

/// RAII guard that disables crossterm raw mode on drop, so an early return or
/// panic in the wizard cannot leave the terminal stuck in raw mode.
struct RawModeGuard;
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Add an MCP server through the three-step wizard.
///
/// Step 1 collects the server name (also used as id). Step 2 lets the user
/// pick the type — `local` (stdio) or `remote` (streamable-http) — using the
/// up/down arrow keys. Step 3 collects the launch command (local) or URL
/// (remote); for local servers, the command string is split on whitespace
/// into `command + args`, matching the behavior of the TUI MCP add dialog.
/// Other `MCPServerConfig` fields use sensible defaults and are validated
/// through the shared `MCPServerConfig::validate` contract before being
/// persisted via `MCPConfigService::save_server_config`.
pub(crate) async fn add_mcp_server(input: McpAddInput) -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;

    println!("Add MCP Server (1/3) — Name");
    let name = read_required_field(
        "Server name (also used as id; no spaces)",
        input.name.as_deref(),
        input.non_interactive,
    )?;
    if name.contains(' ') {
        return Err(anyhow!("Server name cannot contain spaces"));
    }

    println!("Add MCP Server (2/3) — Type");
    let is_local = select_server_type(input.r#type.as_deref(), input.non_interactive)?;
    let server_type = if is_local {
        bitfun_core::service::mcp::MCPServerType::Local
    } else {
        bitfun_core::service::mcp::MCPServerType::Remote
    };
    let transport = if is_local {
        bitfun_core::service::mcp::MCPServerTransport::Stdio
    } else {
        bitfun_core::service::mcp::MCPServerTransport::StreamableHttp
    };

    println!("Add MCP Server (3/3) — Connection");
    let (command, args, url) = if is_local {
        let raw = read_required_field(
            "Command (e.g. `npx -y @modelcontextprotocol/server-xxx`)",
            input.command.as_deref(),
            input.non_interactive,
        )?;
        let (command_value, args_value) = parse_local_command(&raw)?;
        (Some(command_value), args_value, None)
    } else {
        let url_value = read_required_field(
            "URL (e.g. `https://mcp.example.com/mcp`)",
            input.url.as_deref(),
            input.non_interactive,
        )?;
        (None, Vec::new(), Some(url_value))
    };

    let config = bitfun_core::service::mcp::MCPServerConfig {
        id: name.clone(),
        name: name.clone(),
        server_type,
        transport: Some(transport),
        command,
        args,
        env: std::collections::HashMap::new(),
        working_directory: None,
        inherit_parent_environment: None,
        headers: std::collections::HashMap::new(),
        url,
        auto_start: true,
        enabled: true,
        location: bitfun_core::service::mcp::ConfigLocation::User,
        capabilities: Vec::new(),
        settings: std::collections::HashMap::new(),
        oauth: None,
        oauth_enabled: None,
        xaa: None,
        timeouts: Default::default(),
    };

    config
        .validate()
        .map_err(|error| anyhow!("Invalid MCP server config: {}", error))?;

    if mcp_service
        .config_service()
        .get_server_config(&config.id)
        .await?
        .is_some()
    {
        return Err(anyhow!("MCP server already exists: {}", config.id));
    }

    mcp_service
        .config_service()
        .save_server_config(&config)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;

    println!("MCP server '{}' added.", name);
    println!(
        "Run `bitfun mcp list` to verify, `bitfun mcp disable {}` to toggle.",
        name
    );
    Ok(())
}

/// Returns `true` for local, `false` for remote. Uses up/down arrow keys
/// (also left/right, vim-style h/l and j/k) when no flag pre-fills the choice;
/// requires an explicit flag value in `--non-interactive` mode.
fn select_server_type(pre_filled: Option<&str>, non_interactive: bool) -> Result<bool> {
    if let Some(value) = pre_filled {
        let trimmed = value.trim();
        return match trimmed {
            "local" => Ok(true),
            "remote" => Ok(false),
            other => Err(anyhow!("Type must be `local` or `remote`, got `{}`", other)),
        };
    }
    if non_interactive {
        return Err(anyhow!(
            "Server type is required in non-interactive mode (pass `--type local` or `--type remote`)"
        ));
    }

    use crossterm::event::{self, KeyCode, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode().context("Failed to enable raw mode")?;
    let _guard = RawModeGuard;

    let mut is_local = true;
    let mut stdout = std::io::stdout();
    render_type_prompt(&mut stdout, is_local)?;

    loop {
        let event = match event::read() {
            Ok(event) => event,
            Err(error) => return Err(anyhow!("Failed to read key event: {}", error)),
        };
        let key_event = match event {
            crossterm::event::Event::Key(key_event) => key_event,
            _ => continue,
        };
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('k') => {
                is_local = true;
                render_type_prompt(&mut stdout, is_local)?;
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('j') => {
                is_local = false;
                render_type_prompt(&mut stdout, is_local)?;
            }
            KeyCode::Enter => break,
            KeyCode::Esc => {
                println!();
                return Err(anyhow!("Cancelled by user"));
            }
            _ => {}
        }
    }

    println!();
    let _ = disable_raw_mode();
    Ok(is_local)
}

fn parse_local_command(value: &str) -> Result<(String, Vec<String>)> {
    let parts = split_command_line(value)
        .ok_or_else(|| anyhow!("Command contains an unclosed quote or invalid escaping"))?;
    let mut parts = parts.into_iter();
    let command = parts
        .next()
        .filter(|command| !command.is_empty())
        .ok_or_else(|| anyhow!("Command cannot be empty"))?;
    Ok((command, parts.collect()))
}

#[cfg(not(windows))]
fn split_command_line(value: &str) -> Option<Vec<String>> {
    shlex::split(value)
}

#[cfg(windows)]
fn split_command_line(value: &str) -> Option<Vec<String>> {
    let parts = winsplit::split(value);
    if parts.is_empty() || has_unclosed_windows_quote(value) {
        None
    } else {
        Some(parts)
    }
}

#[cfg(windows)]
fn has_unclosed_windows_quote(value: &str) -> bool {
    let mut quoted = false;
    let mut backslashes = 0usize;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' if backslashes % 2 == 0 => {
                quoted = !quoted;
                backslashes = 0;
            }
            _ => backslashes = 0,
        }
    }
    quoted
}

fn render_type_prompt(stdout: &mut std::io::Stdout, is_local: bool) -> Result<()> {
    use std::io::Write;
    // `\r\x1b[2K` moves to column 0 and clears the current line so the prompt
    // can be re-rendered in place while the user cycles between options.
    write!(stdout, "\r\x1b[2K")?;
    let label = if is_local {
        "local (stdio)"
    } else {
        "remote (streamable-http)"
    };
    let marker = |selected: bool| if selected { "›" } else { " " };
    write!(
        stdout,
        "  Type: [{}] {} local (stdio)  {} remote (streamable-http)  ↑/↓ select, Enter confirm, Esc cancel",
        label,
        marker(is_local),
        marker(!is_local),
    )?;
    stdout.flush()?;
    Ok(())
}

fn read_line_trimmed() -> Result<String> {
    let mut buffer = String::new();
    let bytes_read = std::io::stdin()
        .read_line(&mut buffer)
        .context("Failed to read from stdin")?;
    if bytes_read == 0 {
        return Err(anyhow!("Unexpected end of input (stdin closed)"));
    }
    Ok(buffer.trim_end_matches(['\r', '\n']).to_string())
}

fn read_required_field(
    label: &str,
    pre_filled: Option<&str>,
    non_interactive: bool,
) -> Result<String> {
    if let Some(value) = pre_filled {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("{} cannot be empty", label));
        }
        return Ok(trimmed.to_string());
    }
    if non_interactive {
        return Err(anyhow!(
            "{} is required (pass a flag or run without --non-interactive)",
            label
        ));
    }
    print!("  {}: ", label);
    std::io::stdout()
        .flush()
        .context("Failed to flush stdout")?;
    let value = read_line_trimmed()?;
    if value.is_empty() {
        return Err(anyhow!("{} cannot be empty", label));
    }
    Ok(value)
}
pub(crate) async fn print_mcp_json_config() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let snapshot = mcp_service.config_service().load_mcp_json_config().await?;
    println!("{}", snapshot.json_config);
    Ok(())
}

pub(crate) async fn run_mcp_import(command: crate::mcp_import::McpImportCommand) -> Result<()> {
    let _config_service = ensure_global_config_service().await?;
    crate::mcp_import::execute(command).await
}

fn validate_usage_session_id(session_id: &str) -> Result<()> {
    bitfun_agent_runtime::session_control::validate_session_id(session_id)
        .map_err(anyhow::Error::msg)
}

pub(crate) async fn print_usage_report(session_id: Option<&str>) -> Result<()> {
    if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
        validate_usage_session_id(session_id)?;
    }
    let workspace_path = std::env::current_dir().context("Failed to resolve current directory")?;
    let runtime = crate::initialize_core_services(
        &workspace_path,
        crate::runtime::approval::CliApprovalPolicy::Reject,
        crate::BootstrapProfile::Management,
    )
    .await?;
    let resolved_session_id = match session_id {
        Some(session_id) if !session_id.trim().is_empty() => session_id.to_string(),
        _ => runtime
            .agent_runtime()
            .list_sessions(bitfun_runtime_ports::AgentSessionListRequest {
                workspace_path: workspace_path.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await?
            .first()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| anyhow!("No history sessions for current project"))?,
    };

    let report = runtime
        .agent_runtime()
        .generate_session_usage(AgentSessionUsageRequest {
            session_id: resolved_session_id,
            workspace_path: Some(workspace_path.to_string_lossy().to_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            include_hidden_subagents: true,
        })
        .await
        .map_err(|error| anyhow!(error.into_message()))?;

    println!("{}", render_usage_report_markdown(&report));
    Ok(())
}

/// Aggregated statistics across multiple sessions, mirroring OpenCode's stats command.
struct AggregatedStats {
    total_sessions: usize,
    total_messages: usize,
    total_turns: usize,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cached_tokens: u64,
    /// Number of sessions that actually reported token data.
    /// If this is 0, the token totals are "N/A" rather than "0".
    sessions_with_token_data: usize,
    tool_usage: std::collections::BTreeMap<String, u64>,
    model_usage: std::collections::BTreeMap<String, ModelUsageEntry>,
    earliest_ms: u64,
    latest_ms: u64,
    days: u32,
}

#[derive(Default)]
struct ModelUsageEntry {
    call_count: u64,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    /// Whether any token record was available for this model.
    has_token_data: bool,
}

pub(crate) async fn print_stats_report(
    days: Option<u32>,
    tools_limit: Option<usize>,
    models_flag: Option<usize>,
    project_filter: Option<String>,
) -> Result<()> {
    let current_dir =
        std::env::current_dir().context("Failed to resolve current directory")?;
    let runtime = crate::initialize_core_services(
        &current_dir,
        crate::runtime::approval::CliApprovalPolicy::Reject,
        crate::BootstrapProfile::Management,
    )
    .await?;

    // Determine which workspace path(s) to query.
    // - No --project flag or empty string: current directory (default).
    // - --project <path>: use that path as the workspace.
    // - --project all: scan all projects under ~/.bitfun/projects/.
    let workspace_paths: Vec<String> = match project_filter.as_deref() {
        Some("all") => {
            let path_manager = try_get_path_manager_arc()
                .map_err(|error| anyhow!(error.to_string()))?;
            let projects_root = path_manager.projects_root();
            if !projects_root.exists() {
                println!("No projects found under {}", projects_root.display());
                return Ok(());
            }
            let mut paths = Vec::new();
            for entry in std::fs::read_dir(&projects_root)
                .with_context(|| format!("Failed to read projects dir: {}", projects_root.display()))?
            {
                let entry = entry?;
                let sessions_dir = entry.path().join("sessions");
                if sessions_dir.is_dir() {
                    paths.push(sessions_dir.to_string_lossy().to_string());
                }
            }
            if paths.is_empty() {
                println!("No project sessions found under {}", projects_root.display());
                return Ok(());
            }
            paths
        }
        Some(p) if !p.trim().is_empty() => vec![p.to_string()],
        _ => vec![current_dir.to_string_lossy().to_string()],
    };

    // List sessions for each workspace path and aggregate.
    // Track which workspace_path each session belongs to for usage queries.
    let mut sessions: Vec<(bitfun_runtime_ports::AgentSessionSummary, String)> = Vec::new();
    for ws_path in &workspace_paths {
        let result = runtime
            .agent_runtime()
            .list_sessions(bitfun_runtime_ports::AgentSessionListRequest {
                workspace_path: ws_path.clone(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow!(error.into_message()))?;
        for s in result {
            sessions.push((s, ws_path.clone()));
        }
    }

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    // Apply --days filter (cutoff based on last_active_at_ms)
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let cutoff_ms = match days {
        Some(0) => {
            // 0 means today: midnight of current day
            let secs = (now_ms / 1000) as i64;
            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
                .unwrap_or_default()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default()
                .and_utc()
                .timestamp_millis() as u64;
            dt
        }
        Some(d) => now_ms.saturating_sub((d as u64) * 24 * 60 * 60 * 1000),
        None => 0,
    };

    let mut filtered: Vec<&(bitfun_runtime_ports::AgentSessionSummary, String)> = sessions
        .iter()
        .filter(|(s, _)| s.last_active_at_ms >= cutoff_ms)
        .collect();

    if filtered.is_empty() {
        println!(
            "No sessions matching the filter (days={:?}).",
            days
        );
        return Ok(());
    }

    if filtered.len() > 200 {
        eprintln!(
            "Aggregating {} sessions; this may take a moment...",
            filtered.len()
        );
    }

    // Sort by last_active_at_ms ascending so earliest/latest are easy to compute
    filtered.sort_by_key(|(s, _)| s.last_active_at_ms);

    let mut stats = AggregatedStats {
        total_sessions: filtered.len(),
        total_messages: 0,
        total_turns: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cached_tokens: 0,
        sessions_with_token_data: 0,
        tool_usage: std::collections::BTreeMap::new(),
        model_usage: std::collections::BTreeMap::new(),
        earliest_ms: filtered.first().map(|(s, _)| s.last_active_at_ms).unwrap_or(now_ms),
        latest_ms: filtered.last().map(|(s, _)| s.last_active_at_ms).unwrap_or(now_ms),
        days: 0,
    };

    // For each session, fetch usage report and aggregate
    for (session, ws_path) in &filtered {
        let report = runtime
            .agent_runtime()
            .generate_session_usage(AgentSessionUsageRequest {
                session_id: session.session_id.clone(),
                workspace_path: Some(ws_path.clone()),
                remote_connection_id: None,
                remote_ssh_host: None,
                include_hidden_subagents: true,
            })
            .await;

        let report = match report {
            Ok(r) => r,
            Err(error) => {
                eprintln!(
                    "Warning: could not generate usage for session {}: {}",
                    session.session_id,
                    error.into_message()
                );
                continue;
            }
        };

        stats.total_turns += report.scope.turn_count;

        // Track whether this session reported any token data.
        let session_has_tokens = report.tokens.input_tokens.is_some()
            || report.tokens.output_tokens.is_some()
            || report.tokens.cached_tokens.is_some();
        if session_has_tokens {
            stats.sessions_with_token_data += 1;
        }
        if let Some(input) = report.tokens.input_tokens {
            stats.total_input_tokens += input;
        }
        if let Some(output) = report.tokens.output_tokens {
            stats.total_output_tokens += output;
        }
        if let Some(cached) = report.tokens.cached_tokens {
            stats.total_cached_tokens += cached;
        }

        for tool in &report.tools {
            *stats.tool_usage.entry(tool.tool_name.clone()).or_insert(0) += tool.call_count;
        }

        for model in &report.models {
            let entry = stats
                .model_usage
                .entry(model.model_id.clone())
                .or_default();
            entry.call_count += model.call_count;
            let model_has_tokens = model.input_tokens.is_some()
                || model.output_tokens.is_some()
                || model.cached_tokens.is_some();
            if model_has_tokens {
                entry.has_token_data = true;
            }
            if let Some(v) = model.input_tokens {
                entry.input_tokens += v;
            }
            if let Some(v) = model.output_tokens {
                entry.output_tokens += v;
            }
            if let Some(v) = model.cached_tokens {
                entry.cached_tokens += v;
            }
        }
    }

    // Approximate message count from turn count (each turn ~ 1 user + 1 assistant)
    stats.total_messages = stats.total_turns * 2;

    // Compute days from date range
    let range_ms = stats.latest_ms.saturating_sub(stats.earliest_ms);
    stats.days = match days {
        Some(0) => 1,
        Some(d) => d,
        None => std::cmp::max(1, ((range_ms / (24 * 60 * 60 * 1000)) + 1) as u32),
    };

    render_stats(&stats, tools_limit, models_flag);
    Ok(())
}

fn render_stats(stats: &AggregatedStats, tools_limit: Option<usize>, models_flag: Option<usize>) {
    const WIDTH: usize = 56;

    fn render_row(label: &str, value: String) -> String {
        let available = WIDTH.saturating_sub(1);
        let padding = available
            .saturating_sub(label.chars().count())
            .saturating_sub(value.chars().count());
        format!("│{}{}{} │", label, " ".repeat(padding), value)
    }

    fn format_number(n: u64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    let border_top = "┌".to_string() + &"─".repeat(WIDTH) + "┐";
    let border_mid = "├".to_string() + &"─".repeat(WIDTH) + "┤";
    let border_bot = "└".to_string() + &"─".repeat(WIDTH) + "┘";

    // Overview section
    println!("{}", border_top);
    println!("│{: ^WIDTH$}│", "OVERVIEW");
    println!("{}", border_mid);
    println!("{}", render_row("Sessions", stats.total_sessions.to_string()));
    println!("{}", render_row("Messages", stats.total_messages.to_string()));
    println!("{}", render_row("Turns", stats.total_turns.to_string()));
    println!("{}", render_row("Days", stats.days.to_string()));
    println!("{}", border_bot);
    println!();

    // Cost & Tokens section (BitFun has no cost tracking, so we show tokens only)
    println!("{}", border_top);
    println!("│{: ^WIDTH$}│", "TOKENS");
    println!("{}", border_mid);
    if stats.sessions_with_token_data == 0 {
        // No session reported token data — show N/A instead of misleading "0"
        println!("{}", render_row("Source", "unavailable".to_string()));
        println!("{}", render_row("Input", "N/A".to_string()));
        println!("{}", render_row("Output", "N/A".to_string()));
        println!("{}", render_row("Cached", "N/A".to_string()));
        println!("{}", render_row("Total", "N/A".to_string()));
        println!(
            "{}",
            render_row("Avg/Session", "N/A".to_string())
        );
    } else {
        println!(
            "{}",
            render_row("Sessions w/ data", stats.sessions_with_token_data.to_string())
        );
        println!(
            "{}",
            render_row("Input", format_number(stats.total_input_tokens))
        );
        println!(
            "{}",
            render_row("Output", format_number(stats.total_output_tokens))
        );
        println!(
            "{}",
            render_row(
                "Cached",
                format_number(stats.total_cached_tokens)
            )
        );
        let total_tokens = stats.total_input_tokens
            + stats.total_output_tokens
            + stats.total_cached_tokens;
        println!("{}", render_row("Total", format_number(total_tokens)));
        if stats.total_sessions > 0 {
            let avg = total_tokens / stats.total_sessions as u64;
            println!(
                "{}",
                render_row("Avg/Session", format_number(avg))
            );
        }
    }
    println!("{}", border_bot);
    println!();

    // Model Usage section (only if --models flag is provided)
    if let Some(model_limit) = models_flag {
        if !stats.model_usage.is_empty() {
            let mut sorted: Vec<_> = stats.model_usage.iter().collect();
            sorted.sort_by(|a, b| b.1.call_count.cmp(&a.1.call_count));
            let display: Vec<_> = if model_limit == 0 {
                sorted.clone()
            } else {
                sorted.into_iter().take(model_limit).collect()
            };

            println!("{}", border_top);
            println!("│{: ^WIDTH$}│", "MODEL USAGE");
            println!("{}", border_mid);
            for (model, usage) in &display {
                let model_label = if model.len() > 54 {
                    format!("{}..", &model[..52])
                } else {
                    model.to_string()
                };
                println!("│ {: <WIDTH$}│", model_label);
                println!("{}", render_row("  Calls", usage.call_count.to_string()));
                let input_str = if usage.has_token_data {
                    format_number(usage.input_tokens)
                } else {
                    "N/A".to_string()
                };
                let output_str = if usage.has_token_data {
                    format_number(usage.output_tokens)
                } else {
                    "N/A".to_string()
                };
                let cached_str = if usage.has_token_data {
                    format_number(usage.cached_tokens)
                } else {
                    "N/A".to_string()
                };
                println!("{}", render_row("  Input", input_str));
                println!("{}", render_row("  Output", output_str));
                println!("{}", render_row("  Cached", cached_str));
                println!("{}", border_mid);
            }
            println!("{}", border_bot);
            println!();
        }
    }

    // Tool Usage section
    if !stats.tool_usage.is_empty() {
        let mut sorted: Vec<_> = stats.tool_usage.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        let display: Vec<_> = match tools_limit {
            Some(limit) => sorted.into_iter().take(limit).collect(),
            None => sorted,
        };

        let total_tool_calls: u64 = display.iter().map(|(_, c)| **c).sum();
        let max_count: u64 = display.iter().map(|(_, c)| **c).max().unwrap_or(1);

        println!("{}", border_top);
        println!("│{: ^WIDTH$}│", "TOOL USAGE");
        println!("{}", border_mid);
        for (tool, count) in display.iter() {
            let count_val = **count;
            let bar_len = std::cmp::max(1, ((count_val as f64) / (max_count as f64) * 20.0) as usize);
            let bar = "█".repeat(bar_len);
            let percentage = if total_tool_calls > 0 {
                format!("{:.1}", count_val as f64 / total_tool_calls as f64 * 100.0)
            } else {
                "0.0".to_string()
            };
            let max_tool_len = 18;
            let tool_name = if tool.len() > max_tool_len {
                format!("{}..", &tool[..max_tool_len - 2])
            } else {
                tool.to_string()
            };
            let content = format!(
                " {: <width$} {: <20} {: >3} ({: >5}%)",
                tool_name,
                bar,
                count,
                percentage,
                width = max_tool_len
            );
            let padding = WIDTH.saturating_sub(content.chars().count()).saturating_sub(1);
            println!("│{}{} │", content, " ".repeat(padding));
        }
        println!("{}", border_bot);
    }
}

pub(crate) async fn print_plugins() -> Result<()> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let path_manager = try_get_path_manager_arc().map_err(|error| anyhow!(error.to_string()))?;
    let snapshot = refresh_managed_plugin_sources(&workspace)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    println!(
        "User package root: {}",
        crate::plugin_diagnostics::escape_terminal_text(
            &path_manager.user_plugins_dir().to_string_lossy()
        )
    );
    println!(
        "Workspace package root: {}",
        crate::plugin_diagnostics::escape_terminal_text(
            &path_manager
                .project_plugins_dir(&workspace)
                .to_string_lossy()
        )
    );
    println!();
    print_plugin_snapshot(&snapshot);
    Ok(())
}

pub(crate) async fn set_plugin_trust(
    package_id: &str,
    decision: ManagedPluginTrustDecision,
) -> Result<()> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let snapshot = set_managed_plugin_trust(&workspace, package_id, decision)
        .await
        .map_err(|error| {
            anyhow!(crate::plugin_diagnostics::escape_terminal_text(
                &error.to_string()
            ))
        })?;
    let package = snapshot
        .packages
        .iter()
        .find(|package| package.package_id == package_id)
        .ok_or_else(|| anyhow!("Managed plugin package disappeared during trust update"))?;
    let trust_epoch = snapshot
        .trust_epoch
        .ok_or_else(|| anyhow!("Managed plugin source review epoch is unavailable after update"))?;
    println!(
        "Plugin package {} {} is now {} (source review epoch {}).",
        crate::plugin_diagnostics::escape_terminal_text(&package.package_id),
        crate::plugin_diagnostics::escape_terminal_text(&package.version),
        plugin_trust_label(package.trust_level),
        trust_epoch
    );
    println!(
        "Source: {}",
        crate::plugin_diagnostics::escape_terminal_text(&package.source_path)
    );
    println!(
        "Adapter: {}",
        crate::plugin_diagnostics::escape_terminal_text(&package.adapter)
    );
    println!("Content hash: {}", package.content_hash);
    match decision {
        ManagedPluginTrustDecision::ApproveSource => {
            println!("The current manifest and declared files are approved for source review.");
        }
        ManagedPluginTrustDecision::Denied => {
            println!("The current manifest and declared files are denied for this workspace.");
        }
        ManagedPluginTrustDecision::Revoked => {
            println!("The previous approval has been revoked for this workspace.");
        }
    }
    println!("Execution remains unavailable; this action does not enable the package.");
    Ok(())
}

pub(crate) async fn activate_plugin(package_id: &str, confirm: Option<&str>) -> Result<()> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let view = if let Some(content_hash) = confirm {
        activate_managed_plugin(&workspace, package_id, Some(content_hash)).await
    } else {
        preview_managed_plugin_activation(&workspace, package_id).await
    }
    .map_err(|error| {
        let diagnostic = crate::plugin_diagnostics::escape_terminal_text(&error.to_string());
        if confirm.is_some() {
            anyhow!(
                "{}\nRe-run `bitfun plugins activate {}` to preview the current content, then confirm with the new content hash.",
                diagnostic,
                crate::plugin_diagnostics::escape_terminal_text(package_id)
            )
        } else {
            anyhow!(diagnostic)
        }
    })?;

    print_plugin_activation(&view, confirm.is_none());
    if confirm.is_none() {
        println!();
        println!(
            "No activation state changed. Re-run `bitfun plugins activate {} --confirm {}` to confirm this exact package content.",
            crate::plugin_diagnostics::escape_terminal_text(package_id),
            crate::plugin_diagnostics::escape_terminal_text(&view.content_hash)
        );
    }
    Ok(())
}

pub(crate) async fn deactivate_plugin(package_id: &str) -> Result<()> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let result = deactivate_managed_plugin(&workspace, package_id)
        .await
        .map_err(|error| {
            let diagnostic = crate::plugin_diagnostics::escape_terminal_text(&error.to_string());
            if matches!(
                error,
                ManagedPluginSourceError::DeactivationPersistenceUncertain { .. }
            ) {
                anyhow!(
                    "{diagnostic}\nThe saved state may already be cleared. Retry `bitfun plugins deactivate {}` to confirm the result; the operation is idempotent.",
                    crate::plugin_diagnostics::escape_terminal_text(package_id)
                )
            } else {
                anyhow!(diagnostic)
            }
        })?;
    match result {
        ManagedPluginDeactivationResult::Deactivated {
            package_id,
            diagnostics,
        } => {
            let package_id = crate::plugin_diagnostics::escape_terminal_text(&package_id);
            println!("Plugin package {package_id} was deactivated.");
            print_deactivation_diagnostics(&diagnostics);
        }
        ManagedPluginDeactivationResult::ResidualActivationCleared {
            package_id,
            current_package_available,
            diagnostics,
        } => {
            let package_id = crate::plugin_diagnostics::escape_terminal_text(&package_id);
            match current_package_available {
                Some(true) => println!(
                    "Plugin package {package_id} previous source's saved activation state was cleared; the current package was not active."
                ),
                Some(false) => println!(
                    "Plugin package {package_id} is unavailable; its saved activation state was cleared."
                ),
                None => println!(
                    "Plugin package {package_id} saved activation state was cleared; current package availability could not be determined."
                ),
            }
            print_deactivation_diagnostics(&diagnostics);
        }
        ManagedPluginDeactivationResult::AlreadyInactive {
            package_id,
            current_package_available,
            diagnostics,
        } => {
            let package_id = crate::plugin_diagnostics::escape_terminal_text(&package_id);
            match current_package_available {
                Some(true) => println!("Plugin package {package_id} was already inactive."),
                Some(false) => println!(
                    "Plugin package {package_id} is unavailable and has no saved activation state."
                ),
                None => println!(
                    "Plugin package {package_id} has no saved activation state; current package availability could not be determined."
                ),
            }
            print_deactivation_diagnostics(&diagnostics);
        }
    }
    println!("No plugin code or candidate effect was executed.");
    Ok(())
}

fn print_deactivation_diagnostics(diagnostics: &[ManagedPluginSourceIssue]) {
    for diagnostic in diagnostics {
        println!("- {}", render_plugin_source_issue(diagnostic));
    }
}

fn render_plugin_source_issue(issue: &ManagedPluginSourceIssue) -> String {
    format!(
        "[{}:{}] {}: {}",
        if issue.is_error { "error" } else { "warn" },
        crate::plugin_diagnostics::escape_terminal_text(&issue.code),
        crate::plugin_diagnostics::escape_terminal_text(&issue.source_path),
        crate::plugin_diagnostics::escape_terminal_text(&issue.message)
    )
}

fn print_plugin_activation(view: &ManagedPluginActivationView, preview: bool) {
    println!(
        "Plugin activation {}",
        if preview { "preview" } else { "result" }
    );
    println!();
    println!(
        "Package: {} {}",
        crate::plugin_diagnostics::escape_terminal_text(&view.package_id),
        crate::plugin_diagnostics::escape_terminal_text(&view.version)
    );
    println!(
        "Adapter: {}",
        crate::plugin_diagnostics::escape_terminal_text(&view.adapter)
    );
    println!("Content hash: {}", view.content_hash);
    println!(
        "Custom tool candidates: {}",
        if view.provider_candidates_supported {
            "supported"
        } else {
            "not found"
        }
    );
    println!(
        "Permission required before use: {}",
        if view.permission_required {
            "yes"
        } else {
            "no"
        }
    );
    println!("Entries: {}", view.entry_ids.len());
    for entry_id in &view.entry_ids {
        println!(
            "- {}",
            crate::plugin_diagnostics::escape_terminal_text(entry_id)
        );
    }
    println!(
        "{}: {}",
        if preview {
            "Declared candidates requiring permission"
        } else {
            "Candidates requiring permission"
        },
        view.candidates.len()
    );
    for candidate in &view.candidates {
        println!(
            "- {} -> {} (risk: {})",
            crate::plugin_diagnostics::escape_terminal_text(&candidate.entry_id),
            crate::plugin_diagnostics::escape_terminal_text(&candidate.target),
            candidate.risk_level
        );
    }
    for diagnostic in &view.diagnostics {
        println!(
            "- [diagnostic] {}",
            crate::plugin_diagnostics::escape_terminal_text(diagnostic)
        );
    }
    println!("Plugin code was not executed and no tool was registered.");
}

fn print_plugin_snapshot(snapshot: &ManagedPluginSourceSnapshot) {
    let approved_count = snapshot
        .packages
        .iter()
        .filter(|package| package.trust_level == ManagedPluginTrustLevel::SourceApproved)
        .count();
    let warning_count = snapshot
        .issues
        .iter()
        .filter(|issue| !issue.is_error)
        .count();
    let error_count = snapshot
        .issues
        .iter()
        .filter(|issue| issue.is_error)
        .count();
    println!("Managed plugin packages");
    println!();
    println!(
        "{}",
        crate::plugin_diagnostics::render_plugin_source_summary(
            snapshot.packages.len(),
            approved_count,
            warning_count,
            error_count,
        )
    );
    for package in &snapshot.packages {
        println!(
            "- {} {} ({}, {})",
            crate::plugin_diagnostics::escape_terminal_text(&package.package_id),
            crate::plugin_diagnostics::escape_terminal_text(&package.version),
            package.source_scope,
            if snapshot.discovery_complete {
                plugin_trust_label(package.trust_level)
            } else {
                "review state unavailable"
            },
        );
        println!(
            "  Source: {}",
            crate::plugin_diagnostics::escape_terminal_text(&package.source_path)
        );
        println!(
            "  Adapter: {}",
            crate::plugin_diagnostics::escape_terminal_text(&package.adapter)
        );
        println!("  Content hash: {}", package.content_hash);
        println!(
            "  Activation: {}",
            if !snapshot.discovery_complete {
                "unknown; source discovery is incomplete"
            } else if package.activated {
                "active for candidate projection; plugin code is not executed"
            } else {
                "inactive; source review does not activate this package"
            }
        );
    }
    for issue in &snapshot.issues {
        println!("- {}", render_plugin_source_issue(issue));
    }
    println!(
        "{}",
        crate::plugin_diagnostics::render_source_review_epoch(snapshot.trust_epoch)
    );
    println!(
        "Activation epoch: {}",
        snapshot
            .activation_epoch
            .map(|epoch| epoch.to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    );
}

fn plugin_trust_label(trust_level: ManagedPluginTrustLevel) -> &'static str {
    match trust_level {
        ManagedPluginTrustLevel::Unknown => "unreviewed",
        ManagedPluginTrustLevel::SourceApproved => "source-approved",
        ManagedPluginTrustLevel::Denied => "denied",
        ManagedPluginTrustLevel::Revoked => "revoked",
        _ => "other",
    }
}

pub(crate) async fn print_mcp_config_summary() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service)
        .map_err(|error| anyhow!(error.to_string()))?;
    let configs = mcp_service.config_service().load_all_configs().await?;

    println!("MCP configuration summary");
    println!();
    println!(
        "{}",
        crate::plugin_diagnostics::render_mcp_configuration_count(configs.len())
    );
    println!("This command does not probe server readiness.");
    Ok(())
}

pub(crate) async fn print_doctor(product_runtime: &ProductRuntimeParts) -> Result<bool> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let config_dir = crate::config::CliConfig::config_dir()?;
    let config_service = ensure_global_config_service().await?;
    let models = config_service.get_ai_models().await?;
    let agent_registry = get_agent_registry();
    let external_source_error =
        bitfun_core::external_sources::ensure_external_source_workspace_snapshot(Some(&workspace))
            .await
            .err();
    let modes = agent_registry
        .get_modes_info_for_workspace(Some(&workspace), true)
        .await;
    let subagents = agent_registry
        .get_subagents_info(Some(workspace.as_path()))
        .await;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let mcp_configs = mcp_service.config_service().load_all_configs().await?;
    let plugin_sources = refresh_managed_plugin_sources(&workspace)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let approved_plugin_count = plugin_sources
        .packages
        .iter()
        .filter(|package| package.trust_level == ManagedPluginTrustLevel::SourceApproved)
        .count();
    let active_plugin_count = plugin_sources
        .packages
        .iter()
        .filter(|package| package.activated)
        .count();
    let plugin_warning_count = plugin_sources
        .issues
        .iter()
        .filter(|issue| !issue.is_error)
        .count();
    let plugin_error_count = plugin_sources
        .issues
        .iter()
        .filter(|issue| issue.is_error)
        .count();
    let plugin_sources_ready =
        crate::plugin_diagnostics::plugin_source_check_passes(plugin_error_count);

    println!("BitFun CLI doctor");
    println!();
    println!(
        "[ok] Product runtime: {} assembly-ready",
        product_runtime.plan().profile().id()
    );
    println!("[ok] Runtime capability registrations: complete");
    println!("[info] Execution owner: bitfun-core compatibility");
    match product_runtime.plugin_runtime().availability() {
        PluginRuntimeAvailability::Disabled { reason } => {
            println!("[info] Plugin runtime: disabled ({reason})");
        }
        PluginRuntimeAvailability::ProjectionOnly { reason } => {
            println!("[info] Plugin runtime: projection-only ({reason})");
        }
        PluginRuntimeAvailability::Unavailable { reason } => {
            println!("[info] Plugin runtime: unavailable ({reason})");
        }
        PluginRuntimeAvailability::Available => {
            println!("[ok] Plugin runtime: available");
        }
        _ => {
            println!("[info] Plugin runtime: unknown");
        }
    }
    println!("[ok] Workspace: {}", workspace.display());
    println!("[ok] Config directory: {}", config_dir.display());
    println!("[ok] Agent modes: {}", modes.len());
    println!("[ok] Subagents: {}", subagents.len());
    if let Some(error) = external_source_error.as_deref() {
        println!(
            "[warn] External agent sources were not refreshed: {}",
            crate::plugin_diagnostics::escape_terminal_text(error)
        );
    } else {
        println!("[ok] External agent sources: refreshed");
    }
    println!(
        "[ok] AI models: {} total, {} enabled",
        models.len(),
        models.iter().filter(|m| m.enabled).count()
    );
    println!("[ok] MCP configuration entries: {}", mcp_configs.len());
    println!(
        "{}",
        crate::plugin_diagnostics::render_plugin_source_summary(
            plugin_sources.packages.len(),
            approved_plugin_count,
            plugin_warning_count,
            plugin_error_count,
        )
    );
    if plugin_sources.discovery_complete {
        println!(
            "[ok] Managed plugin source integrity checked; {} active. Candidate projection was not probed.",
            active_plugin_count
        );
    } else {
        println!(
            "[error] Managed plugin source scan is incomplete; review and activation status are unavailable. Candidate projection was not probed."
        );
    }
    for issue in plugin_sources.issues.iter().take(10) {
        println!("  - {}", render_plugin_source_issue(issue));
    }
    if plugin_sources.issues.len() > 10 {
        println!(
            "  - {} additional plugin diagnostics omitted",
            plugin_sources.issues.len() - 10
        );
    }
    println!();
    let doctor_checks_ready = external_source_error.is_none() && plugin_sources_ready;
    if !plugin_sources_ready {
        println!("Doctor checks found plugin source errors.");
    } else if external_source_error.is_some() {
        println!("Doctor checks completed with an external agent source warning.");
    } else if plugin_warning_count > 0 {
        println!("Doctor checks completed with plugin warnings.");
    } else {
        println!("Doctor checks passed.");
    }
    Ok(doctor_checks_ready)
}

#[cfg(test)]
mod tests {
    use super::{parse_local_command, select_server_type, validate_usage_session_id};

    #[test]
    fn local_command_parser_preserves_quoted_arguments() {
        let (command, args) =
            parse_local_command(r#"node "path with spaces/server.js" --flag "hello world""#)
                .expect("parse quoted command");

        assert_eq!(command, "node");
        assert_eq!(
            args,
            ["path with spaces/server.js", "--flag", "hello world"]
        );
    }

    #[test]
    fn local_command_parser_rejects_unclosed_quotes() {
        let error =
            parse_local_command(r#"node "unterminated"#).expect_err("unclosed quotes must fail");

        assert!(error.to_string().contains("unclosed quote"), "{error}");
    }

    #[test]
    fn non_interactive_server_type_must_be_explicit() {
        let error = select_server_type(None, true)
            .expect_err("non-interactive add must require an explicit type");

        assert!(error.to_string().contains("--type"), "{error}");
    }

    #[test]
    fn usage_rejects_path_like_session_ids_before_runtime_initialization() {
        let error = validate_usage_session_id("../../other-project/session")
            .expect_err("usage must reject path-like session ids");

        assert!(error.to_string().contains("session_id"), "{error}");
    }
}
