VERSION := $(shell cat ../../../VERSION)

version:
		toml set Cargo.toml dependencies.cosmic-space.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
		toml set Cargo.toml dependencies.cosmic-hyperspace.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
		toml set Cargo.toml dependencies.cosmic-hyperlane.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
		toml set Cargo.toml dependencies.cosmic-hyperlane-tcp.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
		toml set Cargo.toml dependencies.mechtron-host.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
		toml set Cargo.toml dependencies.cosmic-registry-postgres.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml




publish-dry-run:
	cargo publish --dry-run --allow-dirty

publish:
	cargo publish
