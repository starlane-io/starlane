
clean:
	rm my-mechtron.zip || true
	rm -rf pkg || true
	rm -rf bundle || true


all: clean
	wasm-pack --verbose build --release  --target nodejs --mode normal
	mkdir -p bundle/wasm
	cp pkg/my_mechtron_bg.wasm bundle/wasm/my-mechtron.wasm
	cp -r conf/* bundle
	cd bundle && zip -r ../my-mechtron.zip .


	

