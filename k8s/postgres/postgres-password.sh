#!/bin/bash

echo "This probably only works on a mac"

kubectl get secrets my-postgres --template={{.data.password}} | base64 -D | pbcopy
