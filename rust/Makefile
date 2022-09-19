
.PHONY : clean version
clean :
	find . -type f -name "*.toml" -exec touch {} +
	find . -type f -name "Makefile" -exec touch {} +


version:
	cp ../VERSION cosmic/cosmic-universe/src
	$(MAKE) -C cosmic

publish-dry-run-impl: 
	rustup default stable
	$(MAKE) -C cosmic publish-dry-run
  

publish-impl:
	rustup default stable
	$(MAKE) -C cosmic publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl
 
