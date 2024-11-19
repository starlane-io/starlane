
VERSION := $(shell toml get ../Cargo.toml workspace.package.version )
BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

.PHONY : clean version

clean :
	cargo clean

version:
	echo "starlane-primitive-macros version set to ${VERSION}"



