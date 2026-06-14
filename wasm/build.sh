#!/bin/bash

cd hello_wasip2 && ./build.sh && cd ..


cd ..
cargo run --package starlane pack publish