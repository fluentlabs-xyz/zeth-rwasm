RUST_LOG=debug
CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=true

#TARGET=release
TARGET=debug
#BLOCK_NUMBER=3
#BLOCK_NUMBER=16424130
BLOCK_NUMBER=17034871
RETH_URL=http://localhost
RETH_HTTP_RPC_PORT=8545
RETH_NETWORK_LISTENING_PORT=30305
NETWORK=ethereum
ZETH_RUN_BINARY_COMMAND=./target/$(TARGET)/zeth
ZETH_RUN_CARGO_COMMAND=cargo run --bin zeth
ZETH_OPTIONS_BASE=build --network=$(NETWORK) --block-number=$(BLOCK_NUMBER)
#ZETH_OPTIONS=$(ZETH_OPTIONS_BASE) --eth-rpc-url=$(RETH_URL):$(RETH_HTTP_RPC_PORT)
ZETH_OPTIONS=$(ZETH_OPTIONS_BASE) --cache=host/testdata

.PHONE: all
all:
	$(MAKE) build

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


TARGET_WASM=wasm32-unknown-unknown

.PHONY: build_stf
build_stf:
	cargo b -r --manifest-path ./stf/Cargo.toml --target=${TARGET_WASM}
	mkdir -p build
	cp ./target/${TARGET_WASM}/release/fluent_stf.wasm ./build/fluent-stf.wasm
	wasm2wat ./build/fluent-stf.wasm > ./build/fluent-stf.wat
	du -sch build/*

