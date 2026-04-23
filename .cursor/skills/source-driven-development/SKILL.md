---
name: source-driven-development
description: >
  Cite HexDocs before writing framework code. Use when implementing unfamiliar
  Phoenix/LiveView/Ecto API or when user asks "is this the right way to X".
---

# Source-Driven Development

Every framework decision must be backed by official docs. Don't implement from memory — verify, cite, flag unverified.

## Process

```
DETECT → FETCH → IMPLEMENT → CITE
```

### Step 1: Detect Stack and Versions

Read `mix.exs` for exact versions:

```
STACK DETECTED:
- Phoenix 1.8.x (from mix.exs)
- LiveView 1.0.x
- Ecto 3.12.x
- Tailwind CSS 4.x
→ Fetching relevant HexDocs.
```

### Step 2: Fetch Official Docs

Source hierarchy (order of authority):

| Priority | Source |
|----------|--------|
| 1 | HexDocs (hexdocs.pm/phoenix, hexdocs.pm/ecto) |
| 2 | Official guides (hexdocs.pm/phoenix/overview.html) |
| 3 | Elixir core docs (hexdocs.pm/elixir) |
| 4 | Changelog / release notes |

**Not authoritative**: Stack Overflow, blog posts, tutorials, training data.

Be precise:

```
BAD:  Fetch the Phoenix homepage
GOOD: Fetch hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html#stream/4
```

### Step 3: Implement Following Documented Patterns

- Use API signatures from docs, not memory
- Docs show new way → use new way
- Docs deprecate pattern → don't use it

When docs conflict with existing code:

```
CONFLICT:
Existing code uses `live_redirect` but Phoenix 1.8 docs say
use `push_navigate` instead (deprecated in 1.7).
Source: hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html

Options:
A) Use modern pattern (push_navigate)
B) Match existing code for consistency
→ Which approach?
```

### Step 4: Cite Sources

```elixir
# Phoenix LiveView stream with reset for filtering
# Source: hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html#stream/4
socket
|> stream(:messages, messages, reset: true)
```

In conversation:

```
Using stream/4 with reset: true for filtering, per LiveView docs.
Source: hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html#stream/4
```

If docs not found:

```
UNVERIFIED: Could not find official docs for this pattern.
Based on training data — verify before using in production.
```

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Confident about this API" | Confidence not evidence. Verify |
| "Fetching docs wastes tokens" | Hallucinating API wastes more debugging time |
| "Simple task" | Wrong patterns on simple tasks become templates |

## Red Flags

- Writing Phoenix/Ecto code without checking HexDocs for that version
- Using "I believe" about API instead of citing source
- Using deprecated APIs (live_redirect, form_for, etc.)
- Not reading mix.exs before implementing
