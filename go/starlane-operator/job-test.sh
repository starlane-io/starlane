#!/bin/bash

export INDEX=$1

echo "`envsubst < config/samples/starlane_v1alpha1_starlaneresource.yaml`" | kubectl create -f -

