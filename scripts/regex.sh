#!/bin/bash

SCRIPTS=`dirname "$0"`

ROOT=`realpath "$SCRIPTS/.."`

cd $ROOT

find . -name "*.rs" -exec sed -i '' -e '$1' {} ';'

