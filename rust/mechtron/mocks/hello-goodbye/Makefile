
clean:
	rm -rf target || true
	rm *wasm || true
	rm bundle.zip || true



all: 
	RUSTFLAGS="--cfg tokio_unstable" CARGO_TARGET_DIR=./target cargo build --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/hello_goodbye.wasm  .
	cp ./target/wasm32-unknown-unknown/release/hello_goodbye.wasm  bundle/wasm
	cd bundle && zip -r ../bundle.zip . 


debug: 
	cargo build --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/debug/hello_goodbye.wasm  .




	

