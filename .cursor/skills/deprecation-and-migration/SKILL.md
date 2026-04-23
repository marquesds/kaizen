---
name: deprecation-and-migration
description: >
  Deprecation and migration patterns for Elixir/Phoenix. Use when removing old
  systems, migrating data, sunsetting features, or managing Ecto schema changes.
  Treats code as a liability — less code is better.
---

# Deprecation and Migration

Code is liability. Less code = less to maintain, test, debug.

## Code-as-Liability Mindset

Every line has cost. Before adding, ask:
- Solve with existing code?
- Remove instead of add?
- Will future-us curse this?

## Ecto Migration Patterns

### Write Reversible Migrations

```elixir
def change do
  alter table(:messages) do
    add :edited_at, :utc_datetime
    add :original_content, :text
  end

  create index(:messages, [:edited_at])
end
```

### Data Migrations (Separate from Schema)

```elixir
# Schema migration: add column
def change do
  alter table(:users) do
    add :display_name, :string
  end
end

# Data migration (separate file): backfill
def up do
  execute """
  UPDATE users SET display_name = first_name || ' ' || last_name
  WHERE display_name IS NULL
  """
end

def down do
  execute "UPDATE users SET display_name = NULL"
end
```

### Safe Column Removal (Two-Step)

```
Step 1: Stop using column in code (deploy)
Step 2: Remove column in migration (deploy after Step 1 stable)
```

Never remove column code still references.

## Deprecation Patterns

### Hard Deadline

```elixir
@deprecated "Use create_message/3 instead. Removed in v2.0"
def send_message(room, content) do
  create_message(room, current_user(), %{content: content})
end
```

### Soft Migration

```elixir
def old_function(args) do
  Logger.warning("old_function/1 deprecated, use new_function/1")
  new_function(args)
end
```

## Zombie Code Removal

Audit periodically:
- Unused functions (`mix xref`)
- Commented-out blocks
- Feature-flagged code where flag is always on/off
- Modules with no callers

```
DEAD CODE:
- Homunculus.Legacy.format_date/1 — replaced 3 months ago
- lib/homunculus_web/live/old_dashboard_live.ex — no route points here
→ Safe to remove?
```

## Migration Checklist

- [ ] Migration has `up`/`down` or `change`
- [ ] Data migration separate from schema migration
- [ ] No column removed while code references it
- [ ] Deprecated functions have `@deprecated`
- [ ] Dead code identified, scheduled for removal

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Might need later" | git has it. Dead code confuses now |
| "Not hurting anything" | Hurts readability, increases cognitive load |
| "Migration too risky" | Two-step reduces risk to near-zero |

## Red Flags

- Column removed while code still uses it
- Data + schema in same migration
- No `down/0` in migrations
- Deprecated code without `@deprecated`
- Dead code accumulating without cleanup schedule
