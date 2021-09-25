#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$CACHE/tmp/starlane-cli/cache

starlane create "localhost:my-app<App>" "localhost:app-config:1.0.0:/app/my-app.yaml"

echo "go to http://localhost:8080/app/ to see a page served by a Mechtron"

