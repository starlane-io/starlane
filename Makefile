
VERSION := $(shell cat VERSION)
BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

.PHONY : clean version


check: 
	@git diff --exit-code 1> /dev/null 2> /dev/null || $(error local changes in '${BRANCH}' not commited to git) && exit 1
	@git merge-base --is-ancestor HEAD @{u}  1> /dev/null 2> /dev/null || $(error local commit for branch: '${BRANCH}' must be pushed to origin) && exit 1

	@echo $$?

clean :
	find . -type f -name "*.toml" -exec touch {} +
	find . -type f -name "Makefile" -exec touch {} +
	$(MAKE) -C rust clean 

version:
	$(MAKE) -C rust version

release: check
	echo ${COMMITED}
	exit 0
	git rev-parse --verify release/${VERSION} || exit 0
	git flow release start ${VERSION}
	git push --set-upstream origin release/${VERSION}
	gh release create v${VERSION}



publish-dry-run-impl: 
	rustup default stable
  

publish-impl:
	$(MAKE) -C rust publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl


t
