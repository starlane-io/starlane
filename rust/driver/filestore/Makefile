build:
	#cargo build --target wasm32-wasi
	cargo wasix build

mv:
	#cp ./target/wasm32-wasi/debug/filestore.wasm ~/reference/wasmer/lib/driver-host/wasi.wasm
	cp ./target/wasm32-wasmer-wasi/debug/filestore.wasm ../../cosmic/cosmic-host/filestore.wasm
	#cp ./target/wasm32-wasmer-wasi/debug/filestore.wasm ../driver/driver-host/filestore.wasm

all: build mv
