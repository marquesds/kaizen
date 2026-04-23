# ADR 004: AGPLv3 License

## Status
Accepted

## Context
Need OSS license before v0.1. Trade-off: permissive (MIT/Apache) vs strong-copyleft (AGPLv3).

## Decision
AGPLv3-or-later. SPDX header on every `.rs`:
`// SPDX-License-Identifier: AGPL-3.0-or-later`.

## Rationale
- Network-use clause blocks closed SaaS forks of a local-first observability tool.
- Matches project goal: keep improvements to agent telemetry open.
- Compatible with cargo-deny allow-list (MIT/Apache-2.0/AGPL-3.0).

## Alternatives
- MIT/Apache: vendor-friendly but allows closed SaaS fork. Rejected.
- GPLv3: does not cover network use; too weak for SaaS-adjacent tool.
- BSL: not OSI-approved; conflicts with OSS positioning.

## Consequences
- Commercial embedders need a dual-license conversation with maintainer.
- All new files MUST carry SPDX header (CI check in M7).
- Cargo-deny `allow = ["AGPL-3.0"]` entry required.
