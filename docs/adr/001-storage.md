# ADR 001: Storage Layer

## Status
Accepted

## Context
Need persistent store for session records + events. Queries: list sessions by workspace, append events, get session by id. Scale: ~1k sessions at M1, ~100k at M3+.

Spike E measured: SQLite WAL < 7ms for H2/H3/H8 at 1k sessions.

## Decision
SQLite WAL for M0–M1. Single file per workspace at `.kaizen/kaizen.db`.

## Alternatives
- DuckDB: better analytics, heavier dep — defer until 100k sessions.
- Postgres: requires server, overkill for local tool.
- Flat files: no query capability.

## Consequences
- Simple deploy: zero infra.
- WAL mode: concurrent reads + single writer safe.
- Re-evaluate DuckDB at 100k sessions or when analytics queries needed.
