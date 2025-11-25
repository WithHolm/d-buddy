.PHONY: build run clean

build:
	cargo build

run:
    RUST_LOG=d_buddy=trace cargo run -- --log

perf:
	perf record -g --call-graph fp cargo run
	#--latency --call-graph cargo run

clean:
	cargo clean
