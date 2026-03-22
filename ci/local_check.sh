#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
mkdir -p artifacts/readme-gtests artifacts/negative-gtests
greentic-integration-tester run --gtest tests/gtests/README --artifacts-dir artifacts/readme-gtests --errors
greentic-integration-tester run --gtest tests/gtests/negative/smoke --artifacts-dir artifacts/negative-gtests --errors
