#!/bin/sh
RUSTFLAGS="-Clink-args=--import-memory" cargo +nightly build -v --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/testbackend.wasm ..

