BINARY_NAME=sepuh
# PROMPT=""
# PROMPT="eli5 general relativity"
# PROMPT="write me 50 words haiku"
PROMPT="who r u \(specific version\)"

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
	SESEPUH_HUB_VENDOR=anthropic SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

google:
	SESEPUH_HUB_VENDOR=google SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

ollama:
	SESEPUH_HUB_VENDOR=ollama SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

openai:
	SESEPUH_HUB_VENDOR=openai SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

openrouter:
	SESEPUH_HUB_VENDOR=openrouter SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

xai:
	SESEPUH_HUB_VENDOR=xai SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

.PHONY: all dev build release start clean anthropic google ollama openai openrouter xai
