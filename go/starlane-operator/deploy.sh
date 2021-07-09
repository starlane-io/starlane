#!/bin/bash

make deploy IMG="starlane/starlane-operator:latest"

kubectl delete -n starlane-operator-system pod --all
