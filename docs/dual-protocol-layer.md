# Protocol Notes

This repository currently documents and ships the following public surfaces:

## Main HTTP API

Served by the main application server.

Key endpoints include:

- `GET /health`
- `POST /decide-gateway`
- `POST /update-gateway-score`
- `POST /merchant-account/create`
- `GET /merchant-account/{merchantId}`
- `DELETE /merchant-account/{merchantId}`
- `POST /routing/*`
- `POST /rule/*`

Main server wiring lives in `src/app.rs`.

## Health Variants

The health router in `src/routes/health.rs` exposes:

- `/health`
- `/health/ready`
- `/health/diagnostics`

Use `/health` for basic liveness checks.

## Metrics

Metrics are served by a separate server built in `src/metrics.rs`.

Default local metrics endpoint:

- `http://localhost:9094/metrics` for source or compose runs that expose the metrics port directly

## Dashboard And Docs Proxy

In the `dashboard-*` compose profiles, Nginx serves:

- the React dashboard at `/dashboard/`
- Mintlify docs pages such as `/introduction`
- proxied API traffic

The proxy config lives in `nginx/nginx.conf`.
