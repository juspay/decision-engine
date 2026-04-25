<div align="center">

<img src="https://img.shields.io/badge/Rust-1.85%2B-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
<img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=for-the-badge" alt="License"/>
<img src="https://img.shields.io/badge/Docker-Ready-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker"/>
<img src="https://img.shields.io/github/v/release/juspay/decision-engine?include_prereleases&style=for-the-badge&label=Release&color=brightgreen" alt="Release"/>
<img src="https://img.shields.io/badge/Slack-Join%20Chat-4A154B?style=for-the-badge&logo=slack&logoColor=white" alt="Slack"/>

<br/><br/>

# ⚡ Decision Engine

### **Routing control plane for payment decisions**

**Open-Source • Rust • Rule-Based • Success-Rate Based**

Configure routing rules, run gateway decisions, and inspect routing outcomes from APIs or the dashboard.

---

**[🚀 Quick Start](#-quick-start)** •
**[📚 Documentation](#-documentation)** •
**[🏗 Architecture](#-architecture)** •
**[🤝 Contributing](#-contributing)**

</div>

---

## 🎯 What is Decision Engine?

Decision Engine is a payment gateway routing service built in Rust. It selects a gateway for each request using merchant-configured routing strategy, success-rate scores, rule-based policies, volume splits, and debit-routing gates.

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Payment    │────▶│  Decision Engine │────▶│  Best       │
│  Request    │     │  (Fast routing)  │     │  Gateway    │
└─────────────┘     └──────────────────┘     └─────────────┘
```

### Core Use Cases

| Use case | Supported surface |
|----------|-------------------|
| Route by connector score | Auth-rate based routing |
| Route by explicit business condition | Rule-based routing |
| Roll out traffic by percentage | Volume split |
| Gate debit-network routing per merchant | Debit routing toggle |
| Inspect routing outcomes | Analytics and Decision Audit dashboard |

---

## ✨ Features

<table>
<tr>
<td width="50%">

### 🧠 Intelligent Routing

| Feature | What It Does |
|---------|--------------|
| **Eligibility Check** | Filters out ineligible gateways before routing |
| **Rule-Based Ordering** | Apply merchant-specific priority rules |
| **Dynamic Ordering** | Success rate based gateway optimization |
| **Downtime Detection** | Auto-pause failing gateways |

</td>
<td width="50%">

### 🛠 Operational Surfaces

| Capability | Details |
|------------|---------|
| **Dashboard** | Configure routing and inspect analytics/audit views |
| **API reference** | OpenAPI-backed route documentation and curl examples |
| **Analytics storage** | ClickHouse-backed analytics tables for local and deployed environments |
| **Multi-DB app storage** | MySQL and PostgreSQL support |
| **Deployment artifacts** | Docker Compose and Helm configuration are included |

</td>
</tr>
</table>

---

## 🏃 Quick Start

### 🐳 Docker (Recommended)

```bash
# Clone and run
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
docker compose --profile postgres-ghcr up -d

# That's it! API ready at http://localhost:8080
```

For API + dashboard + docs:

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

Open:

- API: `http://localhost:8080`
- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`
- API examples: `http://localhost:8081/api-refs/api-ref`

### 🦀 From Source

```bash
# Prerequisites: Rust 1.85+, MySQL/PostgreSQL, Redis

git clone https://github.com/juspay/decision-engine.git
cd decision-engine
cargo build --release

# Configure
cp config.example.toml config/development.toml
# Edit config with your settings

# Run migrations & start
diesel migration run
./target/release/open_router
```

### ✅ Verify

```bash
curl http://localhost:8080/health
# → {"message":"Health is good"}
```

---

## 📖 Documentation

| 📘 Resource | Description |
|-------------|-------------|
| [Local Setup Guide](docs/local-setup.md) | Canonical guide for CLI, Docker, Compose profiles, and Helm |
| [MySQL Setup Guide](docs/setup-guide-mysql.md) | MySQL-specific walkthrough |
| [PostgreSQL Setup Guide](docs/setup-guide-postgres.md) | PostgreSQL-specific walkthrough |
| [API Reference](docs/api-reference.md) | OpenAPI-backed REST API documentation |
| [API Examples](docs/api-refs/api-ref.mdx) | Curl examples for every route family and routing flavour |
| [Configuration Guide](docs/configuration.md) | All config options explained |
| [Deep Dive Blog](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine) | How routing logic works |

---

## 🏗 Architecture

### High-Level Flow

<div align="center">
  <img src="https://cdn.sanity.io/images/9sed75bn/production/fd872ae5b086e7a60011ad9d4d5c7988e1084d03-1999x1167.png" alt="Decision Engine Architecture" width="80%"/>
</div>

### Integration Pattern

Decision Engine integrates seamlessly into your existing payment stack:

<div align="center">
  <img src="https://github.com/user-attachments/assets/272ad222-8a91-4bb2-aa3a-e1fc9c28e3da" alt="Integration Pattern" width="70%"/>
</div>

**Integration Steps:**

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

**Key Benefits:**
- **Zero PCI scope** — Vault handles all card data
- **Drop-in integration** — Works with any orchestrator
- **Intelligent fallback** — Auto-switches on gateway failure

---

## 🗺 Roadmap

| Status | Feature | Description |
|:------:|---------|-------------|
| ✅ | Rule-based routing | Merchant-defined priority rules |
| ✅ | Dynamic ordering | SR-based gateway selection |
| ✅ | Downtime detection | Automatic health monitoring |
| ✅ | Multi-database | MySQL & PostgreSQL support |
| 🔄 | Enhanced routing models | Better success rate prediction |
| 🔄 | Admin dashboard | Visual rule management UI |
| 📋 | Multi-tenant analytics | Per-tenant routing insights |
| 📋 | GraphQL API | Alternative query interface |

---

## 🤝 Contributing

We ❤️ contributions!

```bash
# 1. Fork & clone
git clone https://github.com/YOUR_USERNAME/decision-engine.git

# 2. Create branch
git checkout -b feature/your-feature

# 3. Make changes & test
cargo test

# 4. Submit PR!
```

👉 See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

🌱 **New to open source?** Check out [good first issues](https://github.com/juspay/decision-engine/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)!

---

## 💬 Community

| Platform | What It's For |
|----------|---------------|
| [![Slack](https://img.shields.io/badge/Slack-Join_Chat-4A154B?logo=slack)](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw) | Real-time help, discussions |
| [GitHub Discussions](https://github.com/juspay/decision-engine/discussions) | Ideas, feature requests |
| [GitHub Issues](https://github.com/juspay/decision-engine/issues) | Bug reports |

---

## 📜 License

Licensed under [GNU AGPL v3.0](LICENSE).

---

<div align="center">

### Built with ❤️ by [Juspay](https://juspay.io)

*Reliable, open-source payments infrastructure for the world.*

**[⬆ Back to Top](#-decision-engine)**

</div>
