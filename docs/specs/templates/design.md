# <Title> Design

Date: <YYYY-MM-DD>
Status: draft | in-progress | stable
Related spec: <path or none>
Scope: <affected paths and layers>
Authority language: <zh-CN | en>

## Problem

## Confirmed approach

Data flow, command/response shapes, port vs implementation ownership.

## Layer ownership

Which Product Surface / Assembly / Feature / Kernel / Adapter / Service / Execution / Contracts own what. Cite [`docs/architecture/product-architecture.md`](../../architecture/product-architecture.md) when relevant.

## State model

| State | Meaning | Owner | Independence notes |
|---|---|---|---|

## Remote compatibility

Local-only? Remote workspace? New network/SSH/agent-loop round-trips?

## Failure / cancel / partial success

## i18n and theme

New locale keys (namespace / shared terms). Prefer existing semantic/component/domain tokens; new color tokens need an owner contract.

## Security

Execution location, sandbox, side effects, auth scope. Unknown capability defaults to restricted.

## Alternatives considered

## Architecture alignment

Conflicts with architecture docs? If yes, justification.

## Test approach

Unit / contract / focused E2E paths.
