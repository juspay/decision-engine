<div align="center">

# Decision Engine

**Open-Source Payment Gateway Router**

*Intelligent routing for payments. Better success rates. Zero vendor lock-in.*

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker)](docker-compose.yaml)
[![GitHub release](https://img.shields.io/github/v/release/juspay/decision-engine?include_prereleases)](https://github.com/juspay/decision-engine/releases)
[![Slack](https://img.shields.io/badge/Slack-Join%20Chat-4A154B?logo=slack)](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw)

[Features](#-features) • [Quick Start](#-quick-start) • [Documentation](#-documentation) • [Architecture](#-architecture) • [Contributing](#-contributing)

</div>

---

## 🚀 What is Decision Engine?

Decision Engine is a **high-performance payment gateway router** that chooses the optimal payment gateway for each transaction in real-time. It uses pre-defined rules, success rate analysis, latency metrics, and ML-driven optimization to maximize transaction success rates.

**Why Decision Engine?**

| Problem | Solution |
|---------|----------|
| Payment failures due to gateway downtime | Real-time health monitoring with automatic failover |
| Suboptimal routing decisions | ML-driven dynamic ordering based on success rates |
| Vendor lock-in | Modular design works with any orchestrator and PCI-compliant vault |
| Complex rule management | Flexible rule-based routing with merchant-specific policies |

---

## ✨ Features

### 🎯 Core Capabilities

| Feature | Description |
|---------|-------------|
| ✅ **Eligibility Check** | Ensures only eligible gateways are used, reducing failures and improving success rates |
| 📌 **Rule-Based Ordering** | Routes transactions based on predefined merchant rules for predictable, obligation-driven processing |
| 🔄 **Dynamic Gateway Ordering** | Uses real-time success rates and ML optimization to route to the best-performing gateway |
| ⚠️ **Downtime Detection** | Monitors gateway health, dynamically reordering or pausing routing during downtime |

### 🛠 Technical Highlights

- **Built in Rust** — Blazing fast, memory-safe, and highly concurrent
- **Multi-database support** — MySQL and PostgreSQL
- **Redis caching** — Sub-millisecond routing decisions
- **Docker-ready** — One-command deployment with Docker Compose
- **Kubernetes-native** — Helm charts included for cloud deployment
- **Extensible** — Plugin architecture for custom routing logic

---

## 🏃 Quick Start

### Option 1: Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/juspay/decision-engine.git
cd decision-engine

# Start with Docker Compose (includes MySQL, Redis, and all dependencies)
docker compose up -d

# The API will be available at http://localhost:8080
```

### Option 2: Cargo

```bash
# Prerequisites: Rust 1.85+, MySQL or PostgreSQL, Redis

# Clone and build
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
cargo build --release

# Copy and configure
cp config.example.toml config/development.toml
# Edit config/development.toml with your database and Redis settings

# Run database migrations
diesel migration run

# Start the server
./target/release/open_router
```

### Option 3: Docker with PostgreSQL

```bash
# Use the PostgreSQL variant
docker compose -f docker-compose.yaml --profile postgres up -d
```

### Verify Installation

```bash
# Health check
curl http://localhost:8080/health

# Expected response: {"status":"ok"}
```

---

## 📖 Documentation

| Resource | Description |
|----------|-------------|
| [Setup Guide (MySQL)](docs/setup-guide-mysql.md) | Step-by-step MySQL setup |
| [Setup Guide (PostgreSQL)](docs/setup-guide-postgres.md) | Step-by-step PostgreSQL setup |
| [API Reference](docs/api-reference1.md) | Complete API documentation |
| [Configuration](docs/configuration.md) | Configuration options explained |
| [Blog Post](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine) | Deep dive into routing logic |

---

## 🏗 Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Payment Orchestrator                      │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Decision Engine                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ Eligibility │  │   Rules     │  │   Dynamic Ordering      │  │
│  │    Check    │  │   Engine    │  │  (Success Rate + ML)    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              Downtime Detection & Health Monitor            │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              ▼                  ▼                  ▼
        ┌──────────┐      ┌──────────┐      ┌──────────┐
        │ Gateway A│      │ Gateway B│      │ Gateway C│
        └──────────┘      └──────────┘      └──────────┘
```

### Integration with Your Architecture

Decision Engine sits between your payment orchestrator and payment gateways:

```
┌────────────────┐     ┌─────────────────┐     ┌──────────────────┐
│   Your App     │────▶│ Orchestrator    │────▶│ Decision Engine  │
└────────────────┘     └─────────────────┘     └──────────────────┘
                                                       │
                                        ┌──────────────┴──────────────┐
                                        ▼                             ▼
                                 ┌─────────────┐              ┌─────────────┐
                                 │   Vault     │              │   Gateway   │
                                 │ (PCI-compliant)            │   APIs      │
                                 └─────────────┘              └─────────────┘
```

---

## 🗺 Roadmap

| Status | Feature | Description |
|--------|---------|-------------|
| ✅ | Rule-based routing | Merchant-defined priority rules |
| ✅ | Dynamic ordering | ML-driven gateway selection |
| ✅ | Downtime detection | Automatic gateway health monitoring |
| ✅ | Multi-database | MySQL and PostgreSQL support |
| 🔄 | Enhanced ML models | Improved success rate prediction |
| 🔄 | Admin dashboard | Visual rule management UI |
| 📋 | Multi-tenant analytics | Per-tenant routing insights |
| 📋 | GraphQL API | Alternative API interface |

---

## 🤝 Contributing

We welcome contributions! Here's how to get started:

```bash
# Fork the repository
git clone https://github.com/YOUR_USERNAME/decision-engine.git
cd decision-engine

# Create a feature branch
git checkout -b feature/your-feature

# Make your changes and test
cargo test

# Submit a pull request
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

**Good first issues:** Check out [issues labeled `good first issue`](https://github.com/juspay/decision-engine/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) for beginner-friendly tasks.

---

## 💬 Community & Support

| Platform | Purpose |
|----------|---------|
| [Slack](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw) | Real-time discussions, questions, support |
| [GitHub Discussions](https://github.com/juspay/decision-engine/discussions) | Feature requests, roadmap discussions, ideas |
| [GitHub Issues](https://github.com/juspay/decision-engine/issues) | Bug reports, feature requests |

---

## 📜 License

Decision Engine is licensed under the [GNU Affero General Public License v3.0](LICENSE).

---

## 🙏 Acknowledgments

Built with ❤️ by [Juspay](https://juspay.io) and the open-source community.

**Vision:** Build reliable, open-source payments software for the world — interoperable, collaborative, and community-driven.

---

<div align="center">

**[⬆ Back to Top](#decision-engine)**

</div>
