build:
	#cargo build --target wasm32-wasi
	cargo wasix build

mv:
	cp ./target/wasm32-wasmer-wasi/debug/filestore.wasm ../../cosmic/cosmic-host/filestore.wasm
	rm ../../cosmic/cosmic-host/filestore.wasm.ser


all: build mv
