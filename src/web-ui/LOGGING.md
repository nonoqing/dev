# Frontend Logging Guide

The repository-wide level, privacy, ownership, and high-frequency rules are
defined in [`docs/development/logging.md`](../../docs/development/logging.md).
This guide describes how to apply them in `src/web-ui`.

## Standard Logger

Use a stable scoped logger at module scope:

```typescript
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceManager');

log.debug('Workspace initialization completed', {
  outcome: 'success',
  fileCount,
  durationMs,
});
```

Rules:

- Use `createLogger('StableModuleName')`. Do not build the context from props,
  routes, workspace names, IDs, or other runtime values.
- Use a fixed English message without emojis. Put dynamic values in the data
  object.
- Use `camelCase` for frontend diagnostic fields, including `durationMs`.
- Do not call `console.*` from application code. Desktop builds forward console
  calls to persistent WebView logs, so console output has the same privacy and
  performance impact as the shared logger.
- Do not add a second frontend logging abstraction for a feature. A dedicated
  diagnostic recorder is appropriate only when the common standard requires a
  separately gated, bounded channel.

The rendered format is:

```text
[Context] Stable message {"field":"value"}
```

## Levels

Apply the common level semantics with these frontend examples:

| Level | Frontend example |
|---|---|
| `TRACE` | A bounded, opt-in diagnostic summary for one refresh or layout operation |
| `DEBUG` | State transition, cache decision, retry attempt, or skipped expected branch |
| `INFO` | Application or long-lived service became ready; significant workflow completed |
| `WARN` | UI remains usable after fallback, dropped work, stale state, or degraded capability |
| `ERROR` | The owning user operation or frontend service definitively failed |

Do not use `WARN` for a normal unavailable optional capability, and do not use
`ERROR` for an exception that is caught and handled without user-visible or
state impact.

## Safe Data and Errors

The logger serializes the data it receives. Serialization is not a privacy
boundary and does not make an object safe.

Prefer a safe allowlisted summary:

```typescript
log.warn('Request retry budget exhausted', {
  operation: 'loadConfig',
  attemptCount,
  errorType: classifyRequestError(error),
  retryable: false,
});
```

Do not pass request configuration, event payloads, application state, raw
responses, paths, or arbitrary `Error` objects. At API and transport boundaries,
use the existing sanitizer as defense in depth after first selecting only the
fields that are operationally necessary:

```typescript
import { sanitizeErrorForLog } from '@/infrastructure/api/logSanitizer';

log.error('Request failed', {
  action,
  error: sanitizeErrorForLog(error),
});
```

Do not treat `sanitizeLogValue` as permission to dump a whole request or state
tree. Key-based sanitization cannot identify every private value. Error stacks
and error messages may contain URLs, paths, payload fragments, or user content;
do not pass them to ordinary release logs unless the boundary guarantees that
the error shape is safe.

## Timing

Use `src/shared/utils/timing.ts` for frontend diagnostic timing:

```typescript
import { measureAsyncAndLog } from '@/shared/utils/timing';

await measureAsyncAndLog(log, 'Workspace refresh completed', refreshWorkspace, {
  level: 'debug',
  data: { reason: 'windowFocus' },
});
```

Rules:

- Prefer `measureSync`, `measureAsync`, `measureSyncAndLog`,
  `measureAsyncAndLog`, `logDuration`, and `logElapsed` over handwritten timing
  logs.
- Use `durationMs` for diagnostic fields. Preserve protocol or persisted names
  such as `duration_ms` when they are part of an existing contract.
- Do not time or log every render, animation frame, token, event, or polling
  iteration.
- A slow-path warning needs a measured threshold and must be rate-limited if the
  condition can repeat continuously.

## Performance and Diagnostics

Calling `log.trace(...)` does not make eager payload construction free. Do not
clone, map, serialize, stringify, or collect large values before a disabled log
call.

On a hot path, use an existing dedicated diagnostic switch before constructing
data. If a general lazy logging capability is required, add a level-aware API to
the shared logger with tests rather than implementing local checks in multiple
features.

Flow Chat viewport diagnostics are the reference pattern for high-frequency
frontend diagnostics:

- Explicitly enabled and disabled independently of the ordinary log level.
- Data is constructed lazily.
- Entries are batched and the queue is bounded.
- Dropped entries are summarized.
- Output uses a dedicated rotated file.

Use this pattern only for a specific reproducible problem. Ordinary logging of
SSE chunks, terminal output, file events, store subscriptions, scroll, pointer,
resize, rendering, polling, heartbeat, or progress callbacks is prohibited.

`sendDebugProbe` is a convenience wrapper over the ordinary logger and timing
helpers. It is not a separate safe or high-frequency channel.

## Environment Behavior

- Tauri desktop sends frontend logs through `@tauri-apps/plugin-log` into the
  WebView log target.
- Browser-only builds fall back to the browser console.
- Desktop startup synchronizes the frontend threshold with the runtime logging
  configuration.
- Filtering changes visibility only. It does not permit sensitive data at a
  lower level.

## Verification

For changes to the frontend logger, sanitizer, logging configuration, or a
diagnostic recorder, run the smallest focused tests plus:

```bash
pnpm run type-check:web
```

Add privacy canaries when changing serialization or export behavior, and add
queue, batching, drop-summary, and disabled-path tests for high-frequency
diagnostic recorders.
