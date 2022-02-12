SUBDIRS = wasm/my-app 

.PHONY: subdirs $(SUBDIRS)

subdirs: $(SUBDIRS)

$(SUBDIRS):
	$(MAKE) -C $@ all

clean:
	rm bundle.zip

zip:
	cd bundle && zip -r ../bundle.zip . 

all: subdirs zip

