
clean:
	rm -rf target || true
	rm bundle/wasm/*
	rm bundle.zip


all: 
	CARGO_TARGET_DIR=./target cargo build --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/hello.wasm bundle/wasm
	zip -r bundle.zip bundle


	

