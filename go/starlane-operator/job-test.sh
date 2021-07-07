#!/bin/bash

export INDEX=$1
export STARLANE_UID=`kubectl get starlane starlane  -o jsonpath='{.metadata.uid}'`

echo "`envsubst < config/samples/starlane_v1alpha1_starlaneresource.yaml`" | kubectl create -f -

