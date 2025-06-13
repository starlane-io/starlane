
VERSION := $(shell toml get Cargo.toml workspace.package.version )
BRANCH := $(shell git rev-parse --abbrev-ref HEAD)
GIT_SHA_FETCH := $(shell git rev-parse HEAD | cut -c 1-8)

.PHONY : clean 


check: 
	num=$(shell git status --porcelain | wc -l)
	echo "NUM: ${num}"
	@echo "RESULT: $$?"

check-old:	

	#&& $(error local changes in '${BRANCH}' not commited to git) 
	$(shell git merge-base --is-ancestor HEAD @{u}  1> /dev/null 2> /dev/null) || $(error local commit for branch: '${BRANCH}' must be pushed to origin) 
	@echo $$?
	
blah:


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



