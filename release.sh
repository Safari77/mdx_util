#!/bin/sh

cargo audit -D warnings && cargo clippy && cargo build --release && \
  cargo release patch --no-publish --execute
