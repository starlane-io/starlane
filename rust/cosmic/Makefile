version:
	$(MAKE) -C cosmic-nom version
	$(MAKE) -C cosmic-macros-primitive version
	$(MAKE) -C cosmic-universe version
	$(MAKE) -C cosmic-macros version
	$(MAKE) -C cosmic-hyperlane version
	$(MAKE) -C cosmic-hyperlane-tcp version
	$(MAKE) -C cosmic-hyperverse version
	$(MAKE) -C cosmic-cli version

publish-dry-run:
	$(MAKE) -C cosmic-nom publish-dry-run
	$(MAKE) -C cosmic-macros-primitive publish-dry-run
	$(MAKE) -C cosmic-universe publish-dry-run
	$(MAKE) -C cosmic-macros publish-dry-run
	$(MAKE) -C cosmic-hyperlane publish-dry-run
	$(MAKE) -C cosmic-hyperlane-tcp publish-dry-run
	$(MAKE) -C cosmic-hyperverse publish-dry-run
	$(MAKE) -C cosmic-cli publish-dry-run

publish:
	#$(MAKE) -C cosmic-nom publish
	#$(MAKE) -C cosmic-macros-primitive publish
	#sleep 60
	#$(MAKE) -C cosmic-universe publish
	#sleep 60
	#$(MAKE) -C cosmic-macros publish
	#sleep 60
	#$(MAKE) -C cosmic-hyperlane publish
	#sleep 60
	$(MAKE) -C cosmic-hyperlane-tcp publish
	sleep 60
	$(MAKE) -C cosmic-hyperverse publish
	$(MAKE) -C cosmic-cli publish

 
