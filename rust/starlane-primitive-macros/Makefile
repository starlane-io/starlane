VERSION := $(shell cat ../../VERSION)
PUB := $(shell cargo show --json starlane-primitive-macros | jq ".versions[0].num" )

version:

publish-dry-run:
	echo $(PUB)
	ifneq( $(VERSION), $(PUB) )
		echo "PUBLISH!"
		#cargo publish --dry-run --allow-dirty
	endif
 
 
publish:
	cargo publish
