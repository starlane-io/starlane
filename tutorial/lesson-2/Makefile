SUBDIRS = wasm/my-app 

.PHONY: subdirs $(SUBDIRS)

subdirs: $(SUBDIRS)

$(SUBDIRS):
	$(MAKE) -C $@ all

zip:
	cd bundle && zip -r ../bundle.zip . 

all: subdirs zip

