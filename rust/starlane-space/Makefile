VERSION := $(shell cat ../../VERSION)

version: 
	toml set Cargo.toml dependencies.starlane-primitive-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml


publish-dry-run:
	cargo publish --dry-run --allow-dirty

publish:
	cargo publish
 
  
 

