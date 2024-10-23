
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
	toml set Cargo.toml workspace.dependencies.starlane-primitive-macros.version ${VERSION} > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	#$(MAKE) -C starlane-space version
	#$(MAKE) -C starlane-macros version
	#$(MAKE) -C starlane version

publish-dry-run-impl: 
	rustup default stable
  

publish-impl:
	./publish.sh starlane-primitive-macros 10
	./publish.sh starlane-space  10
	./publish.sh starlane-macros 10
	./publish.sh starlane 0

yokel:
	#@echo $(shell echo 'doing stuff'; exit 123)
	#@echo 'command exited with $(.SHELLSTATUS)'
	#@exit $(.SHELLSTATUS)

	ifeq ($$?,0)
		echo "Published"
	else 
		echo "Not Published"
	endif

	exit 0

	cargo info starlane-primitive-macros@${version} || echo "starlane-primitive-macros@${version} already published" || $(MAKE) -C starlane-primitive-macros publish || true


	exit 0
	$(MAKE) -C starlane-primitive-macros publish || true
	sleep 30
	$(MAKE) -C starlane-space publish
	sleep 30
	$(MAKE) -C starlane-macros publish
	sleep 30
	$(MAKE) -C starlane/starlane publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl

