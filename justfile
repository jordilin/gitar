export RUSTC_WRAPPER := "sccache"

alias t:= cargo-test
alias ta:= test-all
alias tc:= test-coverage
alias b:= build
alias br:= build-release
alias mut:= mutation
alias c:= contracts

build-release:
    cargo build --release

build:
    cargo build

cargo-test:
    cargo nextest run

cargo-test-nocapture:
    cargo test -- --nocapture

contracts:
    bash contracts/verify_git.sh
    python3 contracts/verify_gitlab.py
    python3 contracts/verify_github.py

test-all:
    cargo test
    bash contracts/verify_git.sh
    python3 contracts/verify_gitlab.py

test-watch:
    cargo watch --clear --exec test

test-coverage:
    cargo tarpaulin --frozen --exclude-files=src/main.rs --out Html

test-coverage-llvm:
    cargo llvm-cov --html

mutation:
    cargo mutants

audit:
    mkdir -p .cargo-audit-db/db
    cargo audit -D warnings -d .cargo-audit-db/db

doc:
    cargo doc --no-deps --open
