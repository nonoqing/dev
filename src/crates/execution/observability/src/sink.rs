use crate::{TelemetryResource, ValidatedRecord};
use std::sync::Mutex;

/// Non-blocking destination for records that already passed the privacy gate.
///
/// Implementations must isolate failures from product control flow. Network
/// implementations should enqueue with a bounded `try_send` instead of waiting
/// for transport I/O in this method.
pub trait TelemetrySink: Send + Sync + 'static {
    /// Configure immutable OTel Resource facts before the first record.
    fn configure_resource(&self, _resource: TelemetryResource) {}

    fn emit(&self, record: ValidatedRecord);

    fn discard_pending(&self) {}
}

#[derive(Debug, Default)]
pub struct NoopSink;

impl TelemetrySink for NoopSink {
    fn emit(&self, _record: ValidatedRecord) {}
}

#[derive(Debug, Default)]
pub struct InMemorySink {
    resource: Mutex<Option<TelemetryResource>>,
    records: Mutex<Vec<ValidatedRecord>>,
}

impl InMemorySink {
    pub fn resource(&self) -> Option<TelemetryResource> {
        self.resource
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn records(&self) -> Vec<ValidatedRecord> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn take(&self) -> Vec<ValidatedRecord> {
        std::mem::take(
            &mut *self
                .records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    }
}

impl TelemetrySink for InMemorySink {
    fn configure_resource(&self, resource: TelemetryResource) {
        *self
            .resource
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(resource);
    }

    fn emit(&self, record: ValidatedRecord) {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(record);
    }

    fn discard_pending(&self) {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PolicySnapshot, Telemetry, TelemetryEntrypoint, TelemetryLevel};
    use std::sync::Arc;

    #[test]
    fn build_configures_resource_before_any_records() {
        let sink = Arc::new(InMemorySink::default());
        let (telemetry, _) = Telemetry::build_for_entrypoint(
            PolicySnapshot::new(TelemetryLevel::Off),
            TelemetryEntrypoint::Cli,
            sink.clone(),
        );

        assert_eq!(sink.resource(), Some(telemetry.resource()));
        assert_eq!(telemetry.resource().entrypoint(), TelemetryEntrypoint::Cli);
        assert!(sink.is_empty());
    }
}
