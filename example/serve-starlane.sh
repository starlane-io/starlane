#!/bin/bash

export STARLANE_DATA=$HOME/tmp/starlane/data
export STARLANE_CACHE=$CACHE/tmp/starlane/cache

rm -rf $STARLANE_DATA
rm -rf $STARLANE_CACHE

starlane serve


