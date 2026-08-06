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

## Remote compatibility

- When adding features, consider remote workspace and remote control synchronization from the start. Local-only behavior can silently leave remote scenarios incomplete.
- If a feature cannot reasonably support remote workspaces, gate it or show a clear unsupported-state message instead of failing with a generic error.
- Every desktop Tauri command must declare its remote-workspace policy in
  `src/apps/desktop/src/api/remote_workspace_policy.rs`. The contract test there rejects new
  commands without an explicit policy and forbids growing the legacy-unaudited backlog.
