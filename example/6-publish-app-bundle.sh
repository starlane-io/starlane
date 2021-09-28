#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$HOME/tmp/starlane-cli/cache

cd app && make all
cd ..

starlane publish app/bundle "localhost:app-config:1.0.0"

echo 
echo 
echo 

echo "you have just build a Mechtron and published it to an artifact bundle"

