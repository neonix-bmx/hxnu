#!/usr/bin/env bash
set -euo pipefail

rustup toolchain install nightly --profile minimal --component rust-src --component llvm-tools-preview
rustup target add x86_64-unknown-none --toolchain nightly
