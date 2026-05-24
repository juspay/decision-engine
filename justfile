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
                | .packages[] | select( IN(.id; $package_ids[]) ) | select(.name != "gsm") | .features | keys[]
                | select( . != "mysql" and . != "postgres" and . != "default" and . != "release")
            ]
            | unique
            | join(",")
    ')"

    set -x
    cargo clippy {{ check_flags }} --features "${FEATURES},mysql,default,release"  {{ FLAGS }}
    cargo clippy --no-default-features --features "${FEATURES},postgres"  {{ FLAGS }}
    set +x
alias c := check

check *FLAGS:
    #! /usr/bin/env bash
    set -euo pipefail

    FEATURES="$(cargo metadata --all-features --format-version 1 --no-deps | \
        jq -r '
            [ ( .workspace_members | sort ) as $package_ids
                | .packages[] | select( IN(.id; $package_ids[]) ) | select(.name != "gsm") | .features | keys[]
                | select( . != "mysql" and . != "postgres" and . != "default" and . != "release")
            ]
            | unique
            | join(",")
    ')"

    set -x
    cargo check {{ check_flags }} --features "${FEATURES},mysql,default,release"  {{ FLAGS }}
    cargo check --no-default-features --features "${FEATURES},postgres"  {{ FLAGS }}
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

# Run all Cypress E2E tests headlessly (replicates CI behaviour — single browser, sequential)
cypress:
    npx cypress run --spec "cypress/e2e/ui/**/*.cy.js,cypress/e2e/api/**/*.cy.js" --headless
alias cy := cypress

# Run a single Cypress spec headlessly  e.g.: just cypress-spec cypress/e2e/ui/euclid-rules-builder.cy.js
cypress-spec spec:
    npx cypress run --spec "{{ spec }}" --headless

# Run all Cypress E2E tests across 3 balanced parallel workers.
#
# Split is based on measured spec durations (see worker comments).
# Euclid specs are distributed across all three workers so no single
# worker is left idle while another finishes the euclid suite alone.
#
#   Worker 1 ~78s — heavy euclid:  builder(1:07) + enum-operators(0:11)
#   Worker 2 ~75s — medium euclid: e2e(0:36)     + lifecycle(0:39)
#   Worker 3 ~86s — fast euclid + general UI + API:
#                   nested-branches(0:09) + volume-split-priority(0:14) +
#                   volume-split(0:14) + all general UI(0:35) + all API(0:14)
cypress-parallel:
    #!/usr/bin/env bash
    set -uo pipefail

    npx cypress run --headless \
      --spec "cypress/e2e/ui/euclid-rules-builder.cy.js,cypress/e2e/ui/euclid-rules-enum-operators.cy.js" \
      2>&1 | sed 's/^/[worker-1] /' &
    pid1=$!

    npx cypress run --headless \
      --spec "cypress/e2e/ui/euclid-rules-e2e.cy.js,cypress/e2e/ui/euclid-rules-lifecycle.cy.js" \
      2>&1 | sed 's/^/[worker-2] /' &
    pid2=$!

    npx cypress run --headless \
      --spec "cypress/e2e/ui/euclid-rules-nested-branches.cy.js,cypress/e2e/ui/euclid-rules-volume-split-priority.cy.js,cypress/e2e/ui/euclid-rules-volume-split.cy.js,cypress/e2e/ui/analytics-page.cy.js,cypress/e2e/ui/auth-page.cy.js,cypress/e2e/ui/dashboard-overview.cy.js,cypress/e2e/ui/debit-routing-page.cy.js,cypress/e2e/ui/decision-explorer.cy.js,cypress/e2e/ui/payment-audit.cy.js,cypress/e2e/ui/volume-split-page.cy.js,cypress/e2e/api/**/*.cy.js" \
      2>&1 | sed 's/^/[worker-3] /' &
    pid3=$!

    failed=0
    wait "$pid1" || failed=1
    wait "$pid2" || failed=1
    wait "$pid3" || failed=1
    exit "$failed"
alias cyp := cypress-parallel

# Run pre-commit checks
precommit: fmt clippy

ci_hack:
    scripts/ci-checks.sh

# Use the env variables if present, or fallback to default values

db_user := env_var_or_default('DB_USER', 'db_user')
db_password := env_var_or_default('DB_PASSWORD', 'db_pass')
db_host := env_var_or_default('DB_HOST', 'localhost')
db_port := env_var_or_default('DB_PORT', '5432')
db_name := env_var_or_default('DB_NAME', 'decision_engine_db')
default_db_url := 'postgresql://' + db_user + ':' + db_password + '@' + db_host + ':' + db_port + '/' + db_name
database_url := env_var_or_default('DATABASE_URL', default_db_url)
default_migration_params := ''

pg_migration_dir := source_directory() / 'migrations_pg'
pg_config_file_dir := source_directory() / 'diesel_pg.toml'

default_operation := 'run'

[private]
run_migration operation=default_operation migration_dir=pg_migration_dir config_file_dir=pg_config_file_dir url=database_url *other_params=default_migration_params:
    diesel migration \
        --database-url '{{ url }}' \
        {{ operation }} \
        --migration-dir '{{ migration_dir }}' \
        --config-file '{{ config_file_dir }}' \
        {{ other_params }}

# Run database migrations for postgres
migrate-pg operation=default_operation *args='': (run_migration operation pg_migration_dir pg_config_file_dir database_url args)

# ── Load testing ─────────────────────────────────────────────────────────────
#
# Prerequisites: brew install k6
#
# All load-test scripts live in scripts/load-test/.
# benchmark.sh runs stepped VU levels, captures CPU/thread/Redis stats (local),
# and prints a before/after comparison table (sandbox).
#
# VU (Virtual User): a simulated concurrent connection that fires requests in a
# tight loop (request → wait for response → 100ms sleep → repeat).
# Measured sweet spots:
#   local   (2 vCPU / INFO logs) → 20 VUs  (~140 req/s, p95 ≈ 40ms)
#   sandbox (500m req / 1000m limit) → 12 VUs (~66 req/s, p95 ≈ 78ms)

# Stepped benchmark against local container (auto-provisions user+merchant)
# Captures CPU%, thread count, Redis ops alongside the k6 run.
load-test-local vus='5,20,50' duration='30s':
    @bash scripts/load-test/benchmark.sh --vus {{ vus }} --duration {{ duration }}
alias ltl := load-test-local

# Stepped benchmark against sandbox with before/after comparison table.
# Requires TOKEN and MERCHANT_ID:
#   export TOKEN=<your_jwt>  export MERCHANT_ID=<your_merchant_id>
load-test-sandbox vus='5,12,20' duration='30s':
    #!/usr/bin/env bash
    set -euo pipefail
    : "${TOKEN:?TOKEN must be set (export TOKEN=<your_jwt>)}"
    : "${MERCHANT_ID:?MERCHANT_ID must be set (export MERCHANT_ID=<your_merchant_id>)}"
    bash scripts/load-test/benchmark.sh \
        --env sandbox \
        --vus {{ vus }} \
        --duration {{ duration }} \
        -t "$TOKEN" \
        -m "$MERCHANT_ID"
alias lts := load-test-sandbox

# HTML report run — single VU level, generates a self-contained HTML file.
# Opens the report automatically when done.
load-test-report vus='20' duration='30s':
    #!/usr/bin/env bash
    set -euo pipefail
    k6 run scripts/load-test/load_test_report.js \
        -e ENV=local \
        -e VUS={{ vus }} \
        -e DURATION={{ duration }}
    REPORT="scripts/load-test/load_test_report_local_{{ vus }}vu.html"
    [ -f "$REPORT" ] && open "$REPORT" 2>/dev/null || true
alias ltr := load-test-report

# Generate a fresh local auth token (expires in 24h) — useful for manual curl calls
load-test-token:
    @bash scripts/load-test/gen_local_token.sh

# Drop database if exists and then create a new 'hyperswitch_db' Database
resurrect database_name=db_name:
    psql -U postgres -c 'DROP DATABASE IF EXISTS  {{ database_name }}';
    psql -U postgres -c 'CREATE DATABASE {{ database_name }}';
