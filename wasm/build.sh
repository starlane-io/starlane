#!/bin/bash

set -e

cd hello_wasip2 && ./build.sh && cd ..

cargo run --package starlane pack publish package
