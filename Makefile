.PHONY: build
build:
	cargo build

.PHONY: test
test:
	cargo test

.PHONY: bench
bench:
	cargo bench -p isahc-benchmarks
