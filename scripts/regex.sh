#!/bin/bash

SCRIPTS=`dirname "$0"`

ROOT=`realpath "$SCRIPTS/.."`

cd $ROOT


#find .. -type f -exec sed -i '$1' { } +

find . -name "*.rs" -exec sed -i '' -e '$1' {} ';'

