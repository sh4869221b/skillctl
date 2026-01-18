default:
    @just --list

fmt:
    cargo fmt

clippy:
    cargo clippy -- -D warnings

test:
    cargo test

ci:
    just fmt
    just clippy
    just test

mutation:
    ./scripts/mutation.sh
