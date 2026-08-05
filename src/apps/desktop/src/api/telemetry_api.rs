//! Desktop telemetry preference and redacted runtime status API.

use crate::api::app_state::AppState;
use bitfun_observability::{TelemetryLevel, TelemetryUserConfig};
use bitfun_observability_otel::{
    TelemetryDeploymentConfig, TelemetryHealthSnapshot, TelemetryRuntimeHandle,
};
use serde::{Deserialize, Serialize};
use tauri::State;

const TELEMETRY_CONFIG_PATH: &str = "app.telemetry";

#[derive(Debug, Default, Deserialize)]
pub struct TelemetryStateRequest {}

#[derive(Debug, Deserialize)]
pub struct SetTelemetryLevelRequest {
    pub level: TelemetryLevel,
}

/// The frontend receives only user consent and redacted operational status.
/// Receiver configuration, credentials, and installation identity stay private.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryStateResponse {
    pub level: TelemetryLevel,
    pub health: TelemetryHealthSnapshot,
}

#[tauri::command]
pub async fn telemetry_state(
    state: State<'_, AppState>,
    runtime: State<'_, TelemetryRuntimeHandle>,
    _request: TelemetryStateRequest,
) -> Result<TelemetryStateResponse, String> {
    let config = state
        .config_service
        .get_config::<TelemetryUserConfig>(Some(TELEMETRY_CONFIG_PATH))
        .await
        .map_err(|error| format!("Failed to read telemetry preference: {error}"))?;
    Ok(TelemetryStateResponse {
        level: config.effective_level(),
        health: runtime.health(),
    })
}

#[tauri::command]
pub async fn set_telemetry_level(
    state: State<'_, AppState>,
    runtime: State<'_, TelemetryRuntimeHandle>,
    request: SetTelemetryLevelRequest,
) -> Result<TelemetryStateResponse, String> {
    let config = TelemetryUserConfig::new(request.level);
    state
        .config_service
        .set_config(TELEMETRY_CONFIG_PATH, &config)
        .await
        .map_err(|error| format!("Failed to save telemetry preference: {error}"))?;

    crate::api::remote_connect_api::notify_settings_changed();
    if let Err(error) =
        runtime.apply_config(&config, &TelemetryDeploymentConfig::from_product_build())
    {
        log::warn!("Telemetry preference was saved but runtime remains disabled: {error}");
    }

    Ok(TelemetryStateResponse {
        level: request.level,
        health: runtime.health(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_observability_otel::TelemetryHealthState;

    #[test]
    fn response_serialization_excludes_transport_and_identity_fields() {
        let response = TelemetryStateResponse {
            level: TelemetryLevel::Basic,
            health: TelemetryHealthSnapshot {
                state: TelemetryHealthState::Healthy,
                user_level: TelemetryLevel::Basic,
                effective_level: TelemetryLevel::Basic,
                ..TelemetryHealthSnapshot::default()
            },
        };
        let value = serde_json::to_value(response).unwrap();
        let serialized = value.to_string();

        assert_eq!(value["level"], "basic");
        for forbidden in ["endpoint", "header", "secret", "installation", "installId"] {
            assert!(!serialized.contains(forbidden), "leaked field: {forbidden}");
        }
    }
}
