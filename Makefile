docker:
	docker build . --tag starlane/starlane:latest
	docker push starlane/starlane:latest

operator:
	cd go/starlane-operator && ./build.sh && ./deploy.sh

starlane: operator
	kubectl apply -f kubernetes/starlane/starlane.yaml
	kubectl apply -f kubernetes/mysql/conf/starlane-provisioner.yaml

mysql: 
	cd kubernetes/mysql && helm install mysql chart

kube: mysql starlane

mysql-provisioner:
	cd kubernetes/mysql/provisioner/ && docker build . --tag starlane/mysql-provisioner:latest
	docker push starlane/mysql-provisioner:latest

delete:
	cd kubernetes/mysql && helm delete mysql chart
	
