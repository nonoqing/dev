## Current Tool Default Exposure States and Agent Overrides

Notes:
- "Default state" comes from `Tool::default_exposure()`. Tools that do not implement this method default to `Direct`.
- "Overriding agents" only lists built-in agents that explicitly define `tool_exposure_overrides()` in the current code.
- Custom subagents do not currently support independent exposure overrides and inherit the default behavior.
- Dynamically registered MCP tools default to `Deferred`; they are discovered at runtime and are not enumerated in the built-in table below.
- Global `ai.enable_deferred_tool_loading=false` overrides all allowed tools to `Direct` and removes `GetToolSpec` and `CallDeferredTool` from the model-visible manifest.
- `coding-minimal` is a closed allowlist, not a deferred view of the product catalog. Its policy explicitly permits only `Read`, `Edit`, `Write`, `ExecCommand`, `WriteStdin`, and `ExecControl`, all as `Direct`. `WriteStdin` and `ExecControl` are context-hidden unless the current Agent owns an active local or remote ExecCommand runtime session. Dynamic/MCP tools and deferred gateways are never appended.
- `coding-minimal` uses the dedicated `coding_minimal_mode` prompt. The initial prompt and contextual `ExecCommand` description name only the four tools actually sent to the model; command-control names appear only after those controls enter the request manifest.

**Tool Exposure Table**

| Tool | Default State | Overridden By | Override State |
|---|---|---|---|
| `LS` | Direct | None | - |
| `Read` | Direct | `coding-minimal` | Direct |
| `Glob` | Direct | None | - |
| `Grep` | Direct | None | - |
| `Write` | Direct | `coding-minimal` | Direct |
| `Edit` | Direct | `coding-minimal` | Direct |
| `Delete` | Direct | None | - |
| `ExecCommand` | Direct | `coding-minimal` | Direct |
| `WriteStdin` | Direct | `coding-minimal` (only while an owned ExecCommand session is active) | Direct |
| `ExecControl` | Direct | `coding-minimal` (only while an owned ExecCommand session is active) | Direct |
| `GetTime` | Direct | None | - |
| `Task` | Direct | None | - |
| `Skill` | Direct | None | - |
| `AskUserQuestion` | Direct | None | - |
| `TodoWrite` | Direct | None | - |
| `CodeReview` | Direct | None | - |
| `GetToolSpec` | Direct | None | - |
| `CallDeferredTool` | Direct | None | - |
| `CreatePlan` | Deferred | None | - |
| `GetFileDiff` | Deferred | `ReviewFixer`, `ReviewWorker`, `ReviewJudge` | Direct |
| `SessionControl` | Deferred | None | - |
| `SessionMessage` | Deferred | None | - |
| `SessionHistory` | Deferred | None | - |
| `Cron` | Deferred | None | - |
| `WebSearch` | Deferred | `agentic`, `Plan`, `debug`, `Multitask`, `DeepResearch` | Direct |
| `WebFetch` | Deferred | `agentic`, `Plan`, `debug`, `Multitask`, `DeepResearch` | Direct |
| `ListMCPResources` | Deferred | None | - |
| `ReadMCPResource` | Deferred | None | - |
| `ListMCPPrompts` | Deferred | None | - |
| `GetMCPPrompt` | Deferred | None | - |
| `GenerativeUI` | Deferred | None | - |
| `Git` | Deferred | `ReviewFixer`, `ReviewWorker`, `ReviewJudge` | Direct |
| `InitMiniApp` | Direct | None | - |
| `FinalizeMiniApp` | Direct | None | - |
| `PublishMiniApp` | Direct | None | - |
| `PublishAppearance` | Direct | None | - |
| `ControlHub` | Deferred | `ComputerUse` | Direct |
| `ComputerUse` | Deferred | `ComputerUse` | Direct |
| `Playbook` | Deferred | None | - |

**Agents With Override Policies**

| agent id | Overridden Tools |
|---|---|
| `agentic` | `WebSearch`, `WebFetch` |
| `Plan` | `WebSearch`, `WebFetch` |
| `debug` | `WebSearch`, `WebFetch` |
| `Multitask` | `WebSearch`, `WebFetch` |
| `coding-minimal` | `Read`, `Edit`, `Write`, `ExecCommand`, and context-available `WriteStdin`, `ExecControl` |
| `DeepResearch` | `WebSearch`, `WebFetch` |
| `ComputerUse` | `ControlHub`, `ComputerUse` |
| `ReviewFixer` | `GetFileDiff`, `Git` |
| `ReviewWorker` | `GetFileDiff`, `Git` |
| `ReviewJudge` | `GetFileDiff`, `Git` |
