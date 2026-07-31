# bitfun-observability

Portable, OpenTelemetry-SDK-independent contracts for BitFun telemetry.

The crate exposes typed Startup, Session, Turn, Round, Inference, Tool,
Permission, and Compression facts. Callers cannot submit arbitrary attribute
names, log bodies, JSON, errors, paths, prompts, tool inputs, or tool outputs.
Every emitted record passes the static schema and privacy gate before reaching
`TelemetrySink`.

Trace ownership stays at the real asynchronous operation boundary. Agent event
projection is deliberately limited to Metric and structured Log records from
authoritative terminal events; it must not reconstruct a Trace state machine.
