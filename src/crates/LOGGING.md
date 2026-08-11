# Rust Logging Guide

The repository-wide level, privacy, ownership, and high-frequency rules are
defined in [`docs/development/logging.md`](../../docs/development/logging.md).
This guide describes how to apply them in Rust crates and applications.

## Logging API

Follow the logging facade already owned by the crate or application:

- Reusable crates and the desktop application commonly use the `log` facade.
- CLI and server applications that already install a `tracing` subscriber may
  use structured `tracing` events and spans.
- Do not mix styles within a module or add an upward dependency solely to gain
  a different logging API.
- Do not initialize a global logger or tracing subscriber from a library crate.
  Product applications own sinks, filters, rotation, and process-level setup.

`log` example:

```rust
use log::debug;

debug!(
    "Git status completed: outcome=success, changed_file_count={}, duration_ms={}",
    changed_file_count,
    duration_ms
);
```

`tracing` example:

```rust
tracing::warn!(
    operation = "event_fanout",
    outcome = "degraded",
    dropped_count,
    "Event fanout lost continuity"
);
```

Use stable `snake_case` keys. With `log`, format fields as `key=value`; with
`tracing`, use typed fields. Do not interpolate fields into natural-language
sentences when a stable field form is available.

## Levels

Apply the common level semantics with these Rust examples:

| Level | Rust example |
|---|---|
| `TRACE` | Bounded, opt-in details for one protocol phase or sampled decision |
| `DEBUG` | State transition, cache decision, retry attempt, or operation summary |
| `INFO` | Process/service lifecycle or significant successful terminal outcome |
| `WARN` | Fallback, dropped work, lag, partial cleanup, or degraded continuation |
| `ERROR` | Definitive failure owned by this boundary, invariant violation, or possible data loss |

Expected absence and user cancellation are normally `DEBUG` or no log. A
transient failure that will be retried is `DEBUG`; record `WARN` only if the
recovery itself matters, and record one `ERROR` if the owning operation finally
fails.

## Targets and Sinks

Use the module target unless an application already defines a stable routing
target, such as a separate AI or search log. Do not create a target to bypass a
filter or to make high-frequency output visible.

Desktop and CLI applications may split app, AI, WebView, search, or dedicated
diagnostic output into different rotated files. File separation is not a
privacy boundary: every target still follows the common sensitive-data rules.

Library code must not assume that a log is ephemeral. A product host may persist
it, include it in a diagnostic bundle, display it on stderr, or forward it to
another local adapter.

## Safe Errors

Do not blindly print arbitrary errors with `{error}`, `{error:#}`, `{:?}`, or a
full error chain. Provider, HTTP, parser, filesystem, subprocess, hook, MCP, and
protocol errors may contain credentials, content, paths, endpoints, or raw
payloads.

Map failures to safe fields at the owning boundary:

```rust
log::error!(
    "Model request failed: operation=inference, error_type={}, retryable={}",
    safe_error_type,
    retryable
);
```

Raw error text may be retained only in a separately controlled local diagnostic
artifact when the common standard permits that content. Secrets are always
prohibited.

A function that returns an error unchanged should not also log it. Log locally
only when the function retries, falls back, drops or suppresses work, performs
partial cleanup, or otherwise owns an operational consequence.

## Expensive Values

Check the level before formatting, cloning, serializing, or collecting expensive
diagnostic values:

```rust
if log::log_enabled!(log::Level::Debug) {
    let summary = build_safe_bounded_summary(&state);
    log::debug!("Runtime state summarized: {}", summary);
}
```

For `tracing`, use the subscriber-aware equivalent when expensive work cannot
be avoided through ordinary field evaluation. The value must still be safe and
bounded.

Never pretty-print an entire request, response, event, application state,
configuration, prompt, tool invocation, terminal chunk, or file content. A
disabled log statement is not free if its arguments were already constructed.

## Timing

Use the nearest layer-owned timing helper when one exists. In `bitfun-core` and
its consumers, prefer `elapsed_ms`, `elapsed_ms_u64`, and `TimingCollector` for
diagnostic durations. Lower-layer crates must not add an upward dependency on
`bitfun-core` just to use those helpers; use an owner-local helper or
`std::time::Instant` instead.

Use `duration_ms` for Rust diagnostic fields. Preserve names such as
`execution_time_ms` or `response_time_ms` when they belong to an existing event,
API, or persisted contract.

Do not emit a timing log for each chunk, event, loop iteration, file, or callback.
Aggregate timing at the operation boundary. Repeated slow-path warnings require
a justified threshold and rate limiting.

## High-Frequency Paths

Stream chunks, terminal output, event routing, file watchers, LSP progress,
polling, heartbeats, synchronization, pointer input, and similar paths must not
emit one operational log per item.

Use:

- A terminal summary from the owning operation.
- Counts, sizes, duration, maximum queue depth, and outcome.
- State-transition logs instead of repeated state observations.
- Rate-limited warnings with a later `suppressed_count` summary.
- Bounded queues and batched asynchronous writes for explicitly enabled
  diagnostics.

Do not perform synchronous file or network logging on a runtime hot path. Log
writer failure must not block or recursively log through the same failing sink.

## Telemetry

`log` and `tracing` calls are local diagnostics unless a component has an
explicit, reviewed telemetry contract. Do not attach a generic OpenTelemetry
layer that exports all existing events.

Safe Trace, Metric, and Log telemetry must use typed domain facts, registered
finite attributes, bounded cardinality, and authoritative operation ownership.
Raw error messages, business IDs, provider or model names, paths, prompts,
payloads, tool names, tool inputs, and tool outputs must not be exported through
that channel.

The only remote content-bearing exception is the separately authorized
`TelemetryLevel::Debug` Log channel. Callers must use one of its closed
owner-specific record variants; they must not serialize ordinary `log` or
`tracing` events into it. Credentials and identity remain prohibited, known
secret shapes are whole-value redacted, and unknown secrets in free text can
still evade pattern redaction.

## Verification

Documentation-only changes require:

```bash
git diff --check
```

For code changes, run the smallest Rust check and focused test owned by the
changed crate. Add privacy canaries for serialization, redaction, diagnostic
export, or telemetry changes. Add aggregation, rate-limit, disabled-path, and
drop-summary tests when changing a high-frequency diagnostic path.
