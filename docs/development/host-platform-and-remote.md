# Host platform, Tauri, and remote workspace

> Companion to the root `AGENTS.md` entry (STD-05 / STD-06 related host rules).
> Open this when changing desktop commands, UI↔host boundaries, or remote workspace behavior.
>
> [中文](host-platform-and-remote.zh-CN.md)

## Tauri commands

- Command names: `snake_case`.
- TypeScript may wrap with `camelCase`, but invoke Rust with a structured `request`:

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

Also follow [`src/apps/desktop/AGENTS.md`](../../src/apps/desktop/AGENTS.md) for desktop host scope.

## Platform boundaries

- Do not call Tauri APIs directly from UI components; go through the adapter/infrastructure layer.
- Desktop-only host adapters belong in `src/apps/desktop`, then flow through typed capability interfaces and, when event delivery is needed, the production transport adapter.
- In shared core, avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`.

## Remote scenarios

BitFun is not a local-only desktop app. The workspace, the runtime that executes
a turn, and the person driving it can each sit on a different machine. Treat the
four scenarios below as first-class targets of every change, not as a later port.

| Scenario | What it means | Design entry point |
|---|---|---|
| Remote workspace | The active workspace lives on an SSH host, a jump-host chain, or a Docker container; files, terminal, search, and Agent subprocesses must execute there | [remote-workspace-transport.md](../architecture/remote-workspace-transport.md), [remote-workspaces.md](../specs/remote-workspaces.md) |
| Remote control | Mobile web, or a Feishu / Telegram / WeChat bot, drives a session on a Desktop or CLI host through the Remote Connect relay | [`src/mobile-web`](../../src/mobile-web/AGENTS.md), `remote_connect` in [services-integrations](../../src/crates/services/services-integrations/AGENTS.md), [relay-service](../../src/crates/services/relay-service/AGENTS.md) |
| Peer Device Mode | One same-account device becomes the data plane of another: the controller shell stays local, invokes and events come from the peer | [peer-device-mode.md](../architecture/peer-device-mode.md), [peer-device README](../../src/web-ui/src/infrastructure/peer-device/README.md) |
| Detached Dispatch | A controller submits a durable job to another BitFun host and may then disconnect; the target owns the job, session, worktree, event log, and permission mailbox | [detached-task-dispatch.md](../architecture/detached-task-dispatch.md) |

Rules that apply to all four:

- Design the remote path together with the feature. A capability that assumes UI,
  process, and filesystem share one machine is incomplete, not "phase one".
- Degrade loudly. Gate unsupported entry points or return a clear unsupported
  state. Silent local fallback, fake success, empty payloads, and generic errors
  are regressions; local fallback can also leak local content to a remote controller.
- Keep blocking interaction answerable through the existing dialog and permission
  mailbox orchestration. Desktop-only prompts deadlock remote control and dispatch.
- Survive disconnect with resumable cursors and idempotent mutations rather than
  state that exists only while a client is attached.
- Remote workspace paths are POSIX on every client OS. Do not split or join them
  with host `std::path` semantics or reuse controller-side paths on a peer host.

Per-scenario obligations:

- **Remote workspace:** every desktop Tauri command declares its policy in
  [`remote_workspace_policy.rs`](../../src/apps/desktop/src/api/remote_workspace_policy.rs).
  Its contract test rejects missing policy and growth of `LegacyUnaudited`.
- **Remote control:** mobile web and IM bots use the `RemoteCommand` protocol and
  bot router/menu, not Web UI. Session-level capabilities must extend those
  surfaces or return an explicit unsupported reply.
- **Peer Device Mode:** product commands proxy to the peer by default. Commands
  that must stay on the controller are denied consistently in
  [`peer_host_invoke.rs`](../../src/apps/desktop/src/api/peer_host_invoke.rs),
  [`deny.rs`](../../src/apps/cli/src/peer_host/deny.rs), and
  [`peer-device-adapter.ts`](../../src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts).
- **Detached Dispatch:** jobs run headless on the target under the CLI delivery
  profile. Do not require a live submitter; negotiate new target capabilities
  through the dispatch protocol rather than assuming them.

State which remote scenarios a change was exercised in. Local-only tests are not
evidence of remote behavior.

## Upgrade compatibility

Users upgrade in place, and remote scenarios routinely connect different BitFun
versions. Every change must keep existing installs working without manual repair.

- Add persisted fields with defaults, keep deserialization tolerant, and never
  repurpose or narrow fields already on disk. Old data must not lack a required field.
- Never delete or reset user data to recover from parse failures. Preserve the
  record, degrade the feature, and expose a clear state; destructive removal is explicit.
- Negotiate capabilities across Peer HostInvoke, dispatch, relay/mobile web, and
  IM bot version boundaries. Package version equality does not prove behavior.
- Treat renames as migrations: read old names, IDs, and shapes while supported
  peers can send them, and migrate referenced data such as vault entries and paths.
- Prove compatibility with legacy deserialization and old-payload round trips,
  not only data written by the current code.
