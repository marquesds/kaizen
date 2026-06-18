# Terminal UI

Kaizen's TUI provides a keyboard-driven live view of local sessions, events,
tool spans, and metrics. It reads bounded SQLite windows instead of loading the
whole workspace into memory.

## Start it

From the repository you want to inspect:

```bash
kaizen tui
```

To inspect another workspace:

```bash
kaizen tui --workspace /path/to/project
```

The TUI requires an interactive terminal. It watches the SQLite WAL and
coalesces refreshes, so active sessions update without a busy polling loop.

## Keys

| Key | Action |
|---|---|
| `j` / `k`, `Down` / `Up` | Move in the focused pane |
| `g` / `G` | Jump to first or last row |
| `Tab` | Switch pane |
| `Enter` | Open selected event detail |
| `m` | Toggle metrics |
| `/` | Filter sessions by case-insensitive agent prefix |
| `y` | Copy selected session ID |
| `r` | Refresh all visible data |
| `?` | Toggle in-app help |
| `Esc` or `Backspace` | Close the current overlay or detail view |
| `q` | Close an overlay, then quit from the main view |

While entering a filter, press `Enter` to apply it or `Esc` to cancel it.

## Bounded loading

The session list and selected-session detail load virtualized windows. Moving
through a large workspace requests only nearby rows, and duplicate in-flight
requests are suppressed. Store errors remain visible in the status line and
help view instead of terminating the interface.

## Troubleshooting

| Symptom | Action |
|---|---|
| No sessions appear | Run `kaizen sessions list --refresh`, then press `r`. |
| Wrong repository appears | Start with `--workspace /path/to/project`. |
| Navigation seems stuck | Press `Esc` to close overlays, then `Tab` to select the intended pane. |
| Terminal display is damaged after a crash | Run `reset`, then restart `kaizen tui`. |

