# Logging and Diagnostics Standard

This document is the repository-wide authority for application logging and
diagnostic output. Platform guides describe how to apply these rules with the
available APIs:

- Frontend: [`src/web-ui/LOGGING.md`](../../src/web-ui/LOGGING.md)
- Rust backend and applications: [`src/crates/LOGGING.md`](../../src/crates/LOGGING.md)

New code must follow this standard. Existing call sites that do not conform are
legacy debt, not examples to copy.

## Choose the Correct Channel

Logging level is not a privacy or performance boundary. Choose the data channel
before choosing a level.

| Channel | Purpose | Content policy |
|---|---|---|
| Operational local log | Runtime lifecycle, safe state transitions, degradation, and failures | Safe metadata only; never raw user, model, tool, file, terminal, or protocol content |
| Scoped local diagnostic | Reproduce a specific problem such as Flow Chat layout or model exchange behavior | Disabled by default, explicitly enabled, narrowly scoped, bounded, and never automatically uploaded |
| Telemetry | Aggregated operational health across installations | Typed allowlist only; no arbitrary log bodies, attributes, errors, identifiers, paths, or content |
| Debug sensitive telemetry | Explicitly authorized remote troubleshooting | Closed owner-built records only; may include redacted content, paths, commands, and business IDs through the dedicated bounded channel |
| User-facing status | Information the user must understand or act on | Use the owning UI, CLI output, or API result rather than a log entry |

Operational logs must never be forwarded wholesale into telemetry. Scoped
diagnostics must not be implemented by lowering the global operational log
level or by adding raw payloads to ordinary `TRACE` calls.

## Default Levels

Use these defaults for new logging surfaces unless a product-specific contract
requires a stricter value:

- Development persisted logs: `DEBUG`.
- Release persisted logs: `INFO`.
- Interactive console or stderr in release builds: `WARN` or stricter.
- `TRACE`: explicit, temporary diagnostic use only.
- Content-bearing scoped diagnostics: `OFF` until explicitly enabled.

A sink may use a stricter filter. Changing a filter must not change which data
is considered safe to log.

## Level Semantics

| Level | Use when | Do not use for |
|---|---|---|
| `TRACE` | Deep, opt-in detail about a bounded operation, protocol phase, or sampled decision | Per-chunk or per-frame output, raw payloads, secrets, or expensive values built before the level check |
| `DEBUG` | State-machine transitions, branch decisions, cache outcomes, retry attempts, and bounded operation summaries | Information required to operate a release build or routine high-frequency callbacks |
| `INFO` | Low-frequency service lifecycle events, configuration taking effect, and significant successful terminal outcomes | Every request, refresh, event, file, tool chunk, or polling iteration |
| `WARN` | The operation continues but is degraded, data was dropped, a fallback was used, or capacity/continuity is at risk | Expected absence, user cancellation, normal retry attempts, or errors that are immediately returned unchanged |
| `ERROR` | The owning operation has definitively failed, an invariant was violated, data may be corrupted, or a security boundary failed | Expected control flow, recoverable intermediate failures, or the same error at every propagation layer |
| `OFF` | The channel must emit nothing | A substitute for fixing unsafe log content |

Additional rules:

- User cancellation is normally `DEBUG` or no log. Use `INFO` only when the
  cancellation is an important lifecycle outcome.
- An expected optional miss is `DEBUG` or no log.
- Log individual retry attempts at `DEBUG`. Log one `WARN` when recovery causes
  visible degradation, or one `ERROR` when all attempts are exhausted.
- Do not promote a message to `WARN` or `ERROR` merely to make it visible under
  a stricter production filter.

## What to Log

Prefer one terminal summary from the component that owns the operation. Useful
fields include:

- `operation` and `outcome` using stable, bounded values.
- `duration_ms` in Rust or `durationMs` in frontend diagnostics.
- Counts, byte sizes, queue depth, retry count, and dropped or suppressed count.
- A typed `error_type` or `error_code`, and whether the condition is retryable.
- An opaque local correlation ID when it is necessary to connect related log
  entries.

Log low-frequency lifecycle transitions when they explain behavior, such as a
service becoming ready, a transport changing connectivity state, a fallback
being selected, or a bounded queue losing continuity.

Do not log start and finish for every routine operation. A `DEBUG` start entry
is justified only when it helps distinguish a hang from a slow completion.

## Message and Field Format

- Messages must be English-only and contain no emojis.
- Use a stable, concise message template. Put variable data in fields rather
  than interpolating it into prose.
- Use a stable module context or target. Do not derive logger names, field keys,
  or telemetry attributes from runtime input.
- Keep all values bounded. Record counts or classifications instead of arrays,
  object dumps, or unbounded strings.
- Frontend diagnostic objects use `camelCase`; Rust diagnostic fields use
  `snake_case`. Preserve protocol field names when logging a safe protocol fact.
- Avoid redundant fields already supplied by the sink, such as timestamps and
  thread IDs.

Opaque request, run, turn, or trace IDs may be used only in local logs when they
are required for correlation. Do not log account identifiers, user identifiers,
device identifiers, session titles, project names, or other human identity.
Business IDs must not become telemetry attributes or metric labels.

## Sensitive Data

### Prohibited in Every Channel

Never record these values, even in `TRACE`, a scoped diagnostic, a crash report,
or a support bundle:

- API keys, access or refresh tokens, passwords, cookies, authorization headers,
  session keys, private keys, signing material, certificates, pairing secrets,
  delegated credentials, or secret-bearing environment variables.
- Credential prefixes, suffixes, reversible encodings, or stable hashes that
  allow the original secret to be correlated.

Replace the entire value with a fixed marker such as `[redacted]`. Keeping the
first or last characters is not redaction.

### Prohibited in Operational Logs and Safe Telemetry

Do not place the following in ordinary logs or telemetry:

- User prompts, system prompts, assistant responses, reasoning text, or chat
  transcripts.
- Model request or response bodies, SSE frames, token deltas, tool arguments or
  results, MCP payloads, hook payloads, or full event objects.
- File content, diffs, snapshots, terminal input or output, clipboard content,
  DOM or WebView payloads, and serialized application state.
- Absolute paths, repository or workspace names, branch names, URLs with query
  data, hostnames, IP addresses, proxy identities, email addresses, and device
  names.
- Raw third-party errors or stack traces that may echo any of the values above.

The dedicated `TelemetryLevel::Debug` channel is the only remote exception for
the content, paths, commands, and business IDs listed above. It requires
separate persisted sensitive-content consent on Desktop or the explicit
`BITFUN_TELEMETRY_LEVEL=debug` deployment setting on Server/Relay. Its fixed
record variants are created by the authoritative Turn, Inference, Tool, or
Approval owner; ordinary log text and public
events are never projected into it. Known credential shapes are replaced as a
whole value, but free-text pattern redaction may miss an unknown secret.
Accounts, emails, organizations, device identity, and credentials remain
prohibited. Debug records use a separate bounded in-memory queue, are not
written to a retry file, and are discarded immediately when Debug is lowered
or disabled.

Each content-bearing field carries its redacted value, original byte size, and
truncation flag. Content fields share a deterministic per-record budget, and
the serialized record has a final 256 KiB bound. Recovery remains a terminal
fact of Inference, Tool, or Compression; it is not a standalone Debug event.

When a scoped local diagnostic genuinely requires private content, it must have
its own explicit user-facing switch, default to off, state what it captures,
write to a dedicated bounded artifact, and never upload automatically. Secrets
remain prohibited. Exporting or sharing that artifact is a separate user action.

### Safe Alternatives

Prefer finite classes, booleans, counts, sizes, duration, outcome, retryability,
and presence flags. For example, record `payload_bytes=2048` and
`error_type=schema` rather than the payload or parser error text.

Redaction is defense in depth, not permission to log unsafe data. Source-level
allowlisting is preferred because key-based filters cannot recognize secrets
stored under generic names. Truncation alone is never redaction.

## Error Ownership

An error should normally be logged once by the layer that owns its terminal
outcome.

- A lower layer that returns an error unchanged should not also log it.
- A lower layer may log when it retries, recovers, drops work, substitutes a
  fallback, or deliberately suppresses the error.
- A boundary that converts an internal failure into a user-visible or protocol
  result may log one safe terminal summary.
- Do not blindly format Rust `Display` values, JavaScript `Error` objects, HTTP
  bodies, provider errors, or subprocess output. Map them to safe typed fields
  first.
- Stack traces belong in an explicitly controlled crash or diagnostic artifact,
  not in normal release logs.

## High-Frequency Paths

The following are aggregate-only sources for operational logging:

- Model stream chunks, SSE frames, token deltas, reasoning deltas, and partial
  tool arguments.
- Terminal read/write chunks and subprocess stdout or stderr.
- File-watch events, search matches, LSP diagnostics, semantic-token callbacks,
  and progress events.
- Event buses, queues, routers, transport fanout, and frontend store subscriptions.
- Mouse, pointer, scroll, resize, animation-frame, render, and layout callbacks.
- Polling, heartbeat, presence, synchronization, Git refresh, and reconnect
  iterations.

Do not emit one operational Log, Span, or Metric record per item. Instead:

1. Aggregate at the owning operation or time window.
2. Emit a terminal summary containing counts, duration, outcome, and dropped or
   suppressed work.
3. Emit only state transitions, such as connected to disconnected, rather than
   every check that observes the same state.
4. Rate-limit repeated `WARN` and `ERROR` messages. Preserve the first event and
   later emit a summary with `suppressed_count`.
5. Check the level or diagnostic switch before cloning, formatting, serializing,
   collecting a stack, or building a diagnostic object.
6. Keep queues, batch size, entry size, file size, and retention bounded. Record
   overflow as an aggregate rather than blocking the producer.
7. Never perform synchronous file or network IO on an interaction, render,
   streaming, terminal, or event-routing hot path.

A scoped diagnostic may capture higher-frequency facts only when it is opt-in,
uses lazy payload construction, batches writes, has a bounded queue, reports
dropped entries, and can be disabled without changing product behavior.

## Telemetry Boundary

Safe Trace, Metric, and Log telemetry accepts only registered, typed facts. It
must not accept arbitrary attribute names, message bodies, JSON values, raw
errors, business identifiers, paths, prompts, tool inputs, or tool outputs.

Debug sensitive telemetry uses a separate closed record enum and separate Log
scope. It does not relax `ValidatedRecord`, `AttributeValue`, the static safe
attribute allowlist, or Metric labels. There is intentionally no public API for
arbitrary event names or arbitrary JSON records.

Use finite enums and numeric aggregates to keep cardinality bounded. Stream
chunks, terminal chunks, token deltas, and file events contribute only to an
owning operation's aggregate terminal fact. Local `log` or `tracing` output and
content-bearing diagnostics must never be bridged wholesale into a remote
telemetry exporter.

## Review Checklist

Before adding or changing a log, verify:

- The selected channel is correct and the message is useful at that level.
- The operation owner emits the terminal result only once.
- Every field is safe, bounded, and necessary to diagnose an operational fact.
- No raw error, payload, content, path, endpoint, identity, or credential can
  reach the sink through either success or error paths.
- High-frequency producers aggregate, rate-limit, and avoid eager formatting.
- New diagnostic artifacts are off by default and have explicit size,
  retention, export, and upload behavior.
- Privacy tests use canary prompts, paths, identifiers, credentials, endpoints,
  and raw errors when the changed boundary could expose them.
