use anyhow::{Context, Result};

use bitfun_core::agentic::execution::ExecutionEngineConfig;
use bitfun_core::product_assembly::DeliveryProfile;
use bitfun_core::product_runtime::CoreRuntimeServicesProvider;
use bitfun_core::runtime_ownership::CoreRuntimeOwnership;
use std::sync::Arc;

pub(crate) use bitfun_core::agentic::system::AgenticSystem;

fn eval_execution_config() -> ExecutionEngineConfig {
    ExecutionEngineConfig {
        // Pier/Harbor owns the evaluation time budget. Keep BitFun's other
        // loop guards, but remove its per-turn model-round cutoff.
        max_rounds: usize::MAX,
        ..Default::default()
    }
}

pub(crate) fn select_agentic_system_profile(profile: DeliveryProfile) -> Result<()> {
    bitfun_core::agentic::system::select_agentic_system_profile(profile)
        .context("Failed to select agentic system delivery profile")
}

pub(crate) async fn init_agentic_system(
    profile: DeliveryProfile,
    runtime_ownership: Arc<CoreRuntimeOwnership>,
) -> Result<AgenticSystem> {
    let system =
        bitfun_core::agentic::system::init_agentic_system_for_profile_with_execution_config(
            profile,
            runtime_ownership,
            crate::cli_telemetry(),
            eval_execution_config(),
        )
        .await
        .context("Failed to initialize agentic system")?;
    system
        .coordinator
        .set_terminal_port(CoreRuntimeServicesProvider::terminal_port());
    system
        .coordinator
        .set_remote_exec_port(CoreRuntimeServicesProvider::remote_exec_port());
    Ok(system)
}

#[cfg(test)]
mod tests {
    use super::eval_execution_config;

    #[test]
    fn eval_cli_has_no_practical_model_round_cap() {
        assert_eq!(eval_execution_config().max_rounds, usize::MAX);
    }
}
