
clean:
	rm -rf target || true
	rm *wasm


all: 
	#cargo build --release --target wasm32-unknown-unknown
	CARGO_TARGET_DIR=./target cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/my_app.wasm  .

debug: 
	cargo build --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/debug/my_app.wasm  .




	

