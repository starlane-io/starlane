
PACKAGES = cosmic/cosmic-nom cosmic/cosmic-macros-primitive
VERSION := $(shell cat ../VERSION)
all: tmp := $(shell mktemp Cargo.XXXXXXXX)

init:
	cargo install cargo-edit


chver:
	toml set "${@}/Cargo.toml" package.version ${VERSION} > ${tmp}
	mv $(tmp) ${@}/Cargo.toml


all:
	for P in $(PACKAGES); do \
	  echo $$P; \
		toml set "$$P/Cargo.toml" package.version ${VERSION} > ${tmp}; \
		mv $(tmp) $$P/Cargo.toml; \
	done






