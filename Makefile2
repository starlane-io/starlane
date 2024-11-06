
VERSION := $(shell cat ../VERSION)

.PHONY : clean version
clean :
	find . -type f -name "*.toml" -exec touch {} +
	find . -type f -name "Makefile" -exec touch {} +
	cargo clean

version:
	toml set Cargo.toml workspace.package.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-space.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-primitive-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	#$(MAKE) -C starlane-space version
	#$(MAKE) -C starlane-macros version
	#$(MAKE) -C starlane version

publish-dry-run-impl: 
	rustup default stable
  

publish-impl:
	rustup default stable
	./publish.sh starlane-primitive-macros 10
	./publish.sh starlane-space  10
	./publish.sh starlane-macros 10
	./publish.sh starlane 0


publish-dry-run: version publish-dry-run-impl
publish: version publish-impl

