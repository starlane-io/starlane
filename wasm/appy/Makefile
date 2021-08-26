
clean:
	rm appy.zip
	rm -rf pkg
	rm -rf bundle


all: clean
	wasm-pack --verbose build
	mkdir -p bundle/wasm
	cp pkg/appy_bg.wasm bundle/wasm/appy.wasm
	cp -r conf/* bundle
	cd bundle && zip -r ../appy.zip .


	

