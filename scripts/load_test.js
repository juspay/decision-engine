/**
 * k6 load test for /decide-gateway endpoint
 *
 * Usage:
 *   # Local (auto-provisions user + merchant on first run — no token needed)
 *   k6 run scripts/load_test.js
 *
 *   # Local with explicit token override
 *   k6 run scripts/load_test.js -e TOKEN=$(bash scripts/gen_local_token.sh)
 *
 *   # Sandbox
 *   k6 run scripts/load_test.js -e ENV=sandbox -e TOKEN=<your_jwt> -e MERCHANT_ID=<merchant_id>
 *
 *   # Override concurrency / duration
 *   k6 run scripts/load_test.js -e ENV=sandbox -e TOKEN=<your_jwt> -e MERCHANT_ID=<id> -e VUS=50 -e DURATION=60s
 *
 * Never commit TOKEN values. Pass them via shell env or a secrets manager.
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend, Counter } from "k6/metrics";

// ── Environment config ────────────────────────────────────────────────────────

const ENV = __ENV.ENV || "local";
const VUS = parseInt(__ENV.VUS || "20");
const DURATION = __ENV.DURATION || "30s";
const RAMP_DURATION = __ENV.RAMP_DURATION || "10s";

function fail(msg) { throw new Error(msg); }

const ENVS = {
  local: {
    baseUrl: "http://127.0.0.1:8080",
    merchantId: __ENV.MERCHANT_ID || null,
  },
  sandbox: {
    baseUrl: "https://app.hyperswitch.io/decision-engine/api",
    merchantId: __ENV.MERCHANT_ID || null,
  },
};

const envDefaults = ENVS[ENV];
if (!envDefaults) {
  throw new Error(`Unknown ENV="${ENV}". Use "local" or "sandbox".`);
}
if (ENV === "sandbox") {
  if (!__ENV.TOKEN) fail('TOKEN is required for sandbox: -e TOKEN=<your_jwt>');
  if (!envDefaults.merchantId) fail('MERCHANT_ID is required for sandbox: -e MERCHANT_ID=<id>');
}

// ── k6 options ────────────────────────────────────────────────────────────────

export const options = {
  stages: [
    { duration: RAMP_DURATION, target: VUS },
    { duration: DURATION, target: VUS },
    { duration: "5s", target: 0 },
  ],
  thresholds: {
    http_req_duration: ["p(95)<500", "p(99)<1000"],
    http_req_failed: ["rate<0.01"],
    decide_gateway_errors: ["count<5"],
  },
};

// ── Custom metrics ────────────────────────────────────────────────────────────

const gatewaySelected = new Counter("decide_gateway_selected");
const gatewayErrors = new Counter("decide_gateway_errors");
const latencyTrend = new Trend("decide_gateway_latency", true);
const successRate = new Rate("decide_gateway_success_rate");

// Latency breakdown: server-side (from response.latency) vs network overhead
// network = k6 round-trip duration - server-side latency reported by the pod
const serverLatency = new Trend("server_latency_ms", true);
const networkLatency = new Trend("network_latency_ms", true);

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

// ── Setup: provision user + merchant for local; validate token for sandbox ────

const LOAD_TEST_EMAIL = "loadtest@decision-engine.local";
const LOAD_TEST_PASSWORD = "LoadTest#123456";

export function setup() {
  const baseUrl = envDefaults.baseUrl;
  const jsonHeaders = { "Content-Type": "application/json" };

  // Sandbox: token must be provided via env, nothing to provision
  if (ENV !== "local") {
    return { token: __ENV.TOKEN, merchantId: envDefaults.merchantId };
  }

  // Local with explicit token override: skip provisioning
  if (__ENV.TOKEN) {
    const merchantId = envDefaults.merchantId;
    if (!merchantId) fail('MERCHANT_ID is required when using TOKEN override: -e MERCHANT_ID=<id>');
    return { token: __ENV.TOKEN, merchantId };
  }

  // ── Auto-provision: login, or signup then create merchant ──────────────────

  // 1. Try login first (idempotent — works on every subsequent run)
  const loginRes = http.post(
    `${baseUrl}/auth/login`,
    JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
    { headers: jsonHeaders }
  );

  let token, merchantId;

  if (loginRes.status === 200) {
    const body = JSON.parse(loginRes.body);
    token = body.token;
    merchantId = body.merchant_id;
  } else {
    // 2. First run: signup
    const signupRes = http.post(
      `${baseUrl}/auth/signup`,
      JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
      { headers: jsonHeaders }
    );
    if (signupRes.status !== 200) {
      fail(`Signup failed (${signupRes.status}): ${signupRes.body}`);
    }
    const signupBody = JSON.parse(signupRes.body);
    token = signupBody.token;
    merchantId = signupBody.merchant_id;
  }

  // 3. Create merchant if the user has none yet
  if (!merchantId) {
    const authHeaders = { ...jsonHeaders, Authorization: `Bearer ${token}` };
    const merchantRes = http.post(
      `${baseUrl}/onboarding/merchant`,
      JSON.stringify({ merchant_name: "Load Test Merchant" }),
      { headers: authHeaders }
    );
    if (merchantRes.status !== 200) {
      fail(`Merchant creation failed (${merchantRes.status}): ${merchantRes.body}`);
    }
    const mBody = JSON.parse(merchantRes.body);
    token = mBody.token; // refreshed token with merchant_id embedded
    merchantId = mBody.merchant_id;
  }

  console.log(`[setup] using merchant_id=${merchantId}`);
  return { token, merchantId };
}

// ── Main VU function ──────────────────────────────────────────────────────────

export default function (data) {
  const token = data.token;
  const merchantId = data.merchantId;

  const headers = {
    "Content-Type": "application/json",
    Accept: "*/*",
    Authorization: `Bearer ${token}`,
    "x-feature": "decision-engine",
    "x-tenant-id": "public",
  };

  const variant = payloads[__VU % payloads.length];
  const paymentId = `load_test_${Date.now()}_${__VU}_${__ITER}`;

  const body = JSON.stringify({
    merchantId,
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
  const res = http.post(`${envDefaults.baseUrl}/decide-gateway`, body, { headers });
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
      // response.latency is the server-side processing time reported by the pod.
      // Subtracting it from the k6 round-trip gives the network + infra overhead.
      const podMs = json.latency;
      if (typeof podMs === "number" && podMs >= 0) {
        const roundTripMs = res.timings.duration;
        serverLatency.add(podMs);
        networkLatency.add(Math.max(0, roundTripMs - podMs));
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
  const successCount = metrics.decide_gateway_selected?.values?.count ?? "N/A";

  const svrAvg = metrics.server_latency_ms?.values?.avg?.toFixed(1) ?? "N/A";
  const svrP50 = metrics.server_latency_ms?.values?.med?.toFixed(1) ?? "N/A";
  const svrP95 = metrics.server_latency_ms?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const netAvg = metrics.network_latency_ms?.values?.avg?.toFixed(1) ?? "N/A";
  const netP50 = metrics.network_latency_ms?.values?.med?.toFixed(1) ?? "N/A";
  const netP95 = metrics.network_latency_ms?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const hasLatencyBreakdown = metrics.server_latency_ms !== undefined;

  const latencyBreakdown = hasLatencyBreakdown ? `
╠══════════════════════════════════════════════════════════╣
║  Server-side latency (pod, ms)                           ║
║    avg             : ${String(svrAvg).padEnd(36)}║
║    p50             : ${String(svrP50).padEnd(36)}║
║    p95             : ${String(svrP95).padEnd(36)}║
╠══════════════════════════════════════════════════════════╣
║  Network overhead  (round-trip − pod, ms)                ║
║    avg             : ${String(netAvg).padEnd(36)}║
║    p50             : ${String(netP50).padEnd(36)}║
║    p95             : ${String(netP95).padEnd(36)}║` : "";

  const summary = `
╔══════════════════════════════════════════════════════════╗
║          decide-gateway Load Test Results                ║
║  Environment : ${ENV.padEnd(42)}║
╠══════════════════════════════════════════════════════════╣
║  Total Requests    : ${String(total).padEnd(36)}║
║  Throughput        : ${(rps + " req/s").padEnd(36)}║
╠══════════════════════════════════════════════════════════╣
║  Round-trip latency (ms)                                 ║
║    avg             : ${String(avg).padEnd(36)}║
║    p50 (median)    : ${String(p50).padEnd(36)}║
║    p95             : ${String(p95).padEnd(36)}║
║    p99             : ${String(p99).padEnd(36)}║
║    max             : ${String(maxLat).padEnd(36)}║${latencyBreakdown}
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
