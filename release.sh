#!/bin/sh

# RUSTSEC-2024-0436 paste: rust_icu
cargo audit --ignore RUSTSEC-2024-0436 -D warnings && \
  cargo clippy && cargo build --release && \
  cargo release patch --no-publish --execute
