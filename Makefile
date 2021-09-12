docker:
	docker build . --tag starlane/starlane:latest
	docker push starlane/starlane:latest

