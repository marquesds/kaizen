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

- project selection, manual local-path fallback, and refresh controls in the
  responsive top navigation;
- session, active-session, error, and cost totals;
- project-level tool, attention, and telemetry-coverage insights; Tool Pattern
  lists the selected session's three most frequent recent shell commands;
- 30 sessions per page, with Previous and Next controls;
- one session search field that ranks prompt matches first, then matches session
  ID, agent, model, status, branch, and tool;
- selected-session prompt, facts, recent events with bounded command details,
  nested tool spans, touched files, and top tools;
- the exact bounded report under **Developer details**.

Search accepts up to 256 characters. Search results and page controls use the
filtered count, while summary cards continue to show project-wide totals.
Changing the search starts at the first page. Automatic and manual refreshes
preserve the active search and page; if data changes and removes that page,
Kaizen returns to the last available page.

Selected-session detail is capped at 40 events, 40 spans, and 40 files. These
limits keep refresh latency and memory use predictable. The server watches the
selected project's SQLite database and WAL; a committed change requests a new
snapshot within one second. **Refresh now** remains available in the top
navigation for manual checks.

`No completion` means Kaizen received activity but no final session event for
at least 30 minutes. It does not mean the work failed. Models and prompts remain
`Unknown` or unavailable when no captured source provides them.

Web is an Observe-only surface. It cannot mutate experiments, guidance, sync,
configuration, or local data. Use the CLI or MCP for those workflows.

## Local security

The server binds to loopback. Data calls use an authenticated WebSocket, and the
token is included in the URL printed by Kaizen. Treat that URL as a local secret:
do not paste it into issues, chat, or logs.

Prompts and command summaries can contain source code or secrets. They stay in
the authenticated loopback response and are never added to static assets. Raw
event payloads remain omitted.

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
| Expected project is missing | Run one Kaizen command from that repository to register it, then reload. Unsafe roots containing `KAIZEN_HOME` and missing paths are ignored. |
| Default port is busy | Use the URL Kaizen prints; the daemon automatically chooses another loopback port. |
