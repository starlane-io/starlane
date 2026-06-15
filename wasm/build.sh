#!/bin/bash

set -e

cd hello_wasip2 && ./build.sh && cd ..

starlane pack publish package
