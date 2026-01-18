#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-mutants >/dev/null 2>&1; then
  echo "cargo-mutants not found. Install with: cargo install cargo-mutants --locked" >&2
  exit 1
fi

cargo mutants "$@"
