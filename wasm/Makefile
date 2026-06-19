.PHONY : clean

all: build

build:
	$(MAKE) -C mock_registry build
	$(MAKE) -C hello_wasip2 build
	$(MAKE) -C email_validator_util build
	$(MAKE) -C email_validator_util_stateful build
	starlane pack publish package

clean :
	find package -name "*.wasm" -type f -delete

