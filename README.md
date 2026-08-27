# sepuh

`sepuh` is a lighter version of [`sesepuh-hub`](https://github.com/frizadiga/sesepuh-hub), focused on lower memory fingerprint and faster startup time.

It keeps the same simple CLI shape (`--prompt`) and environment-variable based config, but trims fz scope to essential providers.

## Current scope

- Vendor support: 
  - `ollama` also support local network
  - `xai`
  - `google`
  - `openai`
  - `anthropic`
  - Unified gateway like `openrouter`
  - `opencode` (local opencode agent server)
- Sync and streaming response modes
- Optional response-only output mode
- Writes final response to a file for downstream tooling

## Why sepuh

- Smaller runtime surface
- Smaller memory footprint
- Faster cold start
- Simpler dependency and feature set
- Good fit for scripts and terminal workflows that need quick LLM calls

## Build and run

```bash
make release
./target/release/sepuh --prompt "eli5 general relativity"
```

## Configuration

Required by selected provider:

- `SEPUH_PROVIDER`: `openai`, `xai`, or `anthropic`
- `OPENAI_API_KEY` for OpenAI
- `XAI_API_KEY` for xAI
- `ANTHROPIC_API_KEY` for Anthropic

Optional:

- `SEPUH_MODEL` (global model override)
- `SEPUH_STREAMING=1` to stream tokens
- `SEPUH_RES_ONLY=1` to suppress banner/model info

### OpenCode server version

The `opencode` provider talks to a local OpenCode agent server. Two server
generations are supported, selected with `OPENCODE_USE_VERSION` (`1` or `2`,
default `1`):

- **v1** (`opencode serve`, default `http://localhost:4096`)
- **v2** (`opencode2 serve`, default `http://localhost:4097`)

```bash
export OPENCODE_USE_VERSION='1'   # 1 | 2
```

Both use HTTP Basic auth with username `opencode` and the password from
`OPENCODE_SERVER_PASSWORD`. Override the base URL with `OPENCODE_BASE_URL` and
the agent with `OPENCODE_AGENT` (default `plan`).

In v2 the model override (`OPENCODE_MODEL`, e.g. `openrouter/z-ai/glm-5.2`) is
sent as `providerID`/`id` at session creation; the assistant reply is streamed
back over the `/api/event` SSE channel and terminates on `session.execution.succeeded`.
- `SEPUH_REASONING=1` to stream reasoning/thinking tokens to stderr (default: hidden)

## Response file

By default, final response content is written to:

- `$XDG_CONFIG_HOME/sepuh/.response.txt`
- fallback: `$HOME/.config/sepuh/.response.txt` when `XDG_CONFIG_HOME` is unset

This is useful when you want machine-readable output without parsing terminal logs.

## Development shortcuts

The `Makefile` includes:

- `make dev PROMPT="..."`
- `make anthropic PROMPT="..."`
- `make openai PROMPT="..."`
- `make xai PROMPT="..."`
- `make opencode PROMPT="..."`
- `make opencode-reasoning PROMPT="..."` (streams reasoning to stderr)
- `make opencode2 PROMPT="..."` (targets the v2 `opencode2` server)
- `make opencode2-reasoning PROMPT="..."` (v2, streams reasoning to stderr)
- `make release`

## License

MIT. See [LICENSE](LICENSE).
