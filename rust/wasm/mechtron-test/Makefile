
clean:
	rm -rf target || true
	rm -rf bin || true
	rm -rf ../../target || true
  


all: 
	#cargo build --release --target wasm32-unknown-unknown
	CARGO_TARGET_DIR=./target cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --release --target wasm32-unknown-unknown
	mkdir bin || true
	cp ../../target/wasm32-unknown-unknown/release/mechtron_test.wasm  bin

debug: 
	cargo build --target wasm32-unknown-unknown
	cp ../../target/wasm32-unknown-unknown/debug/mechtron_test.wasm  bin


	

