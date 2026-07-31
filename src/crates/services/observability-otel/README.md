# bitfun-observability-otel

Concrete native OpenTelemetry runtime for BitFun. It owns receiver-scoped
installation identity, deployment-only endpoint and secret resolution, bounded
in-memory Trace/Log batching, Metric aggregation, generation fencing, retry,
health, flush, and shutdown.

Product and business code must depend on the portable `Telemetry` facade. Only
native application bootstrap and deployment configuration owners use this crate
directly. The crate never bridges ordinary `log`/`tracing`, model exchange
traces, prompts, tool payloads, file content, paths, or product events.
