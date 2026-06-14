#!/bin/bash
cargo build  --target wasm32-wasip1 --release

mkdir ../../target/wasm || true

cp ../../target/wasm32-wasip1/release/hello.wasm ../../target/wasm

