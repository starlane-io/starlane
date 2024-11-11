#!/bin/bash

set -x
set -e


echo off

src="./starlane/src/$1"
dst="./starlane-hyperspace/src/hyperspace"


mv ${src} $dst

exit 1

replace="s#crate::$1#starlane_hyperspace::$1#g"

echo $replace

find starlane -name "*.rs" -exec sed -i "" -e $replace {} +




