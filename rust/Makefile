
VERSION := $(shell cat ../VERSION)

.PHONY : clean version
clean :
	find . -type f -name "*.toml" -exec touch {} +
	find . -type f -name "Makefile" -exec touch {} +


version:
	toml set Cargo.toml workspace.package.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-space.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	toml set Cargo.toml workspace.dependencies.starlane-macros-primitive.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

publish-dry-run-impl: 
	rustup default stable
  

publish-impl:
	rustup default stable
	$(MAKE) -C starlane-primitive-macros publish || true
	sleep 30
	$(MAKE) -C starlane-space publish
	sleep 30
	$(MAKE) -C starlane-macros publish
	sleep 30
	$(MAKE) -C starlane/starlane publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl
 
