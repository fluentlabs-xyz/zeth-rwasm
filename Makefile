RUST_LOG=debug
#TARGET=release
TARGET=debug
BLOCK_NUMBER=3
RETH_URL=http://localhost
RETH_HTTP_RPC_PORT=8545
RETH_NETWORK_LISTENING_PORT=30305
NETWORK=ethereum
ZETH_RUN_BINARY_COMMAND=./target/$(TARGET)/zeth
ZETH_RUN_CARGO_COMMAND=cargo run --bin zeth
ZETH_OPTIONS=build --network=$(NETWORK) --eth-rpc-url=$(RETH_URL):$(RETH_HTTP_RPC_PORT) --block-number=$(BLOCK_NUMBER)

.PHONY: run_zeth_binary
run_zeth_binary:
	$(ZETH_RUN_BINARY_COMMAND) $(ZETH_OPTIONS)

.PHONY: zeth_binary_help
zeth_binary_help:
	$(ZETH_RUN_BINARY_COMMAND) $(ARGS) --help

.PHONY: run_zeth_cargo
run_zeth_cargo:
	$(ZETH_RUN_CARGO_COMMAND) -- $(ZETH_OPTIONS)


.PHONY: build
build:
	cargo build --profile=dev #-r
