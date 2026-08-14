# BitFun CLI Agent Guide

Scope: `src/apps/cli`.

Read the repository `AGENTS.md` first. For architecture-sensitive work, also
read:

- [`cli-product-line-design.md`](../../../docs/architecture/cli-product-line-design.md)
- [`product-architecture.md`](../../../docs/architecture/product-architecture.md)
- [`agent-runtime-deployment-design.md`](../../../docs/architecture/agent-runtime-deployment-design.md)
- [`product-customization-blueprint.md`](../../../docs/architecture/product-customization-blueprint.md) when changing product assembly, branding, or packaging

## Ownership

CLI owns only surface concerns:

- Clap entrypoints and CLI-local configuration
- terminal acquisition/restoration and input normalization
- TUI state, rendering, popups, local draft history, and local effects such as
  clipboard or external-editor integration
- projection of Runtime events into text, JSON, JSONL, and user diagnostics
- Shared Runtime client/server adaptation and Peer Device host presentation

Session, turn, model round, tool execution, permissions, cancellation,
persistence, context, workspace binding, MCP, Subagent, and other product facts
belong to their shared owners. Do not add CLI-only managers or reproduce shared
behavior behind a TUI branch.

Existing Core compatibility forwarding may remain until a reviewed owner
migration has behavior-equivalence tests. A typed port is not evidence that the
runtime owner moved.

## Runtime paths

Normal interactive submissions follow:

```text
ChatView -> TuiAgentClient -> TuiBackend -> App Server client
         -> App Server handler -> Agent Runtime owner -> ToolPipeline
```

Embedded TUI uses an in-memory App Server connection. During the migration,
Shared TUI uses a CLI Host compatibility adapter that implements `TuiBackend`
over the private versioned Runtime IPC; the TUI client and controllers must not
reference that IPC. Replacing the physical Shared transport with App Server
Pipe/UDS framing belongs to Phase 5. Side-effecting operations need stable
identities, controller/idle rules, bounded frames, and outcome-unknown handling
before a connection can retry.

Explicit Shell input follows:

```text
SHELL composer -> AgentUserShellCommandPort -> Core coordinator
               -> ToolPipeline(ExecCommand) -> TerminalPort / RemoteExecPort
               -> standard UserDialog + ModelRound persistence and events
```

CLI must never spawn the submitted command directly or expose a generic tool or
process API. Explicit user input may auto-approve an interactive `ask`, but
static `deny` rules, workspace routing, cancellation, audit, and tool
restrictions remain enforced.

## TUI rules

- Derive slash commands, palette actions, help, availability, and key bindings
  from the action registry. Do not add a second command table. Emacs-style
  editing keys (Ctrl+A/E/K/U, Alt+D/Ctrl+D, undo) are handled by a shared
  `TextInput::handle_emacs_edit_key` fallback in both chat and startup.
- Match established competitor entry flows when equivalent behavior exists.
  Prefer OpenCode names and interactions; do not invent `/shell` or aliases for
  the `!` Shell entry.
- Keep terminal input, state transitions, effects, and rendering independently
  testable. Views and reducers do not perform filesystem, network, config, or
  Agent operations.
- Shell mode is CLI presentation state only. It accepts an empty-composer `!`,
  keeps chat/shell histories separate, treats `/` as command text, and rejects
  images and structured `@` references before Runtime submission.
- Direct paste, `Ctrl+V`, and bracketed paste share `ComposerDraft`. Shared TUI
  rejects unsupported image payloads before IPC. Prompt stash
  (`StashStore`, JSONL-persisted) is a separate local-only mechanism and does
  not enter `ComposerDraft` or Runtime submission.
- Local effects such as `/editor`, copy, and export stay local. Product work
  such as shell execution, session mutation, and permissions goes through typed
  Runtime owners. Session pinning is local-only (persisted to CLI config);
  model favorite is persisted to backend `AIModelConfig`.
- Terminal suspend (Unix Ctrl+Z via SIGTSTP/SIGCONT) is handled at the event
  loop layer before key dispatch. With focused non-empty composer input,
  Ctrl+D deletes one word forward like Alt+D; with empty input it exits when
  the current page/runtime state permits. Ctrl+C retains clear/interrupt/exit
  behavior.
- Session-lineage membership, order, legacy relationship normalization,
  transcript reads, and targeted cancellation stay in shared Runtime owners.
  TUI may keep only the selector/read-only inspection state and must preserve
  the root composer while a descendant is visible.
- Always restore raw mode, alternate screen, mouse capture, paste mode, and the
  cursor on success, error, cancellation, initialization failure, or panic.
- Protocol stdout contains only the selected result format. Logs are English,
  contain no emoji, and use stderr or log files.

## Product and external-source boundaries

- Assemble CLI through `DeliveryProfile::Cli` and validated product Runtime
  parts. Hiding a command is not a backend capability restriction.
- The CLI selects the reviewed `bitfun-core` owner-feature closure
  (`agent-runtime`, `canvas-runtime`, `external-sources`, `plugin-runtime`, and
  `ssh-remote`). Do not replace it with `product-full` or a CLI-named umbrella;
  add a Core feature only when a production CLI path consumes that owner.
- CLI consumes typed external-source summaries and actions. It does not parse
  source files, import executable modules, start plugin workers, duplicate
  approval state, or treat static discovery as runtime availability.
- ACP agents, configuration import, executable plugins, Hooks, and Peer Device
  hosting have separate trust and lifecycle state. Do not infer one from
  another.
- Remote-unsupported local effects must fail visibly; never fall back to the
  controller machine.

Detailed compatibility rules belong in the dedicated architecture documents,
not in this file.

## Commands

```bash
pnpm run cli:dev
pnpm run cli:install
```

## Verification

Run the smallest checks matching the changed path:

```bash
cargo check -p bitfun-cli
cargo test -p bitfun-cli
```

When a CLI change crosses a shared boundary, use the focused command maintained
by that owner: Agent Runtime for port/SDK behavior, the IPC adapter for shared
protocol behavior, Core for turn/tool/persistence behavior, Terminal for
PTY/ConPTY lifecycle, and Product Assembly for packaging. Do not copy those
owners' commands into this guide.

Use [`README.md`](README.md) for user-facing behavior and installation. Keep
developer internals here or in architecture docs instead of expanding the user
guide.
