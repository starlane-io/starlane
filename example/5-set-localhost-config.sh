#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$CACHE/tmp/starlane-cli/cache

starlane set "localhost::config=localhost:config:1.0.0:/routes.conf"

echo "go to http://localhost:8080 notice that localhost is now configured"
echo "if you go to http://localhost:8080/files/ you will see index.html"

