#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$CACHE/tmp/starlane-cli/cache

starlane cp websites/simple-site1/index.html "localhost:my-files:/index.html"

echo "you have uploaded one file 'index.html' this is going to be our whole website later..."


