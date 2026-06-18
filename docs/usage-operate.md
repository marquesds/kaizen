# Operate and Integrate

[Back to CLI index](usage.md).

## `kaizen upgrade`

Detects the install method, downloads the matching release asset when
appropriate, verifies its SHA-256 checksum, and replaces the current binary.

| Install method | Action |
|---|---|
| Homebrew | Runs `brew upgrade kaizen-cli`. |
| Release or Cargo-path binary | Downloads, verifies, and replaces the binary. |
| Source fallback | `kaizen upgrade --from-source` runs `cargo install kaizen-cli --locked --force`. |

```bash
kaizen upgrade
```

Current releases use SQLite only. Legacy `hot/` and `cold/` artifacts are
ignored. Before deleting them, confirm
`~/.kaizen/projects/<slug>/kaizen.db` is intact. A previous `migrate v2` run
also created `kaizen.db.v1.bak`.

## `kaizen gc`

Deletes sessions and dependent rows older than `[retention].hot_days`, or an
explicit `--days` value. `hot_days = 0` disables automatic pruning but does not
permit `gc` without a positive `--days`.

```bash
kaizen gc
kaizen gc --days 14
kaizen gc --vacuum
```

`--vacuum` shrinks the SQLite file after deletion and can be slow.

## `kaizen completions`

Prints a shell completion script to stdout. Supported shells are bash, elvish,
fish, PowerShell, and zsh.

```bash
kaizen completions bash > ~/.local/share/bash-completion/completions/kaizen
kaizen completions zsh | sudo tee /usr/local/share/zsh/site-functions/_kaizen
kaizen completions fish > ~/.config/fish/completions/kaizen.fish
```

Redirect stdout to the path your shell expects, then restart or reload it.

## `kaizen proxy run`

Starts a local Anthropic-style or OpenAI-compatible HTTP forwarder. Proxy
events are stored in the project's SQLite database.

```bash
kaizen proxy run
kaizen proxy run --listen 127.0.0.1:9000
kaizen proxy run --upstream https://api.anthropic.com
kaizen proxy run --provider openai
kaizen observe --agent codex -- codex
```

For normal collection, prefer `kaizen init`. `kaizen observe` is a
daemon-backed debug wrapper for one child command. Use
`kaizen sessions trace <id>` for proxy-backed LLM spans and
`kaizen metrics quality --json` for coverage and trace-correlation health. See
[llm-proxy.md](llm-proxy.md).

## `kaizen ingest hook`

Reads one hook event from stdin and appends it to SQLite. `kaizen init` wires
this command; users rarely call it directly.

```bash
kaizen ingest hook --source cursor < event.json
kaizen ingest hook --source claude < event.json
```

## `kaizen sync`

Flushes the redacted outbox to the configured ingest endpoint.

```bash
kaizen sync run
kaizen sync run --once
kaizen sync status
```

See [ingest-contract.md](ingest-contract.md).

## `kaizen mcp`

Starts the Model Context Protocol server over stdio. Most CLI workflows are
available as MCP tools. `doctor`, `guidance`, `gc`, `completions`, `proxy run`,
and all telemetry subcommands remain shell-only. See [mcp.md](mcp.md).
