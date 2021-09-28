#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane-cli/data
export STARLANE_CACHE=$HOME/tmp/starlane-cli/cache

starlane create "localhost:my-files<FileSystem>"

echo "You have created a local FileSystem called 'my-files'"


