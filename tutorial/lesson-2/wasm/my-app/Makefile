
clean:
	rm -rf target || true


all: clean
	cargo build --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/debug/my_app.wasm  ../../bundle/wasm/my-app.wasm


	

