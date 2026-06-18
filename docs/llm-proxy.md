# LLM HTTP proxy

Run Kaizen in front of Anthropic-style or OpenAI-compatible model APIs so every model
call is logged locally with `EventSource::Proxy` events and Datadog-style `llm`
trace spans. Exports use the same redaction path as hooks.

## Security

- Default bind is loopback. Do not expose this server to a network without a separate TLS
  front (not included).
- The proxy forwards the client’s `x-api-key` to the real API. Do not point untrusted clients at
  this process.

## Daemon-owned proxy

After `kaizen init`, the daemon owns capture for the workspace. Run
`kaizen init --deep` to have the daemon start loopback proxy tasks and report
which providers are ready. This does not silently rewrite agent provider config
unless Kaizen can verify a supported setting; unsupported agents stay on
hook/transcript capture and are reported as partial deep capture.

`kaizen observe --agent <claude|codex|cursor|auto> -- <cmd>` is a manual wrapper
around the same daemon proxy endpoints. It injects `KAIZEN_SESSION_KEY`,
`X_KAIZEN_SESSION`, and the provider base URL env vars into the child command.

## Manual proxy

1. Start the proxy in a workspace. It uses that workspace's database under
   `$KAIZEN_HOME/projects/<slug>/kaizen.db` (normally
   `~/.kaizen/projects/<slug>/kaizen.db`):

   ```bash
   kaizen proxy run
   ```

2. Point tools at the proxy instead of `https://api.anthropic.com`, for example:

   ```bash
   export ANTHROPIC_BASE_URL="http://127.0.0.1:3847"
   # ANTHROPIC_API_KEY unchanged
   ```

   For Codex/OpenAI-compatible clients:

   ```bash
   kaizen proxy run --provider openai
   export OPENAI_BASE_URL="http://127.0.0.1:3847/v1"
   # OPENAI_API_KEY unchanged
   ```

3. Optional: tie multiple requests to one Kaizen `session` row:

   ```bash
   # Any HTTP client can send a stable id:
   X-Kaizen-Session: my-coding-session-1
   ```

   If missing, the proxy issues a new `proxy-<uuid-v7>` id per HTTP request.

## Config (`[proxy]`)

The project-data `config.toml` and `~/.kaizen/config.toml` support:

| Key | Default | Notes |
|-----|---------|--------|
| `listen` | `127.0.0.1:3847` | `--listen` overrides |
| `upstream` | `https://api.anthropic.com` | No trailing slash; `--upstream` overrides |
| `provider` | `anthropic` | `anthropic`, `openai`, or `auto`; `openai` changes the default upstream to `https://api.openai.com` |
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

- One `Cost` event per successful HTTP completion with optional Anthropic or OpenAI
  token fields parsed from JSON or `text/event-stream` bodies, plus path, method,
  status, latency, cache tokens, stop reason, and context usage where available.
- One `llm` trace span per proxied round trip, with redacted metadata only. Raw
  prompts and raw model responses are not written into `payload` fields.
- `Error` when the proxy cannot talk to the upstream, read the body, or when you exceed
  `max_response_body_mb`.

## Limits (v1)

- Each upstream response is fully buffered in memory (see `max_response_body_mb`). Very large
  streams are not teed; raise the cap or avoid the proxy for huge downloads.
- The proxy does not terminate TLS; use `https` upstream and `http` locally as usual for dev.

## Model checking

- [`specs/llm-proxy.qnt`](../specs/llm-proxy.qnt) for abstract lifecycle invariants.
- `cargo test -p kaizen-cli --test llm_proxy` replays the Quint spec via `quint-connect`.
