# Installation Guide

Use this page to choose the right installation path. For actual commands, use the linked guides.

## Supported Paths

| Path | Best for | Primary doc |
|------|----------|-------------|
| Docker Compose with published images | fastest local or on-prem style bring-up | [Local Setup Guide](local-setup.md) |
| Docker Compose with local builds | validating local source changes | [Local Setup Guide](local-setup.md) |
| Source build | backend debugging and feature-specific runs | [Local Setup Guide](local-setup.md) |
| Helm | Kubernetes and on-prem deployment work | [Local Setup Guide](local-setup.md) and `helm-charts/README.md` |

## Database-Specific Shortcuts

- PostgreSQL commands: [PostgreSQL Setup Guide](setup-guide-postgres.md)
- MySQL commands: [MySQL Setup Guide](setup-guide-mysql.md)

## Dashboard And Docs

If you want the React dashboard and Mintlify docs, use one of the `dashboard-*` compose profiles from [Local Setup Guide](local-setup.md).

That exposes:

- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`

## Canonical Source

Treat [Local Setup Guide](local-setup.md) as the canonical installation and startup document for this repository.
