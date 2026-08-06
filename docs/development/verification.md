# Verification matrix

> Companion to the root `AGENTS.md` entry (STD-08). This table is the
> **authoritative “what to run after this change”** selector. For the command
> dictionary, see [`common-commands.md`](common-commands.md).
>
> [中文](verification.zh-CN.md)

Run the smallest local precheck that matches the touched files. CI is expected to
cover full builds and broad test suites; run heavier local commands only when the
change directly affects build, packaging, or CI cannot protect the path.

| Change type | Minimum verification |
|---|---|
| Frontend UI, state, or adapters without i18n resource/contract changes | `pnpm run type-check:web`, plus nearest focused test when behavior changed |
| Locale resource-only changes | `pnpm run i18n:audit` |
| Locale contract or shared terms | `pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit` |
| Web UI i18n runtime, namespace loading, or direct `i18nService.t(...)` usage | `pnpm run i18n:contract:test && pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts` |
| Mobile web UI, state, pairing, disconnect, or reconnect behavior | `pnpm --dir src/mobile-web run type-check`; include manual pairing/reconnect notes when behavior changed |
| Product definition, schema, resolver, or Desktop/CLI product build adapter | `pnpm run product:test`, plus `pnpm run product:check` for the default definition |
| Shared Rust logic in `core`, `transport`, adapters, or services | `cargo check --workspace`, plus nearest focused `cargo test` when behavior changed |
| Desktop integration, Tauri APIs, browser/computer-use, or desktop-only behavior | `cargo check -p bitfun-desktop`, plus focused desktop tests when behavior changed |
| Behavior covered by desktop smoke/functional flows | Nearest focused E2E/smoke check; rely on CI for broad build/test unless build behavior changed |
| `src/crates/adapters/ai-adapters` | Relevant Rust checks above; add `cargo test -p bitfun-agent-stream` only when stream contracts changed |
| Installer frontend or i18n runtime without packaging changes | `pnpm --dir BitFun-Installer run type-check` |
| Installer Tauri/Rust changes | `cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml` |
| Installer packaging, payload, install/uninstall flow, or native bundling | `pnpm run installer:build` |
| Build scripts or prerequisite changes | `pnpm run check:build-prereqs`, plus `node --test scripts/check-build-prereqs.test.mjs` when the check logic changed |
| Documentation structure, indexes, local links, anchors, or naming | `pnpm run docs:links:check && pnpm run docs:architecture:check && git diff --check`; add the matching `docs:links:test` / `docs:architecture:test` when a checker changed |
