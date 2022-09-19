
version: 
	$(MAKE) -C cosmic

publish-dry-run-impl: 
	rustup default stable
	$(MAKE) -C cosmic publish-dry-run
  

publish-impl:
	rustup default stable
	$(MAKE) -C cosmic publish

publish-dry-run: version publish-dry-run-impl
publish: version publish-impl
 
