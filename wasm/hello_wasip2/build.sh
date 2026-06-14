#!/bin/bash
cargo build  --target wasm32-wasip2 --release

mkdir ../../target/wasm || true

cp ../../target/wasm32-wasip2/release/hello_wasip2.wasm ../package

