#!/bin/bash
cargo build  --target wasm32-wasip2 --release

mkdir ../../target/wasm || true

cp ../../target/wasm32-wasip2/release/email_validator_util.wasm ../package

