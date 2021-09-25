#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$CACHE/tmp/starlane-cli/cache

starlane publish ./localhost-config "localhost:config:1.0.0"

echo "publish the localhost-config."
