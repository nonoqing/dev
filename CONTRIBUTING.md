# Contributing

[中文版](./CONTRIBUTING_CN.md)

Thanks for your interest in BitFun! BitFun is a multi-platform AI programming environment powered by Rust and TypeScript, with shared core logic across Desktop/CLI/Server.

This file is the **human contribution entry**: how to set up, open PRs, and what we welcome.  
**Code-change norms, architecture, and verification** live in [`AGENTS.md`](AGENTS.md) — do not treat this file as a third command or verification encyclopedia.

## Code of Conduct

Be respectful, kind, and constructive. We welcome contributors of all backgrounds and experience levels.

## Quick start

### Prerequisites

- Node.js 22.12+ (LTS recommended)
- pnpm 10.15.0 via Corepack
- Rust toolchain (install via [rustup](https://rustup.rs/))
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for desktop development

BitFun standardizes local JavaScript builds and CI on Node.js 22.12+. GitHub Actions in this repo may use Node.js 24-compatible action runtimes, but project scripts should run on Node.js 22.12+ unless a narrower local guide says otherwise. After switching from an older Node.js version, rerun `pnpm install`.

#### Windows: OpenSSL

Most Windows contributors do not need to configure OpenSSL manually. Use `pnpm run desktop:dev` or the normal `desktop:build*` scripts; they bootstrap a pre-built OpenSSL package when needed.

Only handle OpenSSL yourself when the bootstrap fails, you are preparing CI, or you intentionally use `pnpm run desktop:dev:raw`. In that case, run `scripts/ci/setup-openssl-windows.ps1`, or set `OPENSSL_DIR` to a pre-built x64 OpenSSL directory and set `OPENSSL_STATIC=1`.

#### Build Prerequisites Check

When `cargo check --workspace`, `cargo check -p bitfun-desktop`, or pnpm build
commands fail with confusing errors (e.g., "resource path doesn't exist" or
sherpa-onnx download failures), run the preflight check to identify missing
prerequisites and get actionable fix commands:

```bash
pnpm run check:build-prereqs           # check only
pnpm run check:build-prereqs -- --fix  # attempt to fix missing prerequisites
```

The check detects:

- Missing `node_modules` (fix: `pnpm install`)
- Missing `src/mobile-web/dist` (fix: `pnpm run prepare:mobile-web` — the
  bitfun-desktop Tauri build script references this directory as a resource,
  so `cargo check -p bitfun-desktop` and `cargo check --workspace` fail
  without it)
- Missing sherpa-onnx prebuilt libs (the sherpa-onnx-sys build script
  downloads from GitHub at build time; if the download fails on poor
  connectivity, set `SHERPA_ONNX_LIB_DIR` to the prebuilt lib directory
  under `target/sherpa-onnx-prebuilt/` to use the local copy)

### Install dependencies

```bash
pnpm install
```

### Daily commands

```bash
pnpm run desktop:dev                # full hot-reload: Vite HMR + Rust auto-rebuild & restart
pnpm run desktop:preview:debug      # pre-built binary + Vite HMR; no Rust auto-rebuild
```

Prefer `desktop:dev` for active development. Use `desktop:preview:debug` for frontend-only iteration or a faster cold start.

Full command catalog: [`docs/development/common-commands.md`](docs/development/common-commands.md) (also [`package.json`](package.json)).  
After changes, pick the smallest check from [`docs/development/verification.md`](docs/development/verification.md).  
Architecture and coding norms: [`AGENTS.md`](AGENTS.md).

### Desktop debugging

Dev builds enable the `devtools` Cargo feature. `F12` opens native webview DevTools; `Cmd/Ctrl + Shift + I` toggles the BitFun element inspector; `Cmd/Ctrl + Shift + J` also opens native DevTools. These tools are disabled in end-user `release` builds.

## Code standards

Use [`AGENTS.md`](AGENTS.md) (and the nearest module `AGENTS.md`) for architecture, module boundaries, i18n/theme/logging, host/remote rules, and the verification matrix. Keep PRs aligned with those indexes; do not duplicate them here.

Docs placement (Spec / Design / Plan): [`docs/development/docs-governance.md`](docs/development/docs-governance.md) and [`docs/specs/README.md`](docs/specs/README.md).

## What we welcome

1. Ideas and creativity (features, interactions, visuals) via issues — product/UI contributors are welcome to submit via PI; we help refine for development.
2. Agent system quality and overall product quality
3. Stability and foundational capabilities
4. Ecosystem expansion (Skills, MCP, LSP plugins, or better domain-specific scenarios)

### Beyond features and fixes

| Area | Location | Example |
| --- | --- | --- |
| Prompts | `src/crates/assembly/core/src/agentic/agents/prompts/` | Add or refine prompts and related logic |
| Tools | `src/crates/assembly/core/src/agentic/tools/implementations/`, `.../registry.rs` | Implement and register tools |
| Subagents | `src/crates/assembly/core/src/agentic/agents/custom_subagents/`, `.../registry.rs` | Implement and register subagents |
| Modes | `*_mode.rs`, `prompts/*_mode.md`, `src/web-ui/src/locales/*/settings/modes.json` | Keep mode logic, prompts, and UI copy in sync |
| Scenario guides | `website/src/docs/` | Workflows / playbooks (or link from `README.md`) |

## Contribution workflow

### Before you start

- Open an issue for larger changes to avoid duplication and design conflicts.
- Discuss design early for new features or UI changes.
- Use issue and PR templates; keep the PR focused; explain skipped verification when it matters.

### PR title and description

Prefer Conventional Commits: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.

UI changes should include before/after screenshots or a short recording. If the work is AI-assisted, note that and the testing level (untested / lightly tested / fully tested).

Do not commit transient AI prompts, local absolute paths, generated scratch files, pairing secrets, tokens, certificates, or unrelated artifacts.

### Branch and scope

**Open all PRs against `main`.** Keep PRs small and focused; avoid unrelated changes.

## Testing and verification

Run the **smallest** checks that match the changed files. Full matrix: [`docs/development/verification.md`](docs/development/verification.md). CI covers broad suites; run heavier local commands only when build, packaging, or CI-blind paths are affected.

If you cannot run a relevant check, explain why in the PR and provide a lower-risk manual path.

## Security

- Do not commit secrets, tokens, certificates, or sensitive data.
- When adding dependencies, ensure license compatibility and explain the purpose.

## Thanks

Every contribution matters. Issues, PRs, and suggestions are all welcome!
