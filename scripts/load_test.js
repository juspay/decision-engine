/**
 * k6 load test for /decide-gateway endpoint
 *
 * Usage:
 *   # Localhost
 *   k6 run scripts/load_test.js
 *
 *   # Sandbox
 *   k6 run scripts/load_test.js -e ENV=sandbox
 *
 *   # Override concurrency / duration
 *   k6 run scripts/load_test.js -e ENV=sandbox -e VUS=50 -e DURATION=60s
 *
 *   # With custom auth token
 *   k6 run scripts/load_test.js -e TOKEN=<your_jwt>
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend, Counter } from "k6/metrics";

// ── Environment config ────────────────────────────────────────────────────────

const ENV = __ENV.ENV || "local";
const VUS = parseInt(__ENV.VUS || "20");
const DURATION = __ENV.DURATION || "30s";
const RAMP_DURATION = __ENV.RAMP_DURATION || "10s";

const ENVS = {
  local: {
    baseUrl: "http://127.0.0.1:8080",
    // Re-generate with: bash scripts/gen_local_token.sh (or login via /auth/signup + /onboarding/merchant)
    token:
      __ENV.TOKEN ||
      "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZGM4OWJiYy03MDI2LTQ3YzItYjA4YS04NmNlMDZmYjE3NjMiLCJ1c2VyX2lkIjoiYWRjODliYmMtNzAyNi00N2MyLWIwOGEtODZjZTA2ZmIxNzYzIiwiZW1haWwiOiJsb2FkdGVzdEB0ZXN0LmNvbSIsIm1lcmNoYW50X2lkIjoibWVyY2hhbnRfYmFlYTI1YzUzNjI2Iiwicm9sZSI6ImFkbWluIiwianRpIjoiY2ZhY2MzZjQtOTkwZi00NGY3LTg5ZWEtZGNjODRiMzhmZDg1IiwiaWF0IjoxNzc5MzcwNjk5LCJleHAiOjE3Nzk0NTcwOTl9.T44OI_8-j1Pw6zr2WEoxcpBbvYL_I3ygpSKMZz0JKZA",
    merchantId: "merchant_baea25c53626",
  },
  sandbox: {
    baseUrl: "https://app.hyperswitch.io/decision-engine/api",
    token:
      __ENV.TOKEN ||
      "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxNmVjNmY0YS05Njc3LTRjNzktOTgzOC1mMjRlMTQ4MDk5MjEiLCJ1c2VyX2lkIjoiMTZlYzZmNGEtOTY3Ny00Yzc5LTk4MzgtZjI0ZTE0ODA5OTIxIiwiZW1haWwiOiJ0ZXN0QGdtYWlsLmNvbSIsIm1lcmNoYW50X2lkIjoibWVyY2hhbnRfNjIxYjFkMTRlNDk5Iiwicm9sZSI6ImFkbWluIiwianRpIjoiMzAzMWQ4Y2MtMjkxNC00ZjE3LTllODItYjBkMWUwZGI5MjY5IiwiaWF0IjoxNzc5MzcwMTc4LCJleHAiOjE3Nzk0NTY1Nzh9.NJiV4-a1YODBNqOLK3tOPU1eAdzfQTDJq33OojSvLy8",
    merchantId: "merchant_621b1d14e499",
  },
};

const config = ENVS[ENV];
if (!config) {
  throw new Error(`Unknown ENV="${ENV}". Use "local" or "sandbox".`);
}

// ── k6 options ────────────────────────────────────────────────────────────────

export const options = {
  stages: [
    { duration: RAMP_DURATION, target: VUS }, // ramp up
    { duration: DURATION, target: VUS }, // steady load
    { duration: "5s", target: 0 }, // ramp down
  ],
  thresholds: {
    http_req_duration: ["p(95)<500", "p(99)<1000"], // 95th pct < 500ms, 99th < 1s
    http_req_failed: ["rate<0.01"], // error rate < 1%
    decide_gateway_errors: ["count<5"], // custom: < 5 non-HTTP errors
  },
};

// ── Custom metrics ────────────────────────────────────────────────────────────

const gatewaySelected = new Counter("decide_gateway_selected");
const gatewayErrors = new Counter("decide_gateway_errors");
const latencyTrend = new Trend("decide_gateway_latency", true);
const successRate = new Rate("decide_gateway_success_rate");

// ── Payload variants (rotate to simulate realistic traffic) ───────────────────

const payloads = [
  {
    paymentMethodType: "CARD",
    paymentMethod: "DEBIT",
    authType: "THREE_DS",
    cardBrand: "VISA",
    currency: "AED",
    amount: 1000,
    eligibleGatewayList: ["stripe", "adyen"],
  },
  {
    paymentMethodType: "CARD",
    paymentMethod: "CREDIT",
    authType: "NO_THREE_DS",
    cardBrand: "MASTERCARD",
    currency: "USD",
    amount: 5000,
    eligibleGatewayList: ["stripe", "adyen", "checkout"],
  },
  {
    paymentMethodType: "CARD",
    paymentMethod: "DEBIT",
    authType: "THREE_DS",
    cardBrand: "AMEX",
    currency: "EUR",
    amount: 2500,
    eligibleGatewayList: ["adyen", "braintree"],
  },
];

// ── Headers ───────────────────────────────────────────────────────────────────

const headers = {
  "Content-Type": "application/json",
  Accept: "*/*",
  Authorization: `Bearer ${config.token}`,
  "x-feature": "decision-engine",
  "x-tenant-id": "public",
};

// ── Main VU function ──────────────────────────────────────────────────────────

export default function () {
  const variant = payloads[__VU % payloads.length];
  const paymentId = `load_test_${Date.now()}_${__VU}_${__ITER}`;

  const body = JSON.stringify({
    merchantId: config.merchantId,
    paymentInfo: {
      paymentId,
      amount: variant.amount,
      currency: variant.currency,
      paymentType: "ORDER_PAYMENT",
      paymentMethodType: variant.paymentMethodType,
      paymentMethod: variant.paymentMethod,
      authType: variant.authType,
      cardBrand: variant.cardBrand,
    },
    eligibleGatewayList: variant.eligibleGatewayList,
    rankingAlgorithm: "SR_BASED_ROUTING",
    eliminationEnabled: false,
  });

  const start = Date.now();
  const res = http.post(`${config.baseUrl}/decide-gateway`, body, { headers });
  const duration = Date.now() - start;

  latencyTrend.add(duration);

  const ok = check(res, {
    "status is 200": (r) => r.status === 200,
    "has decided_gateway": (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.decided_gateway !== undefined || body.decidedGateway !== undefined;
      } catch {
        return false;
      }
    },
  });

  successRate.add(ok);

  if (res.status === 200) {
    try {
      const json = JSON.parse(res.body);
      const gw = json.decided_gateway || json.decidedGateway;
      if (gw) {
        gatewaySelected.add(1, { gateway: gw });
      }
    } catch (_) {
      gatewayErrors.add(1);
    }
  } else {
    gatewayErrors.add(1);
    if (__ENV.VERBOSE) {
      console.error(
        `[VU ${__VU}] HTTP ${res.status}: ${res.body?.substring(0, 200)}`
      );
    }
  }

  sleep(0.1); // 100ms think time between iterations
}

// ── Summary ───────────────────────────────────────────────────────────────────

export function handleSummary(data) {
  const metrics = data.metrics;

  const rps = metrics.http_reqs?.values?.rate?.toFixed(1) ?? "N/A";
  const p50 = metrics.http_req_duration?.values?.med?.toFixed(1) ?? "N/A";
  const p95 = metrics.http_req_duration?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const p99 = metrics.http_req_duration?.values?.["p(99)"]?.toFixed(1) ?? "N/A";
  const avg = metrics.http_req_duration?.values?.avg?.toFixed(1) ?? "N/A";
  const maxLat = metrics.http_req_duration?.values?.max?.toFixed(1) ?? "N/A";
  const errRate = (
    (metrics.http_req_failed?.values?.rate ?? 0) * 100
  ).toFixed(2);
  const total = metrics.http_reqs?.values?.count ?? 0;
  const successCount =
    metrics.decide_gateway_selected?.values?.count ?? "N/A";

  const summary = `
╔══════════════════════════════════════════════════════════╗
║          decide-gateway Load Test Results                ║
║  Environment : ${ENV.padEnd(42)}║
╠══════════════════════════════════════════════════════════╣
║  Total Requests    : ${String(total).padEnd(36)}║
║  Throughput        : ${(rps + " req/s").padEnd(36)}║
╠══════════════════════════════════════════════════════════╣
║  Latency (ms)                                            ║
║    avg             : ${String(avg).padEnd(36)}║
║    p50 (median)    : ${String(p50).padEnd(36)}║
║    p95             : ${String(p95).padEnd(36)}║
║    p99             : ${String(p99).padEnd(36)}║
║    max             : ${String(maxLat).padEnd(36)}║
╠══════════════════════════════════════════════════════════╣
║  Error Rate        : ${(errRate + "%").padEnd(36)}║
║  Successful Routes : ${String(successCount).padEnd(36)}║
╚══════════════════════════════════════════════════════════╝
`;

  console.log(summary);

  return {
    stdout: summary,
    "scripts/load_test_results.json": JSON.stringify(data, null, 2),
  };
}
