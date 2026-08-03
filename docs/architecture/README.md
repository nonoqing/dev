# Architecture index

Purpose: topic map for stable architecture docs under `docs/architecture/`.  
Scope: product runtime boundaries and cross-cutting design authorities.  
Status: stable  
Authority language: Chinese for most design bodies; this index is English for AI / ops navigation.

Governance: directories whose article set changes need a README — see [`docs/development/docs-governance.md`](../development/docs-governance.md).  
Product norms entry: root [`AGENTS.md`](../../AGENTS.md). Specs / plans: [`docs/specs/README.md`](../specs/README.md).

## Start here

| Topic | Authority |
|---|---|
| Product runtime boundaries / decomposition | [`product-architecture.md`](product-architecture.md) (§1.1 guardrails) |
| Agent runtime & services | [`agent-runtime-services-design.md`](agent-runtime-services-design.md) |
| Agent SDK product shape | [`agent-sdk-product-architecture.md`](agent-sdk-product-architecture.md) |
| CLI / TUI product line | [`cli-product-line-design.md`](cli-product-line-design.md) |
| HarmonyOS PC / portability | [`platform-portability-design.md`](platform-portability-design.md) |
| Product customization | [`product-customization-blueprint.md`](product-customization-blueprint.md) |
| i18n | [`i18n.md`](i18n.md) |
| Theme / color tokens | [`theme-token-optimization.md`](theme-token-optimization.md) |
| Peer device mode | [`peer-device-mode.md`](peer-device-mode.md) |
| Remote workspace transport | [`remote-workspace-transport.md`](remote-workspace-transport.md) |
| Review lifecycle | [`review-lifecycle.md`](review-lifecycle.md) |
| Deep Review | [`deep-review.md`](deep-review.md) |
| Cache-friendly messages | [`cache-friendly-message-structure.md`](cache-friendly-message-structure.md) |
| Model request cache reuse | [`model-request-cache-reuse.md`](model-request-cache-reuse.md) |

## Extensions

Open under [`extensions/`](extensions/). Hot entry:

| Topic | Authority |
|---|---|
| OpenCode compatibility (incl. **current P0 runtime guardrail**) | [`extensions/opencode-extension-compatibility.md`](extensions/opencode-extension-compatibility.md) |
| External AI work sources | [`extensions/external-ai-work-sources-design.md`](extensions/external-ai-work-sources-design.md) |
| Capability runtime integration | [`extensions/capability-runtime-integration-design.md`](extensions/capability-runtime-integration-design.md) |
| Plugin runtime host | [`extensions/plugin-runtime-host-design.md`](extensions/plugin-runtime-host-design.md) |

Other extension designs live beside these files; prefer the OpenCode overview table for the full map.
