#!/bin/sh

# RUSTSEC-2024-0436 paste: rust_icu
# RUSTSEC-2026-0253 lru: tantivy
cargo audit --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2026-0253 -D warnings && \
  cargo clippy && cargo build --release && \
  cargo release patch --no-publish --execute
