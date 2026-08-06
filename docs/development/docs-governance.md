# In-repo docs governance

Purpose: how to place, write, and index documentation inside the BitFun **code** repository.  
Scope: `docs/`, root `AGENTS` / `CONTRIBUTING`, and nearest module `AGENTS.md`.  
Status: stable (target layout fixed; `ade-spec` / `features` / `plans` / `superpowers` have merged into `specs/`)  
Authority language: Chinese — see [`docs-governance.zh-CN.md`](docs-governance.zh-CN.md). This file is the English summary for AI / ops readers.

Rules for a separate product/docs site (user manuals, onboarding guides) are out of scope.

## Non-negotiable preservation rules

1. A documentation reorganization must preserve normative meaning. Splitting,
   merging, renaming, and re-indexing may change presentation, but must not
   change owners, requirements, current/target status, failure behavior, or
   acceptance criteria.
2. Before moving or renaming a page, inventory inbound references from Markdown,
   source code, configuration, tests, packaging, and product-facing URLs. Update
   every reference in the same change; do not leave compatibility to memory.
3. A page may leave the code repository only after proving that no code,
   runtime behavior, build/package step, test, or user-facing product link
   depends on its repository path. Otherwise retain it, or migrate the
   dependency and its tests together and verify the replacement URL is stable.
4. Preserve current/proposed/completed labels exactly when consolidating text.
   Moving a statement does not authorize changing its maturity or authority.
5. Record an old-to-new content map in the PR when a reorganization deletes or
   merges authorities. A link checker proves reachability, not semantic parity;
   human review remains required for the map.

## Split: code repo vs docs site

| Keep in code repo | Put in separate docs site |
|---|---|
| Boundaries and ops needed to change this codebase | User manuals, integration guides, external narratives |
| Architecture constraints, verification matrix, command catalog | Training, marketing, long prose weakly tied to implementation |
| In-progress / stable specs and implementation plans that evolve with PRs | Pure historical archive, deployment/operator setup guides |
| Nearest `AGENTS.md` / `LOGGING.md` | — |

Tracking in-progress specs and implementation plans is an intentional current
workflow policy. Ephemeral prompts, research scratch, review drafts, and
personal notes are not repository documentation: keep them untracked and use a
`.local.md` suffix when a local filename helps.

`docs/remote-connect/` is temporarily retained because Web UI product links
currently point at its repository URLs. It may migrate only after stable public
URLs exist and the code references plus focused tests migrate in the same change.

## Target `docs/` layout

```text
docs/
  README.md         # Directory map and placement router; no policy body
  architecture/     # Stable architecture; ADRs live here (no top-level ADR dir)
  development/      # Dev ops: commands, verification, host/remote, agent-loop, this doc
  performance/      # Measured performance investigations and reports
  remote-connect/   # Temporary user guides with product URL dependencies
  specs/            # Specs + plans (see README index)
    README.md
    templates/
    plans/
  sdlc-harness/     # Target-project governance product docs (keep in-repo)
```

`ade-spec/`, `features/`, `plans/`, and `superpowers/` have merged into `specs/` (old directories removed).

## Directory boundaries

| Directory | Must contain | Must not contain |
|---|---|---|
| `docs/architecture/` | Stable cross-module architecture boundaries, owner/dependency rules, accepted design authorities, ADRs | Implementation task lists, temporary review notes, user setup guides, benchmark dumps, module-local coding rules |
| `docs/development/` | Repository operations and code-change rules: commands, verification, host/platform constraints, logging, i18n operations, test-id policy, docs governance | Product requirements, feature implementation plans, user manuals, stable product architecture duplicated from `architecture/` |
| `docs/specs/` | Draft/in-progress specs, feature designs, closeout records; `plans/` owns implementation plans and `templates/` owns authoring templates | A second stable cross-cutting architecture authority, personal scratch files, generated evidence, user/operator guides |
| `docs/sdlc-harness/` | Product requirements, architecture, features, research, and governance for the SDLC quality-harness product | Generic BitFun repository rules that belong in root `AGENTS` or `development/`, unrelated feature specs |
| `docs/performance/` | Reproducible performance investigations, measurements, bottleneck reports, and optimization evidence | Normative architecture, command/verification authorities, unbounded raw profiler output, claims without environment and measurement context |
| `docs/remote-connect/` | Temporarily retained end-user setup pages whose repository URLs are consumed by the product | Runtime architecture, credentials/secrets, internal deployment runbooks, new guides without a product-link dependency |
| `docs/` root | `README.md` only; local untracked `*.local.md` scratch may exist in a developer workspace | Tracked topical articles, tracked `.local.md` files, duplicate indexes, generated output |

The nearest directory README owns the exact article list and local boundary.
Stable conclusions discovered in a spec or performance report move to the
existing architecture authority; the source document links to that authority
instead of retaining a competing rule body.

## Two-hop index

```text
AGENTS.md  →  directory README / single authority  →  (at most one more hop) body
```

- At most two hops from the matching entry/index to the authoritative body.
- Every maintained documentation directory with multiple articles needs a
  README that states scope, exclusions, and a complete article index.
- Every non-template governed page must have at least one inbound index or task
  route. New pages and renames update the nearest index in the same change.
- Hot single pages may be linked directly from AGENTS (e.g. `product-architecture.md`, `verification.md`).
- Indexes contain routing summaries only. They must not fork normative bodies.

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
- Paired locale files use `<name>.md` and `<name>.zh-CN.md`. Root compatibility
  entrypoints keep their established names (`AGENTS-CN.md`,
  `CONTRIBUTING_CN.md`).
- Standalone implementation plans end in `-plan.md`; closeout records end in
  `-completed.md`.

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
- Development index: [`README.md`](README.md)
- Documentation map: [`docs/README.md`](../README.md)
- Norms entry: [`AGENTS.md`](../../AGENTS.md) / [`AGENTS-CN.md`](../../AGENTS-CN.md)
- Contributing: [`CONTRIBUTING.md`](../../CONTRIBUTING.md) / [`CONTRIBUTING_CN.md`](../../CONTRIBUTING_CN.md)
