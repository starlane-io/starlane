build:
	cargo build --target wasm32-wasi
	cargo wasix build

mv:
	cp ./target/wasm32-wasi/debug/filestore.wasm ~/reference/wasmer/lib/driver-host/wasi.wasm
	cp ./target/wasm32-wasmer-wasi/debug/filestore.wasm ~/reference/wasmer/lib/driver-host/wasix.wasm

all: build mv
