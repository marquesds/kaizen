# Web dashboard

Kaizen includes a read-only local dashboard for people who prefer a browser to
terminal output. It uses the same SQLite data and Rust query handlers as the CLI
and TUI.

## Open it

Run this from the repository you want to inspect:

```bash
kaizen open
```

Kaizen starts the local daemon when needed and opens an authenticated loopback
URL. To print the URL without launching a browser:

```bash
kaizen open --no-browser
```

## What you can inspect

The dashboard provides:

- project selection, including a manual local path;
- session, active-session, error, and cost totals;
- the latest 30 sessions for the selected project;
- selected-session facts, recent events, nested tool spans, touched files, and
  top tools;
- the exact bounded report under **Developer details**.

Selected-session detail is capped at 40 events, 40 spans, and 40 files. Those
limits keep refresh latency and memory use predictable. The page refreshes every
20 seconds while connected; **Refresh now** requests an immediate snapshot.

Web is an Observe-only surface. It cannot mutate experiments, guidance, sync,
configuration, or local data. Use the CLI or MCP for those workflows.

## Local security

The server binds to loopback. Data calls use an authenticated WebSocket, and the
token is included in the URL printed by Kaizen. Treat that URL as a local secret:
do not paste it into issues, chat, or logs.

Kaizen stores the restart-stable token in
`$KAIZEN_HOME/web_token.hex` (normally `~/.kaizen/web_token.hex`) with mode
`0600` on Unix. Static assets contain no session data. See [daemon.md](daemon.md)
for protocol and runtime-file details.

## Troubleshooting

| Symptom | Action |
|---|---|
| Browser did not open | Run `kaizen open --no-browser` and open the printed URL. |
| Page says connection failed | Run `kaizen daemon status`; restart with `kaizen daemon stop` followed by `kaizen open`. |
| URL has no valid token | Run `kaizen open --no-browser` again instead of editing the URL. |
| Expected project is missing | Open its path manually or run `kaizen sessions list --refresh` from that repository. |
| Default port is busy | Use the URL Kaizen prints; the daemon automatically chooses another loopback port. |

