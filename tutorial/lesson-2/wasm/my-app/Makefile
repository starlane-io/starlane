
clean:
	rm -rf pkg || true


all: clean
	wasm-pack --verbose build --release  --target nodejs --mode normal
	cp pkg/my_app_bg.wasm ../../bundle/wasm/my-app.wasm


	

