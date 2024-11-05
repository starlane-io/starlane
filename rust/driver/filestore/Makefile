build:
	#cargo build --target wasm32-wasi
	cargo wasix build
	cargo build

mv:
	mkdir ../../cosmic/cosmic-host/bins || true
	cp ./target/wasm32-wasmer-wasi/debug/filestore.wasm ../../cosmic/cosmic-host/bins
	cp ./target/debug/filestore ../../cosmic/cosmic-host/bins
	rm ../../cosmic/cosmic-host/filestore.wasm.ser

clean: 
	rm -rf ../../cosmic-host/bins

all: build mv
