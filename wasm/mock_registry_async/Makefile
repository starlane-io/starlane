

# Find all Rust source files
SOURCES := $(shell find src -name '*.rs') Cargo.toml

NAME := $(shell toml get Cargo.toml package.name)

.PHONY : clean build

clean :
	cargo clean

blah:
	@echo $(NAME)

all: build

build: ../package/$(NAME).wasm

# Build wasm if any source files are newer than the target
../../target/wasm32-wasip2/release/$(NAME).wasm: $(SOURCES)
	cargo build  --target wasm32-wasip2 --release

# Copy to package directory if source wasm is newer or target doesn't exist
../package/$(NAME).wasm: ../../target/wasm32-wasip2/release/$(NAME).wasm
	cp $< $@





