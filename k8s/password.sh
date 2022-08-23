#!/bin/bash

kubectl get secrets my-starlane --template={{.data.password}} | base64 -D | pbcopy
