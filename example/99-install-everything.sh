#!/bin/bash

set -e

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$CACHE/tmp/starlane-cli/cache

starlane create "localhost<Space>"
starlane create "localhost:my-files<FileSystem>"
starlane cp websites/simple-site1/index.html "localhost:my-files:/index.html"
starlane publish ./localhost-config "localhost:config:1.0.0"
starlane set "localhost::config=localhost:config:1.0.0:/routes.conf"

cd app && make all 
cd ..

starlane publish app/bundle "localhost:app-config:1.0.0"
starlane create "localhost:my-app<App>" "localhost:app-config:1.0.0:/app/my-app.yaml"


