
clean:
	rm -rf target || true


all: 
	#cargo build --release --target wasm32-unknown-unknown
	cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/my_app.wasm  ../../bundle/wasm/my-app.wasm

debug: 
	cargo build --target wasm32-unknown-unknown

	cp ./target/wasm32-unknown-unknown/debug/my_app.wasm  ../../bundle/wasm/my-app.wasm




	

