VERSION := $(shell cat ../../VERSION)
NAME := $(shell toml get Cargo.toml package.name | tr -d '"')
PUB := $(shell cargo show --json $(NAME) | jq ".versions[0].num" | tr -d '"' )
	

version:

publish-dry-run:
	echo $(PUB)
	ifneq( $(VERSION), $(PUB) )
		echo "PUBLISH!"
		#cargo publish --dry-run --allow-dirty
	endif
 
 
pub:
	@echo $(NAME)
	@echo $(PUB)
	@echo $(VERSION)

	if [ "$(VERSION)" == "$(PUB)" ]; then echo "Equal"; 
	else echo "NOT EQUAL";
	fi


	#cargo publish
