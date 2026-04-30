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
	SEPUH_VENDOR=anthropic SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

google:
	SEPUH_VENDOR=google SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

ollama:
	SEPUH_VENDOR=ollama SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

openai:
	SEPUH_VENDOR=openai SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

openrouter:
	SEPUH_VENDOR=openrouter SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

xai:
	SEPUH_VENDOR=xai SEPUH_MODEL='' cargo run -- --prompt $(PROMPT)

xai-web:
	SEPUH_VENDOR=xai SEPUH_MODEL='' SEPUH_WEB_SEARCH=1 cargo run -- --prompt $(PROMPT)

xai-x:
	SEPUH_VENDOR=xai SEPUH_MODEL='' SEPUH_X_SEARCH=1 cargo run -- --prompt $(PROMPT)

xai-search:
	SEPUH_VENDOR=xai SEPUH_MODEL='' SEPUH_WEB_SEARCH=1 SEPUH_X_SEARCH=1 cargo run -- --prompt $(PROMPT)

.PHONY: all dev build release start clean anthropic google ollama openai openrouter xai xai-web xai-x xai-search
