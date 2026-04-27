<div align="center">

<img src="https://img.shields.io/badge/Rust-1.85%2B-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
<img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=for-the-badge" alt="License"/>
<img src="https://img.shields.io/badge/Docker-Ready-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker"/>
<img src="https://img.shields.io/github/v/release/juspay/decision-engine?include_prereleases&style=for-the-badge&label=Release&color=brightgreen" alt="Release"/>
<img src="https://img.shields.io/badge/Slack-Join%20Chat-4A154B?style=for-the-badge&logo=slack&logoColor=white" alt="Slack"/>

<br/><br/>

# Decision Engine

### Routing control plane for payment decisions

**Open-Source • Rust • Rule-Based • Success-Rate Based**

Configure routing rules, run gateway decisions, and inspect routing outcomes from APIs or the dashboard.

---

**[Quick Start](#quick-start)** •
**[Documentation](#documentation)** •
**[Architecture](#architecture)** •
**[Contributing](#contributing)**

</div>

---

## What is Decision Engine?

Decision Engine is a Rust service that sits between your orchestrator and your list of payment gateways. When a payment comes in, it picks the best available gateway based on rules you configure — priority ordering, success-rate scoring, volume splits, or debit-network gates — and returns the decision over HTTP.

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Payment    │────▶│  Decision Engine │────▶│  Best       │
│  Request    │     │  (Fast routing)  │     │  Gateway    │
└─────────────┘     └──────────────────┘     └─────────────┘
```

It runs as a standalone service — no vendor lock-in, no mandatory orchestrator. Your existing stack calls it over HTTP before dispatching to a gateway, and over time it improves decisions using outcome feedback you push back via the score update API.

What it ships today:

- **Rule-based routing** — define priority rules per merchant using Euclid, Juspay's open rule engine
- **Success-rate ordering** — gateways ranked dynamically from transaction outcome feedback
- **Volume splits** — distribute traffic across gateways by percentage
- **Debit routing gates** — per-merchant toggle for debit-network routing
- **Downtime detection** — auto-excludes gateways that are failing
- **Analytics** — ClickHouse-backed tables for routing outcomes and decision audit
- **Dashboard** — React UI for configuring rules and inspecting decisions
- **Multi-DB** — MySQL and PostgreSQL support
- **Team management** — invite and manage members per merchant account

---

## Quick Start

### Docker (Recommended)

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
docker compose --profile postgres-ghcr up -d
```

API is ready at `http://localhost:8080`. That's it.

For API + dashboard + docs together:

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

Open:

- API: `http://localhost:8080`
- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`
- API reference: `http://localhost:8081/api-overview`

For deployed docs or dashboard environments, use the same paths under your deployed host, e.g. `https://<docs-host>/api-overview`.

### From Source

Prerequisites: Rust 1.85+, MySQL or PostgreSQL, Redis, [`just`](https://just.systems)

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine

cp config.example.toml config/development.toml
# Edit config with your DB, Redis, and ClickHouse connection details
```

**MySQL** (default features):
```bash
cargo build --release --features release
diesel migration run   # set DATABASE_URL=mysql://user:pass@host/dbname first
RUSTFLAGS="-Awarnings" cargo run --features release
```

**PostgreSQL**:
```bash
cargo build --release --no-default-features --features middleware,kms-aws,postgres
just migrate-pg        # sets DATABASE_URL from env or justfile defaults
RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres
```

For the full local dev environment (API + dashboard on port 5173 + docs), run:

```bash
./oneclick.sh
```

This brings up Postgres, Redis, ClickHouse, and Kafka via Docker Compose, runs migrations, and starts the API server and dashboard locally. See [Local Setup Guide](docs/local-setup.md) for full details and options like `ONECLICK_KEEP_INFRA=1`.

### Verify

```bash
curl http://localhost:8080/health
# → {"message":"Health is good"}
```

---

## Documentation

| Resource | Description |
|----------|-------------|
| [Local Setup Guide](docs/local-setup.md) | CLI, Docker, Compose profiles, and Helm |
| [MySQL Setup Guide](docs/setup-guide-mysql.md) | MySQL-specific walkthrough |
| [PostgreSQL Setup Guide](docs/setup-guide-postgres.md) | PostgreSQL-specific walkthrough |
| [API Reference (Swagger)](https://juspay.github.io/decision-engine/api-docs/) | Interactive Swagger UI — browse and try every endpoint against the OpenAPI spec |
| [API Overview](docs/api-overview.md) | Entrypoint to API examples, OpenAPI schema, and route access rules |
| [Configuration Guide](docs/configuration.md) | All config options explained |
| [Deep Dive Blog](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine) | How the routing logic works |

---

## Architecture

### High-Level Flow

<div align="center">
  <img src="https://cdn.sanity.io/images/9sed75bn/production/fd872ae5b086e7a60011ad9d4d5c7988e1084d03-1999x1167.png" alt="Decision Engine Architecture" width="80%"/>
</div>

### Integration Pattern

<div align="center">
  <img src="https://github.com/user-attachments/assets/272ad222-8a91-4bb2-aa3a-e1fc9c28e3da" alt="Integration Pattern" width="70%"/>
</div>

Decision Engine fits into an existing payment stack without replacing your orchestrator. The orchestrator calls Decision Engine to get a gateway recommendation, then dispatches to that gateway. Card data stays in your vault — Decision Engine never touches it.

| Step | Direction | Component | Action |
|:----:|:---------:|-----------|--------|
| 1 | → | Your App | Initiates payment request |
| 2 | → | Orchestrator | Forwards to Decision Engine |
| 3 | → | Decision Engine | Selects optimal gateway |
| 4 | → | Vault | Returns card token (PCI-safe) |
| 5 | → | Gateway | Processes payment |
| 6 | ← | Gateway | Returns result |
| 7 | ← | Orchestrator | Routes response back |
| 8 | ← | Your App | Receives final result |

---

## Roadmap

| Status | Feature | Description |
|:------:|---------|-------------|
| ✅ | Rule-based routing | Merchant-defined priority rules via Euclid |
| ✅ | Dynamic ordering | Success-rate based gateway selection |
| ✅ | Downtime detection | Automatic health monitoring |
| ✅ | Multi-database | MySQL & PostgreSQL support |
| ✅ | Dashboard | Visual rule management and decision audit UI |
| ✅ | Team management | Invite members to merchant accounts |
| 🔄 | Enhanced routing models | Better success rate prediction |
| 📋 | Multi-tenant analytics | Per-tenant routing insights |
| 📋 | GraphQL API | Alternative query interface |

---

## Contributing

Contributions are welcome — bug reports, feature requests, docs, or code.

```bash
# Fork & clone
git clone https://github.com/YOUR_USERNAME/decision-engine.git

# Create a branch
git checkout -b feature/your-feature

# Make changes and test
cargo test

# Submit a PR
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, and check [good first issues](https://github.com/juspay/decision-engine/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) if you're new to the codebase.

---

## Community

| Platform | Purpose |
|----------|---------|
| [![Slack](https://img.shields.io/badge/Slack-Join_Chat-4A154B?logo=slack)](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw) | Real-time help and discussions |
| [GitHub Discussions](https://github.com/juspay/decision-engine/discussions) | Feature requests and ideas |
| [GitHub Issues](https://github.com/juspay/decision-engine/issues) | Bug reports |

---

## License

Licensed under [GNU AGPL v3.0](LICENSE).

---

<div align="center">

Built by [Juspay](https://juspay.io)

**[Back to Top](#decision-engine)**

</div>
