#!/bin/bash

IMG="starlane/starlane-operator:latest"

make docker-build docker-push IMG=$IMG

cd config/manager 

kustomize edit set image controller=${IMG}

cd ../..

kustomize build config/default > ../../k8s/starlane-operator.yml


