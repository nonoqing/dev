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

## 远程场景

BitFun 不只是本地桌面应用：工作区、执行当前轮次的 runtime 与操作者可能分别位于不同机器。
以下四种场景是一等目标，不是事后补做的移植。

| 场景 | 含义 | 设计入口 |
|---|---|---|
| 远程工作区 | 工作区位于 SSH 主机、跳板机链路或 Docker 容器；文件、终端、搜索与 Agent 子进程必须在那里执行 | [remote-workspace-transport.md](../architecture/remote-workspace-transport.md)、[remote-workspaces.md](../specs/remote-workspaces.md) |
| 远程控制 | mobile web 或飞书 / Telegram / 微信 Bot 通过 Remote Connect relay 驱动 Desktop 或 CLI 宿主上的会话 | [`src/mobile-web`](../../src/mobile-web/AGENTS.md)、[services-integrations](../../src/crates/services/services-integrations/AGENTS.md) 的 `remote_connect`、[relay-service](../../src/crates/services/relay-service/AGENTS.md) |
| 多端互控（Peer Device Mode） | 同账号设备互为数据平面；控制端外壳留在本地，invoke 与事件来自 peer | [peer-device-mode.md](../architecture/peer-device-mode.md)、[peer-device README](../../src/web-ui/src/infrastructure/peer-device/README.md) |
| Dispatch 分离任务 | 控制端把持久化任务提交给另一宿主后可断开；目标端拥有 job、session、worktree、事件日志与权限信箱 | [detached-task-dispatch.md](../architecture/detached-task-dispatch.md) |

四种场景共同适用：

- 功能设计必须同时覆盖远程路径。默认 UI、进程与文件系统位于同一机器的能力属于未完成。
- 不支持要显式暴露。静默回落本地、假成功、空载荷与笼统错误都属于回归；本地回落还可能泄露内容。
- 阻塞交互必须通过既有 dialog 与权限信箱编排送达操作者；只能靠桌面窗口解除会造成远程死锁。
- 使用可恢复 cursor 与幂等变更承受断线，不依赖客户端恰好在线才存在的状态。
- 远程工作区路径在任何客户端 OS 上都是 POSIX 路径；不得按宿主 `std::path` 语义处理，也不得复用控制端路径。

各场景的具体义务：

- **远程工作区：**每个桌面 Tauri 命令都必须在
  [`remote_workspace_policy.rs`](../../src/apps/desktop/src/api/remote_workspace_policy.rs)
  声明策略；契约测试拒绝缺失策略或扩大 `LegacyUnaudited`。
- **远程控制：**mobile web 与 IM Bot 使用 `RemoteCommand` 协议和 bot router/menu，不走 Web UI。
  会话级能力必须扩展这些产品面，或返回明确的不支持回复。
- **多端互控：**产品命令默认代理到 peer。必须留在控制端的命令要同步维护
  [`peer_host_invoke.rs`](../../src/apps/desktop/src/api/peer_host_invoke.rs)、
  [`deny.rs`](../../src/apps/cli/src/peer_host/deny.rs) 与
  [`peer-device-adapter.ts`](../../src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts) 三份拒绝清单。
- **Dispatch 分离任务：**任务在目标端以 CLI delivery profile 无界面运行。不得依赖提交方在线；
  新目标端能力必须通过 dispatch 协议协商，不能直接假设存在。

改动说明必须写清验证过哪些远程场景；只跑本地测试不能作为远程行为证据。

## 升级兼容性

用户会原地升级，远程场景也经常让不同版本的 BitFun 位于同一链路。所有改动都必须让既有安装无需手工修复。

- 落盘结构新增字段必须有默认值，反序列化保持容错，不得重新定义或收窄已有字段；旧数据给不出的字段不能变成必填。
- 不得通过删除或重置用户数据来恢复解析失败；应保留记录、降级功能并展示清晰状态，销毁性删除只能由用户显式触发。
- Peer HostInvoke、dispatch、relay/mobile web 与 IM Bot 的跨版本边界必须协商 capability；包版本相同不能证明行为相同。
- 改名属于迁移：受支持对端仍可能发送旧名称、ID 或结构时必须兼容读取，并同步迁移 vault 条目、路径等引用数据。
- 必须用旧数据反序列化与旧载荷往返测试证明兼容性，不能只验证当前代码写出的数据。
