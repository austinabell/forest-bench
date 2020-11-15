install:
	cargo install --locked --force --path .

build:
	cargo build

release:
	cargo build --release

.PHONY: build release install
