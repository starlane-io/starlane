
VERSION := $(shell toml get Cargo.toml workspace.package.version )
BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

.PHONY : clean 


check: 
	@echo ${VERSION}
	@echo ${BRANCH}
	@git diff --exit-code
	@echo $$?
	@git diff --exit-code 1> /dev/null 2> /dev/null || $(error local changes in '${BRANCH}' not commited to git) && exit 1
	
blah:
	@git merge-base --is-ancestor HEAD @{u}  1> /dev/null 2> /dev/null || $(error local commit for branch: '${BRANCH}' must be pushed to origin) && exit 1

	@echo $$?

clean :
	cargo clean
	find . -type f -name "*.toml" -exec touch {} +
	find . -type f -name "Makefile" -exec touch {} +
	$(MAKE) -C starlane clean 

version:
	$(MAKE) -C starlane-primitive-macros version
	$(MAKE) -C starlane-macros version
	$(MAKE) -C starlane version

release: check
	echo ${COMMITED}
	git rev-parse --verify v${VERSION} || exit 0
	git flow release start ${VERSION}
	git push --set-upstream origin release/${VERSION}
	gh release create v${VERSION}


publish-dry-run-impl: 
	rustup default stable
  

publish-impl:
	$(MAKE) -C rust publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl



