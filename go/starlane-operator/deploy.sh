#!/bin/bash


make deploy IMG="starlane/starlane-operator:snapshot"

kubectl delete -n starlane-operator-system pod --all
