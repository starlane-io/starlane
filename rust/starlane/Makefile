VERSION := $(shell cat ../../VERSION)

version:
	toml set Cargo.toml dependencies.starlane-macros-primitive.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml dependencies.starlane-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml dependencies.starlane-space.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml



publish-dry-run:
	cargo publish --dry-run --allow-dirty

publish:
	cargo publish
