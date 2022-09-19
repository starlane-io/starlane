VERSION := $(shell cat ../../../VERSION)

version:
	toml set Cargo.toml package.version ${VERSION} > Cargo.toml.tmp
	mv Cargo.toml.tmp Cargo.toml

publish-dry-run:
	cargo publish --dry-run --allow-dirty
 
 
publish:
	cargo publish
