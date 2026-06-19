.PHONY : clean

build:
	$(MAKE) -C hello_wasip2 build
	$(MAKE) -C email_validator_util build
	starlane pack publish package

clean :
	find package -name "*.wasm" -type f -delete

