/**
 * Saturation test — finds the server's true throughput ceiling.
 *
 * No sleep between requests. Ramps VUs in steps and reports throughput +
 * latency at each plateau so you can see where the server saturates.
 *
 * Usage:
 *   k6 run scripts/load-test/saturation_test.js
 *   k6 run scripts/load-test/saturation_test.js -e MAX_VUS=300
 */

import http from "k6/http";
import { check } from "k6";
import { Trend, Counter, Rate } from "k6/metrics";

const BASE_URL = "http://127.0.0.1:8080";
const MAX_VUS  = parseInt(__ENV.MAX_VUS || "500");
const STEP_DUR = __ENV.STEP_DUR || "30s";  // hold time per VU level
const RAMP_DUR = __ENV.RAMP_DUR || "5s";   // ramp between levels

const LOAD_TEST_EMAIL    = "loadtest@decision-engine.local";
const LOAD_TEST_PASSWORD = "LoadTest#123456";

// VU levels to test — stop at MAX_VUS
const _DEFAULT_STEPS = [5, 10, 20, 30, 50, 75, 100, 150, 200, 300, 400, 500];
const VU_STEPS = _DEFAULT_STEPS.filter(v => v <= MAX_VUS);
if (VU_STEPS[VU_STEPS.length - 1] < MAX_VUS) VU_STEPS.push(MAX_VUS);

// Build k6 stages: ramp → hold → ramp → hold ...
function buildStages() {
  const stages = [];
  for (const vu of VU_STEPS) {
    stages.push({ duration: RAMP_DUR, target: vu });
    stages.push({ duration: STEP_DUR, target: vu });
  }
  stages.push({ duration: "5s", target: 0 });
  return stages;
}

export const options = {
  stages: buildStages(),
  thresholds: {
    http_req_failed: ["rate<0.05"],  // allow up to 5% errors under saturation
  },
};

const latency    = new Trend("decide_latency_ms", true);
const serverLat  = new Trend("server_latency_ms", true);
const errors     = new Counter("decide_errors");
const successRate = new Rate("decide_success");

const srPayloads = [
  { paymentMethodType: "CARD", paymentMethod: "DEBIT",  authType: "THREE_DS",    cardBrand: "VISA",       currency: "AED", amount: 1000, eligibleGatewayList: ["stripe", "adyen"] },
  { paymentMethodType: "CARD", paymentMethod: "CREDIT", authType: "NO_THREE_DS", cardBrand: "MASTERCARD", currency: "USD", amount: 5000, eligibleGatewayList: ["stripe", "adyen", "checkout"] },
  { paymentMethodType: "CARD", paymentMethod: "DEBIT",  authType: "THREE_DS",    cardBrand: "AMEX",       currency: "EUR", amount: 2500, eligibleGatewayList: ["adyen", "braintree"] },
];

// ── Setup: identical auth flow to load_test.js ───────────────────────────────
export function setup() {
  const json = { "Content-Type": "application/json" };
  let token, merchantId;

  const loginRes = http.post(`${BASE_URL}/auth/login`,
    JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
    { headers: json });

  if (loginRes.status === 200) {
    const b = JSON.parse(loginRes.body);
    token = b.token;
    merchantId = b.merchant_id;
  } else {
    const signupRes = http.post(`${BASE_URL}/auth/signup`,
      JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
      { headers: json });
    if (signupRes.status !== 200) throw new Error(`Signup failed (${signupRes.status}): ${signupRes.body}`);
    const sb = JSON.parse(signupRes.body);
    token = sb.token;
    merchantId = sb.merchant_id;
  }

  if (!merchantId) {
    const authHeaders = { ...json, Authorization: `Bearer ${token}` };
    const merchantRes = http.post(`${BASE_URL}/onboarding/merchant`,
      JSON.stringify({ merchant_name: "Saturation Test Merchant" }),
      { headers: authHeaders });
    if (merchantRes.status !== 200) throw new Error(`Merchant create failed (${merchantRes.status}): ${merchantRes.body}`);
    const mb = JSON.parse(merchantRes.body);
    token = mb.token;
    merchantId = mb.merchant_id;
  }

  console.log(`[setup] merchantId=${merchantId}  stages=${VU_STEPS.join("→")} VUs`);
  return { token, merchantId };
}

// ── Main VU loop — no sleep, tight decide-gateway loop ───────────────────────
export default function (data) {
  const { token, merchantId } = data;
  const headers = {
    "Content-Type": "application/json",
    Accept: "*/*",
    Authorization: `Bearer ${token}`,
    "x-feature": "decision-engine",
    "x-tenant-id": "public",
  };

  const variant   = srPayloads[__ITER % 3];
  const paymentId = `sat_${__VU}_${__ITER}`;

  const body = JSON.stringify({
    merchantId,
    paymentInfo: {
      paymentId,
      amount:            variant.amount,
      currency:          variant.currency,
      paymentType:       "ORDER_PAYMENT",
      paymentMethodType: variant.paymentMethodType,
      paymentMethod:     variant.paymentMethod,
      authType:          variant.authType,
      cardBrand:         variant.cardBrand,
    },
    eligibleGatewayList: variant.eligibleGatewayList,
    rankingAlgorithm:    "SR_BASED_ROUTING",
    eliminationEnabled:  false,
  });

  const start = Date.now();
  const res = http.post(`${BASE_URL}/decide-gateway`, body, {
    headers,
    tags: { name: "decide" },
  });
  const elapsed = Date.now() - start;

  latency.add(elapsed);
  const svr = parseInt(res.headers["X-Response-Time"] || res.headers["x-response-time"] || "0");
  if (svr > 0) serverLat.add(svr);

  const ok = check(res, { "200": r => r.status === 200 });
  successRate.add(ok);
  if (!ok) errors.add(1);
}

// ── Summary ───────────────────────────────────────────────────────────────────
export function handleSummary(data) {
  const m  = data.metrics;
  const rps = (m.http_reqs?.values?.rate ?? 0).toFixed(1);
  const avg = (m.decide_latency_ms?.values?.avg ?? 0).toFixed(1);
  const p50 = (m.decide_latency_ms?.values?.["p(50)"] ?? 0).toFixed(1);
  const p95 = (m.decide_latency_ms?.values?.["p(95)"] ?? 0).toFixed(1);
  const p99 = (m.decide_latency_ms?.values?.["p(99)"] ?? 0).toFixed(1);
  const errRate = ((m.http_req_failed?.values?.rate ?? 0) * 100).toFixed(2);
  const total = m.http_reqs?.values?.count ?? 0;

  const out = [
    "",
    "╔══════════════════════════════════════════════╗",
    "║  Saturation Test Results                     ║",
   `║  VU steps: ${VU_STEPS.join("→").padEnd(34)}║`,
    "╠══════════════════════════════════════════════╣",
   `║  Total Requests : ${String(total).padEnd(26)}║`,
   `║  Peak RPS       : ${rps.padEnd(26)}║`,
    "╠══════════════════════════════════════════════╣",
   `║  Latency (ms)   avg=${avg.padEnd(8)} p50=${p50.padEnd(8)}║`,
   `║                 p95=${p95.padEnd(8)} p99=${p99.padEnd(8)}║`,
    "╠══════════════════════════════════════════════╣",
   `║  Error Rate     : ${errRate.padEnd(26)}║`,
    "╚══════════════════════════════════════════════╝",
    "",
    "Note: peak RPS above is the overall average.",
    "Watch the k6 progress output to see per-stage throughput.",
    "",
  ].join("\n");

  console.log(out);
  return { stdout: out };
}
