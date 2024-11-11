
VERSION := $(shell toml get ../Cargo.toml workspace.package.version )
BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

.PHONY : clean version

clean :
	cargo clean

version:
	toml set Cargo.toml dependencies.starlane-macros ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml dependencies.starlane-primitive-macros ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	echo "starlane version set to ${VERSION}"



