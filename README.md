# sepuh

`sepuh` is a lighter version of [`sesepuh-hub`](https://github.com/frizadiga/sesepuh-hub), focused on lower memory fingerprint and faster startup time.

It keeps the same simple CLI shape (`--prompt`) and environment-variable based config, but trims fz scope to essential vendors.

## Current scope

- Vendor support: `openai`, `xai`
- Sync and streaming response modes
- Optional response-only output mode
- Writes final response to a file for downstream tooling

## Why sepuh

- Smaller runtime surface
- Faster cold start
- Simpler dependency and feature set
- Good fit for scripts and terminal workflows that need quick LLM calls

## Build and run

```bash
make release
./target/release/sepuh --prompt "eli5 general relativity"
```

## Configuration

Required by selected vendor:

- `SESEPUH_HUB_VENDOR`: `openai` or `xai`
- `OPENAI_API_KEY` for OpenAI
- `XAI_API_KEY` for xAI

Optional:

- `SESEPUH_HUB_MODEL` (global model override)
- `OPENAI_MODEL` (default: `gpt-4o-mini`)
- `XAI_MODEL` (default: `grok-2-latest`)
- `XAI_URL` (default: `https://api.x.ai/v1`)
- `SESEPUH_HUB_STREAMING=1` to stream tokens
- `SESEPUH_HUB_RES_ONLY=1` to suppress banner/model info

## Response file

By default, final response content is written to:

- `$XDG_CONFIG_HOME/sepuh/.response.txt`
- fallback: `$HOME/.config/sepuh/.response.txt` when `XDG_CONFIG_HOME` is unset

This is useful when you want machine-readable output without parsing terminal logs.

## Development shortcuts

The `Makefile` includes:

- `make dev PROMPT="..."`
- `make openai PROMPT="..."`
- `make xai PROMPT="..."`
- `make release`

## License

MIT. See [LICENSE](LICENSE).
