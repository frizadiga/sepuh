BINARY_NAME=sepuh
# PROMPT=""
# PROMPT="eli5 general relativity"
# PROMPT="write me 50 words haiku"
PROMPT="what model you currently use"

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

openai:
	SESEPUH_HUB_VENDOR=openai SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

xai:
	SESEPUH_HUB_VENDOR=xai SESEPUH_HUB_MODEL='' cargo run -- --prompt $(PROMPT)

.PHONY: all dev build release start clean openai xai
