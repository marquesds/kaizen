# LLM HTTP proxy (Anthropic)

Run Kaizen in front of the Anthropic Messages API (and paths that use the same base URL and
headers) so every model call is logged locally with `EventSource::Proxy` events
and can sync to your ingest outbox with the same redaction as hooks.

## Security

- Default bind is loopback. Do not expose this server to a network without a separate TLS
  front (not included).
- The proxy forwards the client’s `x-api-key` to the real API. Do not point untrusted clients at
  this process.

## How to run

1. Start the proxy in a workspace (creates/uses `.kaizen/kaizen.db` there):

   ```bash
   kaizen proxy run
   ```

2. Point tools at the proxy instead of `https://api.anthropic.com`, for example:

   ```bash
   export ANTHROPIC_BASE_URL="http://127.0.0.1:3847"
   # ANTHROPIC_API_KEY unchanged
   ```

3. Optional: tie multiple requests to one Kaizen `session` row:

   ```bash
   # Any HTTP client can send a stable id:
   X-Kaizen-Session: my-coding-session-1
   ```

   If missing, the proxy issues a new `proxy-<uuid-v7>` id per HTTP request.

## Config (`[proxy]`)

Workspace `/.kaizen/config.toml` and `~/.kaizen/config.toml` (same merge as the rest of Kaizen)
support:

| Key | Default | Notes |
|-----|---------|--------|
| `listen` | `127.0.0.1:3847` | `--listen` overrides |
| `upstream` | `https://api.anthropic.com` | No trailing slash; `--upstream` overrides |
| `compress_transport` | `true` | `no_gzip` on the HTTP client when `false` (see [reqwest](https://docs.rs/reqwest)) |
| `minify_json` | `true` | Re-encode JSON bodies to compact `serde_json` (whitespace only) |
| `max_response_body_mb` | `256` | Single-response buffer cap; raise if the proxy returns 502 from slurping a larger body |
| `max_request_body_mb` | `32` | `DefaultBodyLimit` for incoming client bodies before forward |
| `context_policy` | `none` | See below (optional **billed** input reduction) |

### Default “compression” (A)

`minify_json` and `compress_transport` do **not** by themselves change Anthropic’s billed
tokens; they cut whitespace and (when enabled) use normal HTTP `Accept-Encoding` behavior on the
client to upstream. To actually drop context sent to the model, use a **context policy** (B).

### Context policy (B, opt-in)

TOML examples:

```toml
[proxy]
context_policy = { type = "last_messages", count = 20 }
# or
context_policy = { type = "max_input_tokens", max = 200000 }
# or
context_policy = { type = "none" }
```

- `last_messages` keeps the last `count` items in the JSON `messages` array (and leaves `system` blocks alone when present in the same object).
- `max_input_tokens` uses a `chars/4` heuristic to drop the oldest `messages` until the estimate
  is at or below `max`. This is a rough **budget** — not a tokenizer.

## What gets stored

- One `Cost` event per successful HTTP completion with optional `input_tokens` / `output_tokens`
  parsed from JSON or `text/event-stream` bodies, plus `path`, `method`, and HTTP status in the
  payload. Raw prompts are not written into `payload` fields.
- `Error` when the proxy cannot talk to the upstream, read the body, or when you exceed
  `max_response_body_mb`.

## Limits (v1)

- Each upstream response is fully buffered in memory (see `max_response_body_mb`). Very large
  streams are not teed; raise the cap or avoid the proxy for huge downloads.
- The proxy does not terminate TLS; use `https` upstream and `http` locally as usual for dev.

## Model checking

- [`specs/llm-proxy.qnt`](../specs/llm-proxy.qnt) for abstract lifecycle invariants.
- `cargo test -p kaizen-cli --test llm_proxy` replays the Quint spec via `quint-connect`.
