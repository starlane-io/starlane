
clean:
	rm -rf target || true
	rm *wasm || true
	rm bundle.zip || true
	rm bundle/wasm/*wasm || true



all: 
	CARGO_TARGET_DIR=./target cargo build --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/lesson_1.wasm  .
	cp ./target/wasm32-unknown-unknown/release/lesson_1.wasm  bundle/wasm
	cd bundle && zip -r ../bundle.zip . 


debug: 
	cargo build --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/debug/lesson-1.wasm  .




	

