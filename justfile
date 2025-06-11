# List available recipes
list:
    @just --list --justfile {{ source_file() }}


# Run formatter
fmt *FLAGS:
    cargo +nightly fmt {{ fmt_flags }} {{ FLAGS }}
fmt_flags := '--all'

check_flags := '--all-targets'
# Check compilation of Rust code and catch common mistakes
# Create a list of features
clippy *FLAGS:
    #! /usr/bin/env bash
    set -euo pipefail

    FEATURES="$(cargo metadata --all-features --format-version 1 --no-deps | \
        jq -r '
            [ ( .workspace_members | sort ) as $package_ids
            | .packages[] | select( IN(.id; $package_ids[]) ) | .features | keys[] | select( . != "mysql" and . != "postgres") ]
            | unique
            | join(",")
    ')"

    set -x
    cargo clippy {{ check_flags }} --features "${FEATURES},mysql"  {{ FLAGS }}
    cargo clippy {{ check_flags }} --features "${FEATURES},postgres"  {{ FLAGS }}
    set +x
alias c := check

check *FLAGS:
    #! /usr/bin/env bash
    set -euo pipefail

    FEATURES="$(cargo metadata --all-features --format-version 1 --no-deps | \
        jq -r '
            [ ( .workspace_members | sort ) as $package_ids
            | .packages[] | select( IN(.id; $package_ids[]) ) | .features | keys[] | select( . != "mysql" and . != "postgres") ]
            | unique
            | join(",")
    ')"

    set -x
    cargo check {{ check_flags }} --features "${FEATURES},mysql"  {{ FLAGS }}
    cargo check {{ check_flags }} --features "${FEATURES},postgres"  {{ FLAGS }}
    set +x
alias cl := clippy

# Build binaries
build *FLAGS:
    cargo build {{ FLAGS }}
alias b := build

# Build release (optimized) binaries
build-release *FLAGS:
    cargo build --release --features release {{ FLAGS }}
alias br := build-release

# Run server
run *FLAGS:
    cargo run {{ FLAGS }}
alias r := run

doc_flags := '--all-features --all-targets'
doc *FLAGS:
    cargo doc {{ doc_flags }} {{ FLAGS }}
alias d := doc

test *FLAGS:
    cargo test {{ FLAGS }}
alias t := test

# Run pre-commit checks
precommit: fmt clippy

ci_hack:
    scripts/ci-checks.sh
