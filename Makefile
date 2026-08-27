BINARY_NAME=sepuh
# PROMPT=""
# PROMPT="eli5 general relativity"
# PROMPT="write me 50 words haiku"
# PROMPT="who r u \(specific version\)"
PROMPT="update US10Y rate"
# PROMPT="update closing level COMPOSITE INDEX today"

all: dev

dev:
	cargo run -- --prompt $(PROMPT)

build:
	cargo build

release:
	cargo build --release

start:
	./target/release/$(BINARY_NAME) --prompt $(PROMPT)

clean:
	cargo clean

anthropic:
	SEPUH_PROVIDER=anthropic SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

google:
	SEPUH_PROVIDER=google SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

ollama:
	SEPUH_PROVIDER=ollama SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

openai:
	SEPUH_PROVIDER=openai SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

openrouter:
	SEPUH_PROVIDER=openrouter SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

opencode:
	SEPUH_PROVIDER=opencode SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

opencode-reasoning:
	SEPUH_PROVIDER=opencode SEPUH_MODEL='' SEPUH_REASONING=1 cargo run -- --prompt $(PROMPT)

opencode2:
	SEPUH_PROVIDER=opencode SEPUH_MODEL='' OPENCODE_USE_VERSION=2 cargo run -- --prompt $(PROMPT)

opencode2-reasoning:
	SEPUH_PROVIDER=opencode SEPUH_MODEL='' OPENCODE_USE_VERSION=2 SEPUH_REASONING=1 cargo run -- --prompt $(PROMPT)

xai:
	SEPUH_PROVIDER=xai SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

xai-web:
	SEPUH_PROVIDER=xai SEPUH_MODEL='' SEPUH_WEB_SEARCH=1 cargo run -- --prompt $(PROMPT)

xai-x:
	SEPUH_PROVIDER=xai SEPUH_MODEL='' SEPUH_X_SEARCH=1 cargo run -- --prompt $(PROMPT)

xai-search:
	SEPUH_PROVIDER=xai SEPUH_MODEL='' SEPUH_WEB_SEARCH=1 SEPUH_X_SEARCH=1 cargo run -- --prompt $(PROMPT)

.PHONY: all dev build release start clean anthropic google ollama openai openrouter opencode opencode-reasoning opencode2 opencode2-reasoning xai xai-web xai-x xai-search
