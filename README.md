<div align="center">

<img src="https://img.shields.io/badge/Rust-1.85%2B-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
<img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=for-the-badge" alt="License"/>
<img src="https://img.shields.io/badge/Docker-Ready-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker"/>
<img src="https://img.shields.io/github/v/release/juspay/decision-engine?include_prereleases&style=for-the-badge&label=Release&color=brightgreen" alt="Release"/>
<img src="https://img.shields.io/badge/Slack-Join%20Chat-4A154B?style=for-the-badge&logo=slack&logoColor=white" alt="Slack"/>

<br/><br/>

# ⚡ Decision Engine

### **The Brain Behind Smarter Payments**

**Open-Source • High-Performance • ML-Powered**

*Route payments intelligently. Maximize success rates. Zero vendor lock-in.*

---

**[🚀 Quick Start](#-quick-start)** • 
**[📚 Documentation](#-documentation)** • 
**[🏗 Architecture](#-architecture)** • 
**[🤝 Contributing](#-contributing)**

</div>

---

## 🎯 What is Decision Engine?

Decision Engine is a **high-performance payment gateway router** built in Rust that intelligently selects the optimal payment gateway for each transaction — in real-time.

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Payment    │────▶│  Decision Engine │────▶│  Best       │
│  Request    │     │  (Routes in <1ms)│     │  Gateway    │
└─────────────┘     └──────────────────┘     └─────────────┘
```

### Why Teams Choose Decision Engine

| 💥 The Problem | ✅ Our Solution |
|----------------|-----------------|
| Payment failures from gateway downtime | **Real-time health monitoring** with automatic failover |
| Suboptimal routing = lost revenue | **ML-driven routing** based on success rates & latency |
| Vendor lock-in limits flexibility | **Modular design** — works with any orchestrator |
| Complex rule management | **Flexible policies** — rule-based + ML hybrid |

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
| **Dynamic Ordering** | ML optimizes gateway selection in real-time |
| **Downtime Detection** | Auto-pause failing gateways |

</td>
<td width="50%">

### 🛠 Built for Production

| Capability | Details |
|------------|---------|
| **⚡ Blazing Fast** | Sub-millisecond routing decisions |
| **🔐 Memory Safe** | Built in Rust — no data races |
| **📊 Multi-DB** | MySQL & PostgreSQL support |
| **🐳 Docker Ready** | One-command deployment |
| **☸️ K8s Native** | Helm charts included |

</td>
</tr>
</table>

---

## 📊 Performance at a Glance

<div align="center">

| Metric | Value |
|--------|-------|
| Routing Decision Time | **< 1ms** |
| Memory Footprint | **~50MB** |
| Concurrent Requests | **100K+** |
| Uptime SLA Support | **99.99%** |

</div>

---

## 🏃 Quick Start

### 🐳 Docker (Recommended)

```bash
# Clone and run
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
docker compose up -d

# That's it! API ready at http://localhost:8080
```

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
# → {"status":"ok"}
```

---

## 📖 Documentation

| 📘 Resource | Description |
|-------------|-------------|
| [MySQL Setup Guide](docs/setup-guide-mysql.md) | Step-by-step MySQL configuration |
| [PostgreSQL Setup Guide](docs/setup-guide-postgres.md) | Step-by-step PostgreSQL configuration |
| [API Reference](docs/api-reference1.md) | Complete REST API documentation |
| [Configuration Guide](docs/configuration.md) | All config options explained |
| [Deep Dive Blog](https://juspay.io/blog/juspay-orchestrator-and-merchant-controlled-routing-engine) | How routing logic works |

---

## 🏗 Architecture

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     Payment Orchestrator                         │
│                    (Your existing system)                        │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ⚡ Decision Engine                           │
│                                                                  │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│   │  Eligibility │  │    Rules     │  │   Dynamic Ordering   │  │
│   │    Check     │  │    Engine    │  │  (ML + Success Rate) │  │
│   └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                                                                  │
│   ┌────────────────────────────────────────────────────────────┐ │
│   │          🔍 Downtime Detection & Health Monitoring         │ │
│   └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└───────────────────────────────┬─────────────────────────────────┘
                                │
          ┌─────────────────────┼─────────────────────┐
          ▼                     ▼                     ▼
    ┌───────────┐         ┌───────────┐         ┌───────────┐
    │ Gateway A │         │ Gateway B │         │ Gateway C │
    │  (Stripe) │         │  (Adyen)  │         │ (PayPal)  │
    └───────────┘         └───────────┘         └───────────┘
```

### Integration Pattern

Decision Engine integrates seamlessly into your existing payment stack:

```
                              REQUEST FLOW ▶
┌─────────────┐     ┌─────────────────┐     ┌─────────────────────┐
│   Your App  │────▶│  Orchestrator   │────▶│   Decision Engine   │
│             │     │                 │     │                     │
│ Payment Req │     │ Route to DE     │     │ 1. Check eligibility│
└─────────────┘     └─────────────────┘     │ 2. Apply rules      │
                                            │ 3. ML optimization  │
                                            └──────────┬──────────┘
                                                       │
                          ┌────────────────────────────┼────────────────────────┐
                          │                            │                        │
                          ▼                            ▼                        ▼
                   ┌─────────────┐              ┌─────────────┐          ┌─────────────┐
                   │ Vault (PCI) │              │  Gateway A  │          │  Gateway B  │
                   │             │◀─────────────│  (Stripe)   │          │  (Adyen)    │
                   │ Card Tokens │   Token      │             │          │             │
                   └─────────────┘   Lookup      └──────┬──────┘          └──────┬──────┘
                                                    │                        │
                                                    └────────────┬───────────┘
                                                                 │
◀ RESPONSE FLOW                                                  │
┌─────────────┐     ┌─────────────────┐     ┌─────────────────▼───────────┐
│   Your App  │◀────│  Orchestrator   │◀────│   Gateway Response          │
│             │     │                 │     │   (success/decline/redirect)│
│ Final Result│     │ Return to app   │     └─────────────────────────────┘
└─────────────┘     └─────────────────┘
```

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
| ✅ | Dynamic ordering | ML-driven gateway selection |
| ✅ | Downtime detection | Automatic health monitoring |
| ✅ | Multi-database | MySQL & PostgreSQL support |
| 🔄 | Enhanced ML models | Better success rate prediction |
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
