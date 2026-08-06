# 宿主平台、Tauri 与远程工作区

> 根 `AGENTS.md` 配套（STD-05 / STD-06 中与宿主相关的部分）。
> 改桌面 command、UI 与宿主边界，或远程工作区行为时阅读。
>
> [English](host-platform-and-remote.md)

## Tauri command

- 命令名：`snake_case`。
- TypeScript 可用 `camelCase` 包装，调用 Rust 时必须传结构化 `request`：

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

桌面宿主细则另见 [`src/apps/desktop/AGENTS.md`](../../src/apps/desktop/AGENTS.md)。

## 平台边界

- UI 组件不要直接调 Tauri API，应经 adapter / infrastructure 层访问。
- 桌面端专属集成放在 `src/apps/desktop`，再通过类型化能力接口接入；需要事件投递时，使用已有生产 transport adapter。
- 共享 core 里避免出现 `tauri::AppHandle` 等宿主 API；优先用 `bitfun_events::EventEmitter` 这类共享抽象。

## 远程兼容

- 做新功能时，一开始就要考虑远程工作区与远程控制同步。若只按本地实现，远程场景容易悄悄缺能力。
- 某能力确实无法合理支持远程工作区时，应显式屏蔽或给出清晰的「不支持」提示，不要用笼统错误糊弄过去。
- 每个桌面 Tauri 命令都必须在
  `src/apps/desktop/src/api/remote_workspace_policy.rs` 声明远程工作区策略；
  该文件的契约测试会拒绝未声明策略的新命令，也不允许继续扩大 legacy-unaudited 存量。
