mod runtime;

use anyhow::{Context, Result};

async fn run_host() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .init();

    let workspace_root = std::env::current_dir().context("Failed to resolve SDK workspace")?;
    runtime::SdkHostRuntime::select_process_profile()?;
    runtime::initialize_terminal_service().await;

    bitfun_core::service::config::initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    let path_manager = bitfun_core::infrastructure::try_get_path_manager_arc()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let telemetry_runtime = bitfun_observability_otel::TelemetryRuntimeHandle::new(
        bitfun_observability_otel::TelemetryRuntimeMetadata::new(
            bitfun_observability::TelemetryEntrypoint::Sdk,
            path_manager.user_data_dir(),
        ),
        std::sync::Arc::new(bitfun_observability_otel::SystemKeyringTelemetrySecrets),
    );
    let _telemetry_shutdown = telemetry_runtime.shutdown_guard();
    let config_service = bitfun_core::service::config::get_global_config_service().await?;
    let config = config_service
        .get_config::<bitfun_core::service::config::GlobalConfig>(None)
        .await?;
    if let Err(error) = telemetry_runtime.apply_config(
        &config.app.telemetry,
        &bitfun_observability_otel::TelemetryDeploymentConfig::from_product_build(),
    ) {
        tracing::warn!(
            "Telemetry is unavailable; effective level is off: {}",
            error
        );
    }
    let startup_observation = telemetry_runtime.startup_guard();
    bitfun_core::infrastructure::ai::AIClientFactory::initialize_global()
        .await
        .context("Failed to initialize global AI client factory")?;

    let host = runtime::SdkHostRuntime::build(&workspace_root, telemetry_runtime.telemetry())
        .await
        .context("Failed to assemble Agent SDK Host")?;
    startup_observation.complete();
    bitfun_sdk_host_app::transport::serve_stdio(
        host.agent_runtime().clone(),
        host.workspace_root().to_string_lossy().into_owned(),
    )
    .await
    .context("Agent SDK Host transport failed")
}

fn main() {
    bitfun_sdk_host_app::initialize_process_runtime();

    let worker = bitfun_sdk_host_app::spawn_sdk_host_worker(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build SDK Host Tokio runtime");
        runtime.block_on(run_host())
    })
    .expect("failed to spawn SDK Host worker thread");

    match worker.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: SDK Host worker thread panicked");
            std::process::exit(1);
        }
    }
}
