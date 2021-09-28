#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane/data
export STARLANE_CACHE=$HOME/tmp/starlane/cache

rm -rf $STARLANE_DATA
rm -rf $STARLANE_CACHE

echo "Check out http://localhost:8080/ You should get the STARLANE welcome page."
echo "You will have to run additional steps in a new terminal.  To exit use CTRL-C"


starlane serve


