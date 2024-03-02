export RUSTC_WRAPPER := "sccache"

alias t:= cargo-test
alias ta:= test-all
alias tc:= test-coverage-vscode
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
    python3 contracts/verify_github.py

test-watch:
    cargo watch --clear --exec test

test-coverage-tarpaulin:
    cargo tarpaulin --frozen --exclude-files=src/main.rs --out Html

test-coverage-llvm:
    cargo llvm-cov --html

test-coverage-vscode:
    # Provides vscode coverage gutters through
    # https://marketplace.visualstudio.com/items?itemName=ryanluker.vscode-coverage-gutters
    # extension as explained in
    # https://github.com/taiki-e/cargo-llvm-cov?tab=readme-ov-file#display-coverage-in-vs-code
    cargo llvm-cov --lcov --output-path lcov.info

mutation:
    cargo mutants

audit:
    mkdir -p .cargo-audit-db/db
    cargo audit -D warnings -d .cargo-audit-db/db

doc:
    cargo doc --no-deps --open
