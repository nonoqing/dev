//! Concrete OpenTelemetry transport and runtime for BitFun.
//!
//! Product code depends on `bitfun-observability`; this crate is instantiated
//! only by application entrypoints and owns identity, secrets, batching, OTLP,
//! reconfiguration, and health reporting.

mod diagnostics;
mod error;
mod identity;
mod pipeline;
mod runtime;
mod scheduler;
mod secrets;
mod settings;
mod transport;

pub use diagnostics::{TelemetryHealthSnapshot, TelemetryHealthState};
pub use error::TelemetryRuntimeError;
pub use identity::InstallationIdentityStore;
pub use runtime::{
    TelemetryRuntimeHandle, TelemetryRuntimeMetadata, TelemetryRuntimeShutdownGuard,
    TelemetryStartupGuard,
};
pub use secrets::{
    NoTelemetrySecrets, OtlpHeaders, ReadOnlySecretFileProvider, SystemKeyringTelemetrySecrets,
    TelemetrySecretProvider,
};
pub use settings::{
    OtlpCompression, TelemetryBatchConfig, TelemetryCapabilities, TelemetryDeploymentConfig,
    TelemetryRetryConfig, TelemetrySamplingConfig, TelemetrySignalTightening,
};
