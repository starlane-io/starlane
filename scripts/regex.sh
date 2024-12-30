#!/bin/bash

SCRIPTS=`dirname "$0"`

ROOT=`realpath "$SCRIPTS/.."`

cd $ROOT

echo "find .. -type f -exec sed -i '$1' { } +"

find .. -type f -exec sed -i '$1' { } +

