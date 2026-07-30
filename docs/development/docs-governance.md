# In-repo docs governance

Purpose: how to place, write, and index documentation inside the BitFun **code** repository.  
Scope: `docs/`, root `AGENTS` / `CONTRIBUTING`, and nearest module `AGENTS.md`.  
Status: stable (target layout fixed; `ade-spec` / `features` / `plans` / `superpowers` have merged into `specs/`)  
Authority language: Chinese — see [`docs-governance-CN.md`](docs-governance-CN.md). This file is the English summary for AI / ops readers.

Rules for a separate product/docs site (user manuals, onboarding guides) are out of scope.

## Split: code repo vs docs site

| Keep in code repo | Put in separate docs site |
|---|---|
| Boundaries and ops needed to change this codebase | User manuals, integration guides, external narratives |
| Architecture constraints, verification matrix, command catalog | Training, marketing, long prose weakly tied to implementation |
| In-progress / stable specs and implementation plans (with PRs) | Pure historical archive, ops setup guides |
| Nearest `AGENTS.md` / `LOGGING.md` | — |

Prefer migrating out first: `docs/remote-connect/`.

## Target `docs/` layout

```text
docs/
  architecture/     # Stable architecture; ADRs live here (no top-level ADR dir)
  development/      # Dev ops: commands, verification, host/remote, agent-loop, this doc
  specs/            # Specs + plans (see README index)
    README.md
    templates/
    plans/
  sdlc-harness/     # Target-project governance product docs (keep in-repo)
```

`ade-spec/`, `features/`, `plans/`, and `superpowers/` have merged into `specs/` (old directories removed).

## Two-hop index

```text
AGENTS.md  →  directory README / single authority  →  (at most one more hop) body
```

- At most two hops to the body.
- Directories whose article set changes (`specs/`, `architecture/`, `sdlc-harness/`) need a README index.
- Hot single pages may be linked directly from AGENTS (e.g. `product-architecture.md`, `verification.md`).

## Language

| Kind | Language | Bilingual |
|---|---|---|
| Human-facing narrative | Chinese authority | English not required by default |
| Root `AGENTS` / `CONTRIBUTING` | — | Both required; must stay in sync |
| AI / code-change ops constraints (e.g. `development/*`, module `AGENTS`) | English authority | Chinese copy not required by default |
| Logs | English only | No Chinese, no bilingual logs |

## Format

- Page header: purpose, scope, status (`draft`/`stable`), authority language, related links.
- Link authorities; do not paste long bodies into indexes.
- Filenames: English kebab-case.

## Spec / Design / Plan

- Process and index: [`docs/specs/README.md`](../specs/README.md)
- Templates: [`docs/specs/templates/`](../specs/templates/)
- Cross-module plans: [`docs/specs/plans/`](../specs/plans/)

## Root entrypoints

| File | Location | Role |
|---|---|---|
| `AGENTS.md` / `AGENTS-CN.md` | Repo root | Code-change norms entry; progressive disclosure |
| `CONTRIBUTING.md` / `CONTRIBUTING_CN.md` | Repo root | How humans contribute; link commands/verification; link AGENTS for norms |

Cross-link both. CONTRIBUTING must not keep a third full command encyclopedia.

## Related

- Commands: [`common-commands.md`](common-commands.md)
- Verification: [`verification.md`](verification.md)
- Norms entry: [`AGENTS.md`](../../AGENTS.md) / [`AGENTS-CN.md`](../../AGENTS-CN.md)
- Contributing: [`CONTRIBUTING.md`](../../CONTRIBUTING.md) / [`CONTRIBUTING_CN.md`](../../CONTRIBUTING_CN.md)
