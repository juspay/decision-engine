<div align="center">

# ⚡ Decision Engine

**An open-source, high-performance payment routing engine — written in Rust.**

*Route every transaction to the gateway most likely to succeed, in real time, with zero vendor lock-in.*

[![CI](https://img.shields.io/github/actions/workflow/status/juspay/decision-engine/ci-push.yml?branch=main&label=CI&style=flat-square)](https://github.com/juspay/decision-engine/actions/workflows/ci-push.yml)
[![Release](https://img.shields.io/github/v/release/juspay/decision-engine?include_prereleases&style=flat-square&label=release&color=brightgreen)](https://github.com/juspay/decision-engine/releases)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-2496ED?style=flat-square&logo=docker&logoColor=white)](https://github.com/juspay/decision-engine/pkgs/container/decision-engine)
[![Slack](https://img.shields.io/badge/slack-join%20chat-4A154B?style=flat-square&logo=slack&logoColor=white)](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw)

**[Quick Start](#-quick-start)** · **[Documentation](#-documentation)** · **[Architecture](#-architecture)** · **[Routing Strategies](#-routing-strategies)** · **[Contributing](#-contributing)**

</div>

---

## 🎯 What is Decision Engine?

Decision Engine is a standalone payment routing service that, for each transaction, picks the gateway most likely to authorise successfully. It combines **rule-based routing** (merchant-authored policies) with **success-rate–based dynamic routing** (learned from live traffic), and it is designed to drop into any orchestrator — including [Hyperswitch](https://github.com/juspay/hyperswitch), commercial stacks, or a home-grown payments service.

```text
    ┌──────────────┐       ┌──────────────────┐       ┌──────────────┐
    │   Payment    │──────▶│  Decision Engine │──────▶│  Chosen      │
    │   Request    │       │  rules + SR data │       │  Gateway     │
    └──────────────┘       └──────────────────┘       └──────────────┘
                                    │
                                    ▼
                           success / failure feedback
                           (used to update SR scores)
```

### Why teams choose Decision Engine

| Problem you probably have | How Decision Engine solves it |
|---|---|
| Gateway outages silently tank your auth rate | **Elimination routing** deprioritises unhealthy gateways in real time |
| Static routing tables leave money on the table | **SR-based dynamic ordering** learns from live success rates |
| Business rules live in code, not config | **Priority Logic V2** — a versioned, API-driven rule engine |
| Vendor lock-in with existing orchestrators | **Drop-in HTTP service**, DB-agnostic (MySQL or PostgreSQL) |
| Compliance scope creep | **No card data ever touches the engine** — integrate behind your vault |

---

## ✨ Features

### 🧠 Intelligent routing
- **Eligibility filtering** — rule out gateways that can't process the transaction
- **Priority Logic V2** — author rules over any payment field (`amount`, `payment_method`, `card_network`, `billing_country`, …) with `AND` / `OR` / nested logic, number ranges, enum arrays, volume splits, and per-rule metadata
- **SR-based dynamic ordering** — gateways are reordered on live success-rate scores at a configurable dimension (merchant, payment method, BIN, …)
- **Elimination routing** — gateways dropping below a configurable SR threshold are temporarily removed from selection
- **Hedging / exploration** — a tunable percentage of traffic is sent off-policy to keep SR estimates honest
- **Debit network routing (US)** — pick the lowest-cost debit network on co-badged cards
- **Scheduled outage awareness** — exclude gateways during known maintenance windows

### 🛠 Built for production
- **Rust** — memory-safe, no GC pauses, suitable for the hot path of a payments stack
- **Pluggable storage** — MySQL or PostgreSQL; Redis for ephemeral SR state
- **First-class ops** — Prometheus metrics, structured logs, health endpoint, Grafana dashboards
- **Docker & Helm** — pinned GHCR images (`v1.4`) and a shipped Helm chart
- **Versioned Compose profiles** — `postgres-ghcr`, `mysql-ghcr`, `dashboard-*`, `monitoring`, and local-build equivalents (see [local-setup.md](docs/local-setup.md))

---

## 🚀 Quick Start

The fastest path is Docker Compose against the pinned GHCR image.

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine

# PostgreSQL stack (API + Postgres + Redis + migrations)
docker compose --profile postgres-ghcr up -d
```

Verify:

```bash
curl http://localhost:8080/health
# → {"message":"Health is good"}
```

Make your first routing decision:

```bash
curl -s http://localhost:8080/decide-gateway \
  -H 'Content-Type: application/json' \
  -d '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["GatewayA", "GatewayB", "GatewayC"],
    "rankingAlgorithm": "SR_BASED_ROUTING",
    "eliminationEnabled": true,
    "paymentInfo": {
      "paymentId": "PAY12359",
      "amount": 100.50,
      "currency": "USD",
      "customerId": "CUST12345",
      "paymentType": "ORDER_PAYMENT",
      "paymentMethodType": "UPI",
      "paymentMethod": "UPI_PAY"
    }
  }'
```

Need MySQL, a local source build, or Helm? See the [**Local Setup Guide**](docs/local-setup.md) — it is the canonical reference and covers every supported path.

### Build from source

```bash
# Prerequisites: Rust 1.85+, PostgreSQL (or MySQL), Redis

# PostgreSQL build
cargo build --release --no-default-features --features middleware,kms-aws,postgres
just migrate-pg
RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres

# MySQL build
cargo build --release --features release
RUSTFLAGS="-Awarnings" cargo run --features release
```

> A ready-to-import [Postman collection](decision-engine.postman_collection.json) is included at the repo root.

---

## 📖 Documentation

| Resource | What it covers |
|---|---|
| [**Local Setup Guide**](docs/local-setup.md) | Canonical setup — CLI, Docker, Compose profiles, Helm, troubleshooting |
| [PostgreSQL Quickstart](docs/setup-guide-postgres.md) | Postgres-focused commands |
| [MySQL Quickstart](docs/setup-guide-mysql.md) | MySQL-focused commands |
| [**API Reference**](docs/api-reference.md) | Endpoint map + full request/response examples (`decide-gateway`, `update-gateway-score`, rule CRUD, Priority Logic V2) |
| [Configuration Guide](docs/configuration.md) | Every config section, environment overrides, and examples |
| [OpenAPI Spec](docs/openapi.json) | Machine-readable API definition (importable into Postman / Swagger) |
| [Helm Chart](helm-charts/) | Kubernetes deployment |
| [Deep-dive blog](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine) | Background on the routing logic |

Run the full docs portal locally with any `dashboard-*` Compose profile — it is served at `http://localhost:8081/introduction`.

---

## 🏗 Architecture

<div align="center">
  <img src="https://cdn.sanity.io/images/9sed75bn/production/fd872ae5b086e7a60011ad9d4d5c7988e1084d03-1999x1167.png" alt="Decision Engine architecture" width="80%"/>
</div>

### Integration pattern

<div align="center">
  <img src="https://github.com/user-attachments/assets/272ad222-8a91-4bb2-aa3a-e1fc9c28e3da" alt="Integration pattern" width="70%"/>
</div>

| Step | Flow | Component | Action |
|:---:|:---:|---|---|
| 1 | → | Your app | Initiates a payment |
| 2 | → | Orchestrator | Calls Decision Engine with eligible gateways + context |
| 3 | → | Decision Engine | Applies rules, SR scores, and health data to pick a gateway |
| 4 | → | Vault | Detokenises card data (engine never sees PAN) |
| 5 | → | Gateway | Processes the authorisation |
| 6 | ← | Gateway | Returns success / failure |
| 7 | → | Decision Engine | Orchestrator calls `/update-gateway-score` so SR stats stay current |
| 8 | ← | Your app | Receives the final result |

**Design properties**
- **Zero PCI scope** — all card data flows via your vault; the engine sees tokens only.
- **Stateless hot path** — routing decisions read from Redis / DB; horizontal scale is trivial.
- **Orchestrator-agnostic** — a thin HTTP contract with JSON payloads; no SDK lock-in.

---

## 🧭 Routing Strategies

Decision Engine exposes multiple ranking algorithms via the `rankingAlgorithm` field on `/decide-gateway`:

| Algorithm | When to use |
|---|---|
| `SR_BASED_ROUTING` | Default. Pick the gateway with the best live success-rate score at the configured dimension. |
| `NTW_BASED_ROUTING` | US debit routing — select the lowest-cost network on co-badged cards. |
| `PL_BASED_ROUTING` | Evaluate a merchant-authored [Priority Logic V2](docs/api-reference1.md#priority-logic-v2) rule. |

Responses include a `routing_approach` field that tells you exactly *why* a gateway was chosen — `SR_SELECTION_V3_ROUTING`, `SR_V3_DOWNTIME_ROUTING`, `SR_V3_ALL_DOWNTIME_ROUTING`, `SR_V3_HEDGING`, `SR_V3_DOWNTIME_HEDGING`, or `SR_V3_ALL_DOWNTIME_HEDGING`. The semantics of each value are documented in the [API reference](docs/api-reference1.md#routing-approach).

Feedback loop: call `/update-gateway-score` with the transaction outcome (`SUCCESS` / `FAILURE`) so the engine can update its SR estimates. This is the mechanism that makes dynamic routing learn.

---

## 🗺 Roadmap

| Status | Item |
|:---:|---|
| ✅ | Rule-based routing (Priority Logic V2) |
| ✅ | SR-based dynamic ordering |
| ✅ | Elimination routing with downtime detection |
| ✅ | MySQL + PostgreSQL support |
| ✅ | Helm chart & GHCR images |
| 🔄 | Enhanced SR prediction models |
| 🔄 | Admin dashboard for rule management |
| 📋 | Per-tenant routing analytics |
| 📋 | GraphQL API surface |

Legend: ✅ shipped · 🔄 in progress · 📋 planned.

---

## 🤝 Contributing

Contributions are welcome — bug reports, documentation fixes, new features, and rule-engine extensions.

```bash
# 1. Fork and clone
git clone https://github.com/YOUR_USERNAME/decision-engine.git
cd decision-engine

# 2. Create a branch
git checkout -b feat/your-feature

# 3. Run the checks locally
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test

# 4. Open a pull request
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow, code style, and commit message conventions. New contributors: look for [`good first issue`](https://github.com/juspay/decision-engine/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).

---

## 💬 Community & Support

| Channel | For |
|---|---|
| [Slack](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw) | Real-time questions, discussions |
| [GitHub Discussions](https://github.com/juspay/decision-engine/discussions) | Design questions, ideas, RFCs |
| [GitHub Issues](https://github.com/juspay/decision-engine/issues) | Bugs and feature requests |

For security-sensitive reports, please **do not** file a public issue — email the maintainers at `hyperswitch@juspay.in`.

---

## 📜 License

Decision Engine is licensed under the [GNU Affero General Public License v3.0](LICENSE). Commercial and embedded-use questions: `hyperswitch@juspay.in`.

---

<div align="center">

Built by [Juspay](https://juspay.io) and the Hyperswitch community.

**[⬆ Back to top](#-decision-engine)**

</div>
