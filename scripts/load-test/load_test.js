/**
 * k6 load test for /decide-gateway (SR_BASED_ROUTING) and /routing/evaluate (RULE_BASED_ROUTING)
 *
 * Usage:
 *   # Local — auto-provisions user + merchant, no token needed
 *   k6 run scripts/load-test/load_test.js
 *   k6 run scripts/load-test/load_test.js -e ALGORITHM=RULE_BASED_ROUTING
 *
 *   # Sandbox
 *   k6 run scripts/load-test/load_test.js -e ENV=sandbox -e TOKEN=<jwt> -e MERCHANT_ID=<id>
 *   k6 run scripts/load-test/load_test.js -e ENV=sandbox -e TOKEN=<jwt> -e MERCHANT_ID=<id> -e ALGORITHM=RULE_BASED_ROUTING
 *
 *   # Override concurrency / duration
 *   k6 run scripts/load-test/load_test.js -e VUS=50 -e DURATION=60s
 *
 * ALGORITHM options:
 *   SR_BASED_ROUTING   → POST /decide-gateway  (default)
 *   RULE_BASED_ROUTING → POST /routing/evaluate (setup creates + activates a priority rule once)
 *
 * Never commit TOKEN values.
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend, Counter } from "k6/metrics";

// ── Environment config ────────────────────────────────────────────────────────

const ENV       = __ENV.ENV       || "local";
const VUS       = parseInt(__ENV.VUS || "20");
const DURATION  = __ENV.DURATION  || "30s";
const RAMP_DURATION = __ENV.RAMP_DURATION || "10s";
const ALGORITHM = __ENV.ALGORITHM || "SR_BASED_ROUTING";

const VALID_ALGORITHMS = ["SR_BASED_ROUTING", "RULE_BASED_ROUTING"];
if (!VALID_ALGORITHMS.includes(ALGORITHM)) {
  throw new Error(`Unknown ALGORITHM="${ALGORITHM}". Valid: ${VALID_ALGORITHMS.join(", ")}`);
}

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
if (!envDefaults) throw new Error(`Unknown ENV="${ENV}". Use "local" or "sandbox".`);
if (ENV === "sandbox") {
  if (!__ENV.TOKEN) fail('TOKEN is required for sandbox: -e TOKEN=<your_jwt>');
  if (!envDefaults.merchantId) fail('MERCHANT_ID is required for sandbox: -e MERCHANT_ID=<id>');
}

// ── k6 options ────────────────────────────────────────────────────────────────

export const options = {
  stages: [
    { duration: RAMP_DURATION, target: VUS },
    { duration: DURATION,      target: VUS },
    { duration: "5s",          target: 0   },
  ],
  thresholds: {
    http_req_duration:     ["p(95)<500", "p(99)<1000"],
    http_req_failed:       ["rate<0.01"],
    decide_gateway_errors: ["count<5"],
  },
};

// ── Custom metrics ────────────────────────────────────────────────────────────

const gatewaySelected  = new Counter("decide_gateway_selected");
const gatewayErrors    = new Counter("decide_gateway_errors");
const latencyTrend     = new Trend("decide_gateway_latency", true);
const successRate      = new Rate("decide_gateway_success_rate");
const serverLatency    = new Trend("server_latency_ms", true);
const networkLatency   = new Trend("network_latency_ms", true);
const feedbackLatency  = new Trend("feedback_latency_ms", true);
const feedbackErrors   = new Counter("feedback_errors");
const decideReqs       = new Counter("decide_reqs");

// ── SR payload variants ───────────────────────────────────────────────────────

const srPayloads = [
  { paymentMethodType: "CARD", paymentMethod: "DEBIT",  authType: "THREE_DS",    cardBrand: "VISA",       currency: "AED", amount: 1000, eligibleGatewayList: ["stripe", "adyen"] },
  { paymentMethodType: "CARD", paymentMethod: "CREDIT", authType: "NO_THREE_DS", cardBrand: "MASTERCARD", currency: "USD", amount: 5000, eligibleGatewayList: ["stripe", "adyen", "checkout"] },
  { paymentMethodType: "CARD", paymentMethod: "DEBIT",  authType: "THREE_DS",    cardBrand: "AMEX",       currency: "EUR", amount: 2500, eligibleGatewayList: ["adyen", "braintree"] },
];

// ── Rule-based (routing/evaluate) parameter variants ─────────────────────────
// Each variant maps payment attributes to the keys configured in routing_config.

const rulePayloads = [
  { amount: 1000, authentication_type: "three_ds",    currency: "AED" },
  { amount: 5000, authentication_type: "no_three_ds", currency: "USD" },
  { amount: 2500, authentication_type: "three_ds",    currency: "EUR" },
];

// ── Setup ─────────────────────────────────────────────────────────────────────

const LOAD_TEST_EMAIL    = "loadtest@decision-engine.local";
const LOAD_TEST_PASSWORD = "LoadTest#123456";

// Priority rule used for RULE_BASED_ROUTING: checkout > stripe > adyen
const PRIORITY_RULE = {
  name:          "load-test-priority",
  description:   "Load test priority rule: checkout > stripe > adyen",
  algorithm_for: "payment",
  algorithm: {
    type: "priority",
    data: [
      { gateway_name: "checkout", gateway_id: "mca_checkout" },
      { gateway_name: "stripe",   gateway_id: "mca_stripe"   },
      { gateway_name: "adyen",    gateway_id: "mca_adyen"    },
    ],
  },
};

export function setup() {
  const baseUrl     = envDefaults.baseUrl;
  const jsonHeaders = { "Content-Type": "application/json" };

  // Sandbox: token must be provided via env
  if (ENV !== "local") {
    const data = { token: __ENV.TOKEN, merchantId: envDefaults.merchantId };
    if (ALGORITHM === "RULE_BASED_ROUTING") {
      _ensureRoutingRule(baseUrl, data.token, data.merchantId, jsonHeaders);
    }
    return data;
  }

  // Local with explicit token override
  if (__ENV.TOKEN) {
    const merchantId = envDefaults.merchantId;
    if (!merchantId) fail('MERCHANT_ID is required when using TOKEN override: -e MERCHANT_ID=<id>');
    const data = { token: __ENV.TOKEN, merchantId };
    if (ALGORITHM === "RULE_BASED_ROUTING") {
      _ensureRoutingRule(baseUrl, data.token, data.merchantId, jsonHeaders);
    }
    return data;
  }

  // ── Auto-provision: login → signup → create merchant ─────────────────────
  let token, merchantId;

  const loginRes = http.post(
    `${baseUrl}/auth/login`,
    JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
    { headers: jsonHeaders }
  );

  if (loginRes.status === 200) {
    const body = JSON.parse(loginRes.body);
    token = body.token;
    merchantId = body.merchant_id;
  } else {
    const signupRes = http.post(
      `${baseUrl}/auth/signup`,
      JSON.stringify({ email: LOAD_TEST_EMAIL, password: LOAD_TEST_PASSWORD }),
      { headers: jsonHeaders }
    );
    if (signupRes.status !== 200) fail(`Signup failed (${signupRes.status}): ${signupRes.body}`);
    const sb = JSON.parse(signupRes.body);
    token = sb.token;
    merchantId = sb.merchant_id;
  }

  if (!merchantId) {
    const authHeaders = { ...jsonHeaders, Authorization: `Bearer ${token}` };
    const merchantRes = http.post(
      `${baseUrl}/onboarding/merchant`,
      JSON.stringify({ merchant_name: "Load Test Merchant" }),
      { headers: authHeaders }
    );
    if (merchantRes.status !== 200) fail(`Merchant creation failed (${merchantRes.status}): ${merchantRes.body}`);
    const mb = JSON.parse(merchantRes.body);
    token = mb.token;
    merchantId = mb.merchant_id;
  }

  console.log(`[setup] merchant_id=${merchantId}  algorithm=${ALGORITHM}`);

  if (ALGORITHM === "RULE_BASED_ROUTING") {
    _ensureRoutingRule(baseUrl, token, merchantId, jsonHeaders);
  }

  return { token, merchantId };
}

// Creates and activates the priority routing rule if not already active.
// Idempotent: tries to activate an existing rule by name before creating a new one.
function _ensureRoutingRule(baseUrl, token, merchantId, jsonHeaders) {
  const authHeaders = { ...jsonHeaders, Authorization: `Bearer ${token}` };

  // Check if a rule named "load-test-priority" already exists
  const listRes = http.get(`${baseUrl}/routing/list?limit=20`, { headers: authHeaders });
  if (listRes.status === 200) {
    try {
      const body = JSON.parse(listRes.body);
      const records = Array.isArray(body) ? body : (body.data || body.records || []);
      const existing = records.find(r => r.name === PRIORITY_RULE.name);
      if (existing) {
        // Rule exists — activate it
        const activateRes = http.post(
          `${baseUrl}/routing/activate`,
          JSON.stringify({ routing_algorithm_id: existing.rule_id, created_by: merchantId }),
          { headers: authHeaders }
        );
        if (activateRes.status === 200) {
          console.log(`[setup] activated existing rule ${existing.rule_id}`);
          return;
        }
      }
    } catch (_) {}
  }

  // Create the rule
  const createBody = { ...PRIORITY_RULE, created_by: merchantId };
  const createRes = http.post(
    `${baseUrl}/routing/create`,
    JSON.stringify(createBody),
    { headers: authHeaders }
  );
  if (createRes.status !== 200) {
    fail(`Routing rule creation failed (${createRes.status}): ${createRes.body}`);
  }
  const { rule_id } = JSON.parse(createRes.body);

  // Activate it
  const activateRes = http.post(
    `${baseUrl}/routing/activate`,
    JSON.stringify({ routing_algorithm_id: rule_id, created_by: merchantId }),
    { headers: authHeaders }
  );
  if (activateRes.status !== 200) {
    fail(`Routing rule activation failed (${activateRes.status}): ${activateRes.body}`);
  }
  console.log(`[setup] created and activated routing rule ${rule_id}`);
}

// ── Main VU function ──────────────────────────────────────────────────────────

export default function (data) {
  const { token, merchantId } = data;

  const headers = {
    "Content-Type": "application/json",
    Accept: "*/*",
    Authorization: `Bearer ${token}`,
    "x-feature": "decision-engine",
    "x-tenant-id": "public",
  };

  const idx = __VU % 3;

  if (ALGORITHM === "RULE_BASED_ROUTING") {
    _runRuleEvaluate(merchantId, headers, idx);
  } else {
    _runSrDecide(merchantId, headers, idx);
  }

  sleep(0.1);
}

function _runSrDecide(merchantId, headers, idx) {
  const variant   = srPayloads[idx];
  const paymentId = `lt_sr_${Date.now()}_${__VU}_${__ITER}`;

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

  decideReqs.add(1);
  const start = Date.now();
  const res   = http.post(
    `${envDefaults.baseUrl}/decide-gateway`,
    body,
    { headers, tags: { name: "decide" } }
  );
  latencyTrend.add(Date.now() - start);

  const ok = check(res, {
    "status is 200":      (r) => r.status === 200,
    "has decided_gateway": (r) => {
      try { const b = JSON.parse(r.body); return !!(b.decided_gateway || b.decidedGateway); }
      catch { return false; }
    },
  });
  successRate.add(ok);

  if (res.status === 200) {
    try {
      const json = JSON.parse(res.body);
      const gw = json.decided_gateway || json.decidedGateway;
      if (gw) {
        gatewaySelected.add(1, { gateway: gw });
        // 85% CHARGED / 15% FAILURE — realistic SR feedback distribution
        const status = Math.random() < 0.85 ? "CHARGED" : "FAILURE";
        const fbBody = JSON.stringify({
          merchantId,
          gateway:   gw,
          paymentId,
          status,
        });
        const fbStart = Date.now();
        const fbRes = http.post(
          `${envDefaults.baseUrl}/update-gateway-score`,
          fbBody,
          { headers, tags: { name: "feedback" }, responseCallback: http.expectedStatuses({ min: 200, max: 299 }) }
        );
        feedbackLatency.add(Date.now() - fbStart);
        if (fbRes.status < 200 || fbRes.status >= 300) {
          feedbackErrors.add(1);
          if (__ENV.VERBOSE) console.error(`[VU ${__VU}] feedback ${fbRes.status}: ${fbRes.body?.substring(0, 200)}`);
        }
      }
      const podMs = json.latency;
      if (typeof podMs === "number" && podMs >= 0) {
        serverLatency.add(podMs);
        networkLatency.add(Math.max(0, res.timings.duration - podMs));
      }
    } catch (_) { gatewayErrors.add(1); }
  } else {
    gatewayErrors.add(1);
    if (__ENV.VERBOSE) console.error(`[VU ${__VU}] SR ${res.status}: ${res.body?.substring(0, 200)}`);
  }
}

function _runRuleEvaluate(merchantId, headers, idx) {
  const variant   = rulePayloads[idx];
  const paymentId = `lt_rule_${Date.now()}_${__VU}_${__ITER}`;

  const body = JSON.stringify({
    payment_id:  paymentId,
    created_by:  merchantId,
    parameters: {
      amount:              { type: "number",       value: variant.amount              },
      authentication_type: { type: "enum_variant", value: variant.authentication_type },
    },
  });

  const start = Date.now();
  const res   = http.post(`${envDefaults.baseUrl}/routing/evaluate`, body, { headers });
  latencyTrend.add(Date.now() - start);

  const ok = check(res, {
    "status is 200":     (r) => r.status === 200,
    "has priority list": (r) => {
      try { const b = JSON.parse(r.body); return Array.isArray(b.output?.connectors) && b.output.connectors.length > 0; }
      catch { return false; }
    },
    "rule applied (gws not empty)": (r) => {
      try { const b = JSON.parse(r.body); return b.status === "success"; }
      catch { return false; }
    },
  });
  successRate.add(ok);

  if (res.status === 200) {
    try {
      const json = JSON.parse(res.body);
      const topGw = json.output?.connectors?.[0]?.gateway_name;
      if (topGw) gatewaySelected.add(1, { gateway: topGw });
    } catch (_) { gatewayErrors.add(1); }
  } else {
    gatewayErrors.add(1);
    if (__ENV.VERBOSE) console.error(`[VU ${__VU}] RULE ${res.status}: ${res.body?.substring(0, 200)}`);
  }
}

// ── Summary ───────────────────────────────────────────────────────────────────

export function handleSummary(data) {
  const m = data.metrics;

  // Use decide-only counters so feedback calls don't skew throughput / error rate
  const decideCount = m.decide_reqs?.values?.count ?? m.http_reqs?.values?.count ?? 0;
  const testDuration = (data.state?.testRunDurationMs ?? 0) / 1000 || 1;
  const rps     = (decideCount / testDuration).toFixed(1);
  const avg     = m.decide_gateway_latency?.values?.avg?.toFixed(1)       ?? "N/A";
  const p50     = m.decide_gateway_latency?.values?.med?.toFixed(1)       ?? "N/A";
  const p95     = m.decide_gateway_latency?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const p99     = m.decide_gateway_latency?.values?.["p(99)"]?.toFixed(1) ?? "N/A";
  const maxLat  = m.decide_gateway_latency?.values?.max?.toFixed(1)       ?? "N/A";
  const errRate = ((m.http_req_failed?.values?.rate ?? 0) * 100).toFixed(2);
  const total   = decideCount;
  const success = m.decide_gateway_selected?.values?.count ?? "N/A";
  const fbErrCount = m.feedback_errors?.values?.count ?? 0;

  const svrAvg = m.server_latency_ms?.values?.avg?.toFixed(1)        ?? "N/A";
  const svrP50 = m.server_latency_ms?.values?.med?.toFixed(1)        ?? "N/A";
  const svrP95 = m.server_latency_ms?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const netAvg = m.network_latency_ms?.values?.avg?.toFixed(1)       ?? "N/A";
  const netP50 = m.network_latency_ms?.values?.med?.toFixed(1)       ?? "N/A";
  const netP95 = m.network_latency_ms?.values?.["p(95)"]?.toFixed(1) ?? "N/A";
  const hasBreakdown = m.server_latency_ms !== undefined;

  const fbAvg = m.feedback_latency_ms?.values?.avg?.toFixed(1)        ?? null;
  const fbP95 = m.feedback_latency_ms?.values?.["p(95)"]?.toFixed(1) ?? null;

  const latencyBreakdown = hasBreakdown ? `
╠══════════════════════════════════════════════════════════╣
║  Server-side latency (pod, ms)                           ║
║    avg             : ${String(svrAvg).padEnd(36)}║
║    p50             : ${String(svrP50).padEnd(36)}║
║    p95             : ${String(svrP95).padEnd(36)}║
╠══════════════════════════════════════════════════════════╣
║  Network overhead  (round-trip − pod, ms)                ║
║    avg             : ${String(netAvg).padEnd(36)}║
║    p50             : ${String(netP50).padEnd(36)}║
║    p95             : ${String(netP95).padEnd(36)}║${fbAvg !== null ? `
╠══════════════════════════════════════════════════════════╣
║  Feedback latency  (/update-gateway-score, ms)           ║
║    avg             : ${String(fbAvg).padEnd(36)}║
║    p95             : ${String(fbP95).padEnd(36)}║` : ""}` : "";

  const endpoint = ALGORITHM === "RULE_BASED_ROUTING" ? "/routing/evaluate" : "/decide-gateway";

  const summary = `
╔══════════════════════════════════════════════════════════╗
║  Load Test Results                                       ║
║  Algorithm  : ${ALGORITHM.padEnd(44)}║
║  Endpoint   : ${endpoint.padEnd(44)}║
║  Environment: ${ENV.padEnd(44)}║
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
║  Successful Routes : ${String(success).padEnd(36)}║${fbErrCount > 0 ? `
║  Feedback Errors   : ${String(fbErrCount).padEnd(36)}║` : ""}
╚══════════════════════════════════════════════════════════╝
`;

  return {
    stdout: summary,
    [`scripts/load-test/load_test_results_${ALGORITHM}.json`]: JSON.stringify(data, null, 2),
  };
}
