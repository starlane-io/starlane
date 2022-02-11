SUBDIRS = wasm/my-app 

.PHONY: subdirs $(SUBDIRS)

subdirs: $(SUBDIRS)

$(SUBDIRS):
	$(MAKE) -C $@ all

all: subdirs
	cd bundle && zip -r ../bundle.zip . 

