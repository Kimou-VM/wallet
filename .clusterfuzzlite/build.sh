#!/bin/bash -eu

cd "$SRC/wallet/core"
python3 fuzz/generate_seeds.py
cargo fuzz build -O
release="fuzz/target/x86_64-unknown-linux-gnu/release"
for target in $(cargo fuzz list); do
    cp "$release/$target" "$OUT/"
    (cd "fuzz/seeds/$target" && zip -q -r "$OUT/${target}_seed_corpus.zip" .)
done
