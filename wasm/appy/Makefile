
clean:
	rm appy.zip
	rm -rf pkg
	rm -rf bundle


all:
	wasm-pack --verbose build
	mkdir -p bundle/wasm
	cp pkg/appy_bg.wasm bundle/wasm/appy.wasm
	cp -r conf/* bundle
	zip -r appy.zip bundle


	

