
clean:
	rm appy.zip || true
	rm -rf pkg || true
	rm -rf bundle || true


all: clean
	wasm-pack --verbose build --release  --target nodejs --mode normal
	mkdir -p bundle/wasm
	cp pkg/appy_bg.wasm bundle/wasm/appy.wasm
	cp -r conf/* bundle
	cd bundle && zip -r ../appy.zip .


	

