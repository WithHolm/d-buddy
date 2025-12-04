.PHONY: build run clean

UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Linux)
    # OK
else
    $(error This Makefile can only be run on Linux)
endif

ai-check:
	cargo check && cargo fmt #&& cargo clippy

ai-run:
	cargo run -- --check

build:
	cargo build

# run as cargo with logging and tracing enabled..
run:
	RUST_LOG=d_buddy=trace cargo run -- --log

perf:
	perf record -g --call-graph fp cargo run

clippy-fix:
	cargo clippy --fix --bin "d-buddy"
