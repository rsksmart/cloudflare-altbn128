#!/bin/sh
cargo clean -p eth_pairings
RAYON_NUM_THREADS=4 cargo test --release -- --nocapture --ignored benchmark_existing_pairing_precompile

