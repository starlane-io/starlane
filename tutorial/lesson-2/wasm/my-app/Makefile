
clean:
	rm -rf pkg || true


all: clean
	wasm-pack --verbose build --release  --target nodejs --mode normal
	#wasm-pack --verbose build ---target web --mode normal
	cp pkg/my_app_bg.wasm ../../bundle/wasm/my-app.wasm


	

