# DevEco Code Observability Alignment Contract

Status: Frozen for implementation

## 1. Scope and authority

This contract defines the DevEco Code observability semantics that BitFun must
cover. It is an implementation contract, not a replacement for the BitFun
observability design.

Authority rules:

1. The privacy, safety, signal-ownership, and delivery constraints in the
   reviewed *BitFun agent kernel observability and Telemetry design*
   (2026-07-25) remain hard boundaries.
2. This contract is authoritative for DevEco semantic coverage and P0 point
   completeness. When the earlier design has no registered name or point for a
   required semantic, this contract adds it instead of treating the omission as
   a reason to drop the point.
3. A direct conflict with a hard boundary must be resolved explicitly; a design
   omission is not a conflict.

The comparison baseline is:

- Repository: `https://gitcode.com/openharmony-sig/deveco-code`
- Branch: `develop`
- Commit: `766509a2a4f937a8b5bc61b4079ba76ba71b3f83`
- Surfaces: Analytics, Analytics Magpie, and Effect/OpenTelemetry tracing

Alignment means that BitFun can answer the same safe operational questions. It
does not mean copying DevEco field names, transport payloads, raw values, or the
event-reconstructed Magpie state machine.

Every DevEco field or point must end in exactly one state:

- `equivalent`: the same semantic fact is emitted.
- `safe_equivalent`: a bounded classification, aggregate, or Trace relation
  provides the same operational answer without exporting the original value.
- `bitfun_addition`: BitFun emits an additional safe fact.
- `excluded`: the source value is forbidden by the BitFun privacy contract.

There must be no unexplained `missing` state at release readiness.

## 2. Frozen field mapping

| DevEco field or semantic | BitFun representation | State | Rule |
|---|---|---|---|
| `sourceType` | `bitfun.entrypoint` Resource attribute | `safe_equivalent` | Finite product-entrypoint enum |
| `sourceVersion` | `service.version` | `equivalent` | Build-owned value |
| `os_arch` | `host.arch` | `equivalent` | Compile-target mapping |
| `os_name` | `os.type` | `equivalent` | Compile-target mapping |
| `os_version` | none | `excluded` | Exact OS versions add device-fingerprinting entropy; `os.type` and `host.arch` retain the safe fleet breakdown |
| analytics schema version | `bitfun.telemetry.schema.version` | `equivalent` | Resource-level schema major version |
| `providerId` | `bitfun.inference.provider_class` | `safe_equivalent` | Never export a provider name or account identity |
| `modelId` | `bitfun.inference.model_class` | `safe_equivalent` | Never export a model name |
| `agentName` | `bitfun.agent.turn.mode_class` | `safe_equivalent` | Finite product-owned mode class |
| `sessionid`, `messageId`, part/call IDs | OTLP Trace/Span identity | `safe_equivalent` | Never duplicate business IDs as attributes |
| parent session and message references | parent Span or Trace Link | `safe_equivalent` | No business IDs in Link attributes |
| `isSuccess`, status | operation-specific `outcome` | `equivalent` | Finite outcome enum |
| finish reason | operation-specific `finish_reason` | `safe_equivalent` | Finite reason enum |
| error type/message | `error.type` | `safe_equivalent` | Map from typed Rust errors; never inspect display text |
| `totalElapsed` | operation Span duration and duration Metric | `equivalent` | Metric unit is seconds; diagnostic attribute is milliseconds |
| `firstResultElapsed` | `bitfun.agent.turn.first_result_ms` | `equivalent` | Turn start to first non-empty user-visible assistant text |
| input/output token count | `bitfun.inference.usage.*_tokens` | `equivalent` | Numeric Histogram |
| reasoning/cache-read token count | corresponding BitFun token Metrics | `bitfun_addition` | Numeric Histogram |
| builtin/MCP/skill operation groups | `bitfun.tool.source_class` | `safe_equivalent` | `builtin/mcp/skill/plugin/external/custom` |
| tool name | `bitfun.tool.kind` | `safe_equivalent` | Finite capability class; no raw tool names |
| tool duration | Tool Span and duration Metric | `equivalent` | Real pipeline boundary |
| tool status source | `bitfun.tool.execute.failure_source` | `safe_equivalent` | Typed finite enum only |
| tool exit code | `bitfun.tool.execute.exit_status_class` | `safe_equivalent` | 当前只产出 `success/unknown`；`nonzero/signal` 保留但未产出。通用 Tool 边界没有类型化进程退出元数据，禁止解析 Tool 结果或错误文本来补齐 |
| tool background flag | `bitfun.tool.execute.background` | `equivalent` | Boolean |
| telemetry input/output truncation flag | none | `excluded` | DevEco sets this when collected Tool content is shortened; BitFun does not collect that content, so there is no telemetry payload to truncate |
| queue, preflight, confirmation, execution time | phase-specific Tool duration fields | `bitfun_addition` | Numeric values, never path or content |
| modified file count | `bitfun.agent.turn.modified_file_count` | `equivalent` | Turn-level numeric aggregate |
| additions/deletions | `bitfun.agent.turn.added_lines/deleted_lines` | `equivalent` | Turn-level numeric aggregate |
| modified file list/path digest | none | `excluded` | Paths and stable path-derived identifiers are forbidden |
| `startedAt` | OTLP Span/Log timestamp | `equivalent` | Do not duplicate as an attribute |
| query, answer, prompt, reasoning text | none | `excluded` | Content is forbidden at every telemetry level |
| tool input/output/output tail | none | `excluded` | Content is forbidden at every telemetry level |
| project ID, bundle name, UID | none | `excluded` | Project and user identity are forbidden |
| username, machine name, endpoint | none | `excluded` | Identity and service address are forbidden |

## 3. Frozen point matrix

| Operation | DevEco owner/point | BitFun owner | Required signal |
|---|---|---|---|
| Startup | observability/plugin initialization | product runtime assembly initialization | Span + terminal Metric/Log |
| Session create | `Session.create*`, `session.created` | `ConversationCoordinator` create owner | Span + terminal Metric/Log |
| Session delete | Session removal owner | `ConversationCoordinator::delete_session` | Span + terminal Metric/Log |
| Turn | `chat.message`, `SessionPrompt.run/loop` | `ExecutionEngine::execute_dialog_turn` | Span + terminal Metric/Log |
| Round | prompt loop/assistant execution | logical Round owner in `ExecutionEngine`/`RoundExecutor` | Span + terminal Metric/Log |
| Inference request | `LLM.run` | one logical model request including retries | Span + terminal Metric/Log |
| Inference attempt | AI SDK/provider call | each real `send_message_stream` attempt through stream terminal state | child Span |
| First result | first non-empty text delta | first user-visible output observed by the Turn owner | Turn terminal fact |
| Tool | `tool.execute.before/after`, `Tool.execute` | `ToolPipeline::execute_single_tool` | Span + terminal Metric/Log |
| Permission evaluation | `Permission.ask` policy path | `ToolPipeline::draft_permission_plan` | child Span + terminal Metric/Log |
| Permission confirmation | interactive permission wait | `ToolPipeline::await_permission_execution_plan` | child Span + terminal Metric/Log |
| Compression | `SessionCompaction.*` | automatic/manual compression owner in `ExecutionEngine` | child/root Span + terminal Metric/Log |
| File mutation aggregate | file edit/tool diff facts | authoritative mutation/tool result owner | Turn terminal aggregate only |
| Subagent | Task and child-session relation | Task launch and child Turn owner | parent Span or Trace Link |
| MCP tool | MCP tool execution | generic Tool boundary with `source_class=mcp` | Tool signals |
| MCP lifecycle | MCP connect/list/read/call | MCP service owner | P1 Span + terminal Metric/Log |
| Plugin hook | `Plugin.trigger` | plugin runtime owner | P1 Span + terminal Metric/Log |
| Persistence | Session storage functions | session persistence owner | P1 child Span + terminal Metric/Log |

High-frequency stream chunks, terminal chunks, token deltas, and file events are
aggregate-only sources. They must not produce one Span or Log per event.

## 4. Frozen Trace topology

```text
bitfun.app.startup

bitfun.agent.session

bitfun.agent.turn
|- bitfun.context.prepare
|- bitfun.agent.round
|  |- bitfun.inference.request
|  |  |- bitfun.inference.attempt
|  |  `- bitfun.inference.attempt
|  `- bitfun.tool.execute
|     |- bitfun.permission.evaluate
|     `- bitfun.permission.confirmation
`- bitfun.agent.compression
```

`bitfun.context.prepare` is retained as a design-target child Span, but it is
not a DevEco/P0 point-matrix requirement in this contract. It must be added only
after one real preparation owner can close success, failure, cancellation, and
early-return paths without reconstructing state from events.

Rules:

- The real async owner starts and ends each Span.
- A logical Inference request contains all its provider attempts.
- An Inference Attempt begins immediately before a real provider call and ends
  after its stream succeeds, fails, times out, or is cancelled.
- A foreground child Turn may inherit the launching Tool context.
- A background child Turn that can outlive the launching Tool starts a new
  Trace and carries a Link to the launching context.
- Events never start, finish, or reconstruct Trace state.
- Trace context contains only W3C identifiers and sampling state. Baggage,
  installation IDs, Session IDs, paths, and arbitrary metadata are forbidden.

## 5. Frozen signal ownership

| Fact | Trace owner | Metric owner | OTel Log owner |
|---|---|---|---|
| Operation lifecycle | real async business owner | none | none |
| Authoritative terminal outcome | owner finishes Span | the same owner projects terminal Metric | the same owner projects terminal Log with sampled Span context when present |
| Token usage | none | Round owner records provider-returned typed usage | none |
| Turn first result and mutation aggregate | Turn owner supplies and finishes typed terminal facts | the same owner | the same owner |
| Process and exporter health | telemetry runtime | telemetry runtime | telemetry runtime with fixed body |

For one terminal business fact, Metric and OTel Log are emitted exactly once by
the operation owner's terminal finish. Public `AgenticEvent` values remain UI
and host contracts and do not reconstruct or duplicate telemetry terminal
signals. This preserves typed errors, classification facts, and sampled W3C
Span context without adding telemetry-only fields to public events.

Local `log`/`tracing` output and Model Exchange Trace remain separate local
diagnostic channels. They are never bridged wholesale to the remote OTel Log
exporter.

## 6. Frozen safe enums

The initial finite values are:

- Tool source: `builtin`, `mcp`, `skill`, `plugin`, `external`, `custom`.
- Tool failure source: `validation`, `permission`, `execution`, `timeout`,
  `cancellation`, `provider`, `internal`, `other`.
- Exit status: `success`, `unknown`; `nonzero` and `signal` remain registered
  reserved values but are unproduced until a typed metadata channel exists.
- Permission decision: `allow`, `ask`, `policy_deny`, `user_reject`,
  `cancelled`, `failed`.
- Permission source: `policy`, `stored_grant`, `hook`, `auto_approve`, `user`,
  `delegated`, `other`.
- Compression trigger: `threshold`, `context_overflow`, `manual`, `recovery`,
  `other`.
- Compression result source: `model`, `local_fallback`, `none`.
- Session operation: `create`, `resume`, `delete`.
- Session class: `standard`, `subagent`, `internal`, `transient`.

Unknown runtime values map to the finite `other`/`custom` value. They must not
be inserted into telemetry keys or enum values.

The authoritative design already registers `app.startup`, `agent.turn`,
`agent.round`, `agent.compression`, `inference.request`,
`inference.attempt`, `tool.execute`, and `permission.evaluate`. This contract
registers the following compatible extensions required to close the P0 and
DevEco semantic gaps:

| Name | Owner | Purpose | Privacy boundary | Consumer |
|---|---|---|---|---|
| `bitfun.agent.session` | Agent session lifecycle owner | Session create/resume/delete outcome and duration | Finite operation/class/remote facts only; no Session ID or name | lifecycle and reliability dashboards |
| `bitfun.permission.confirmation` | Tool permission owner | Separate policy evaluation latency from interactive wait latency | Finite decision/source/count facts only; no path, resource, feedback, or identity | latency and rejection dashboards |

## 7. Release acceptance

The alignment is release-ready only when all of the following hold:

1. A machine-readable descriptor snapshot covers every non-P1 row in the point
   matrix and every non-excluded field mapping.
2. In-memory Trace tests prove the Turn -> Round -> Inference Request ->
   Inference Attempt and Turn -> Round -> Tool -> Permission hierarchy.
3. Retry, cancellation, timeout, permission rejection, and compression fallback
   paths close every started Span with a typed outcome.
4. Terminal Metric/Log tests prove exactly-once projection.
5. Concurrency tests prove that independent Turns do not share Trace context.
6. Privacy canaries containing prompts, responses, paths, IDs, names,
   credentials, endpoints, and raw errors are absent from all three signals.
7. `off` creates no exported records and policy revision invalidates queued or
   active observations according to the design.
8. Production export remains disabled until consent, scoped installation ID,
   bounded queues, retry, reload, health, and shutdown semantics are complete.
