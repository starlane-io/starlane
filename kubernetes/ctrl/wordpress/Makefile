all:
	tar czvf docker/wordpress.tar.gz chart
	cd docker && docker build . --tag starlane/wordpress-ctrl:latest
	docker push starlane/wordpress-ctrl:latest


