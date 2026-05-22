/**
 * k6 load test with detailed HTML report generation
 *
 * Usage:
 *   # Localhost  (generate a token first: bash scripts/gen_local_token.sh)
 *   k6 run scripts/load_test_report.js -e TOKEN=$(bash scripts/gen_local_token.sh)
 *
 *   # Sandbox
 *   k6 run scripts/load_test_report.js -e ENV=sandbox -e TOKEN=<your_jwt> -e MERCHANT_ID=<merchant_id>
 *
 *   # Custom load
 *   k6 run scripts/load_test_report.js -e ENV=sandbox -e TOKEN=<your_jwt> -e MERCHANT_ID=<merchant_id> -e VUS=12 -e DURATION=60s
 *
 * Never commit TOKEN values. Pass them via shell env or a secrets manager.
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend, Counter, Gauge } from "k6/metrics";
import { textSummary } from "https://jslib.k6.io/k6-summary/0.0.2/index.js";

// ── Config ────────────────────────────────────────────────────────────────────

const ENV = __ENV.ENV || "local";
const VUS = parseInt(__ENV.VUS || (ENV === "sandbox" ? "12" : "20"));
const DURATION = __ENV.DURATION || "60s";
const RAMP_DURATION = __ENV.RAMP_DURATION || "10s";

const token = __ENV.TOKEN
if (!token) {
  throw new Error(
    'TOKEN is required.\n' +
    '  Local:   k6 run scripts/load_test_report.js -e TOKEN=$(bash scripts/gen_local_token.sh)\n' +
    '  Sandbox: k6 run scripts/load_test_report.js -e ENV=sandbox -e TOKEN=<your_jwt> -e MERCHANT_ID=<id>'
  )
}

const ENVS = {
  local: {
    baseUrl: "http://127.0.0.1:8080",
    token,
    merchantId: __ENV.MERCHANT_ID || "merchant_baea25c53626",
  },
  sandbox: {
    baseUrl: "https://app.hyperswitch.io/decision-engine/api",
    token,
    merchantId: __ENV.MERCHANT_ID || fail('MERCHANT_ID is required for sandbox'),
  },
};

function fail(msg) { throw new Error(msg) }

const config = ENVS[ENV];
if (!config) throw new Error(`Unknown ENV="${ENV}". Use "local" or "sandbox".`);

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
  },
};

// ── Custom metrics ────────────────────────────────────────────────────────────

const successRate = new Rate("success_rate");
const gatewayErrors = new Counter("gateway_errors");
const gatewaySelected = new Counter("gateway_selected");

// Track per-gateway routing counts
const gwStripe = new Counter("gw_stripe");
const gwAdyen = new Counter("gw_adyen");
const gwOther = new Counter("gw_other");

const payloads = [
  { method: "DEBIT", auth: "THREE_DS", brand: "VISA", currency: "AED", amount: 1000, gateways: ["stripe", "adyen"] },
  { method: "CREDIT", auth: "NO_THREE_DS", brand: "MASTERCARD", currency: "USD", amount: 5000, gateways: ["stripe", "adyen", "checkout"] },
  { method: "DEBIT", auth: "THREE_DS", brand: "AMEX", currency: "EUR", amount: 2500, gateways: ["adyen", "braintree"] },
];

const headers = {
  "Content-Type": "application/json",
  Accept: "*/*",
  Authorization: `Bearer ${config.token}`,
  "x-feature": "decision-engine",
  "x-tenant-id": "public",
};

export default function () {
  const variant = payloads[__VU % payloads.length];
  const body = JSON.stringify({
    merchantId: config.merchantId,
    paymentInfo: {
      paymentId: `lt_${Date.now()}_${__VU}_${__ITER}`,
      amount: variant.amount,
      currency: variant.currency,
      paymentType: "ORDER_PAYMENT",
      paymentMethodType: "CARD",
      paymentMethod: variant.method,
      authType: variant.auth,
      cardBrand: variant.brand,
    },
    eligibleGatewayList: variant.gateways,
    rankingAlgorithm: "SR_BASED_ROUTING",
    eliminationEnabled: false,
  });

  const res = http.post(`${config.baseUrl}/decide-gateway`, body, { headers });

  const ok = check(res, {
    "status 200": (r) => r.status === 200,
    "has gateway": (r) => {
      try {
        const b = JSON.parse(r.body);
        return !!(b.decided_gateway || b.decidedGateway);
      } catch { return false; }
    },
  });

  successRate.add(ok);

  if (res.status === 200) {
    try {
      const json = JSON.parse(res.body);
      const gw = json.decided_gateway || json.decidedGateway || "";
      gatewaySelected.add(1);
      if (gw === "stripe") gwStripe.add(1);
      else if (gw === "adyen") gwAdyen.add(1);
      else gwOther.add(1);
    } catch (_) { gatewayErrors.add(1); }
  } else {
    gatewayErrors.add(1);
  }

  sleep(0.1);
}

// ── HTML Report ───────────────────────────────────────────────────────────────

function statusBadge(value, warn, crit, unit = "ms", lowerIsBetter = true) {
  const n = parseFloat(value);
  if (isNaN(n)) return `<span class="badge badge-unknown">N/A</span>`;
  const bad = lowerIsBetter ? n > crit : n < crit;
  const caution = lowerIsBetter ? n > warn : n < warn;
  const cls = bad ? "badge-bad" : caution ? "badge-warn" : "badge-good";
  return `<span class="badge ${cls}">${value}${unit}</span>`;
}

function passFailBadge(passed) {
  return passed
    ? `<span class="badge badge-good">PASS</span>`
    : `<span class="badge badge-bad">FAIL</span>`;
}

export function handleSummary(data) {
  const m = data.metrics;
  const now = new Date().toISOString();

  const totalReqs   = m.http_reqs?.values?.count ?? 0;
  const rps         = (m.http_reqs?.values?.rate ?? 0).toFixed(1);
  const avg         = (m.http_req_duration?.values?.avg ?? 0).toFixed(1);
  const med         = (m.http_req_duration?.values?.med ?? 0).toFixed(1);
  const p90         = (m.http_req_duration?.values?.["p(90)"] ?? 0).toFixed(1);
  const p95         = (m.http_req_duration?.values?.["p(95)"] ?? 0).toFixed(1);
  const p99         = (m.http_req_duration?.values?.["p(99)"] ?? 0).toFixed(1);
  const minLat      = (m.http_req_duration?.values?.min ?? 0).toFixed(1);
  const maxLat      = (m.http_req_duration?.values?.max ?? 0).toFixed(1);
  const errRate     = ((m.http_req_failed?.values?.rate ?? 0) * 100).toFixed(2);
  const connTime    = (m.http_req_connecting?.values?.avg ?? 0).toFixed(1);
  const waitTime    = (m.http_req_waiting?.values?.avg ?? 0).toFixed(1);
  const recvTime    = (m.http_req_receiving?.values?.avg ?? 0).toFixed(1);
  const sendTime    = (m.http_req_sending?.values?.avg ?? 0).toFixed(1);
  const successPct  = (100 - parseFloat(errRate)).toFixed(2);

  const gwStripeN   = m.gw_stripe?.values?.count ?? 0;
  const gwAdyenN    = m.gw_adyen?.values?.count ?? 0;
  const gwOtherN    = m.gw_other?.values?.count ?? 0;
  const gwTotal     = gwStripeN + gwAdyenN + gwOtherN || 1;

  const p95Pass     = parseFloat(p95) <= 500;
  const p99Pass     = parseFloat(p99) <= 1000;
  const errPass     = parseFloat(errRate) < 1;

  // Benchmark context
  const benchmarks = [
    { label: "p95 < 100ms (excellent)",  pass: parseFloat(p95) < 100 },
    { label: "p95 < 200ms (good)",       pass: parseFloat(p95) < 200 },
    { label: "p95 < 500ms (acceptable)", pass: parseFloat(p95) < 500 },
    { label: "Throughput > 100 req/s",   pass: parseFloat(rps) > 100 },
    { label: "Throughput > 500 req/s",   pass: parseFloat(rps) > 500 },
    { label: "Error rate < 1%",          pass: parseFloat(errRate) < 1 },
    { label: "Error rate < 0.1%",        pass: parseFloat(errRate) < 0.1 },
  ];

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Load Test Report — decide-gateway (${ENV})</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #0f1117; color: #e2e8f0; min-height: 100vh; }

  .header { background: linear-gradient(135deg, #1a1f2e 0%, #0f1117 100%); border-bottom: 1px solid #2d3748; padding: 32px 40px; }
  .header h1 { font-size: 28px; font-weight: 700; color: #fff; }
  .header .subtitle { margin-top: 6px; color: #718096; font-size: 14px; }
  .header .meta { display: flex; gap: 24px; margin-top: 16px; flex-wrap: wrap; }
  .meta-item { background: #1a2035; border: 1px solid #2d3748; border-radius: 8px; padding: 8px 16px; font-size: 13px; }
  .meta-item strong { color: #63b3ed; }

  .content { padding: 32px 40px; max-width: 1200px; }

  .section { margin-bottom: 40px; }
  .section-title { font-size: 18px; font-weight: 600; color: #a0aec0; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid #2d3748; }

  .grid-3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  .grid-4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; }
  .grid-2 { display: grid; grid-template-columns: repeat(2, 1fr); gap: 24px; }

  .card { background: #1a2035; border: 1px solid #2d3748; border-radius: 12px; padding: 20px; }
  .card-label { font-size: 12px; color: #718096; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 8px; }
  .card-value { font-size: 32px; font-weight: 700; color: #fff; }
  .card-sub { font-size: 13px; color: #718096; margin-top: 4px; }
  .card-value.green { color: #48bb78; }
  .card-value.yellow { color: #ed8936; }
  .card-value.red { color: #fc8181; }

  .badge { display: inline-block; padding: 3px 10px; border-radius: 12px; font-size: 12px; font-weight: 600; }
  .badge-good    { background: #1c4532; color: #48bb78; }
  .badge-warn    { background: #744210; color: #f6ad55; }
  .badge-bad     { background: #63171b; color: #fc8181; }
  .badge-unknown { background: #2d3748; color: #718096; }

  table { width: 100%; border-collapse: collapse; font-size: 14px; }
  th { background: #1a2035; color: #a0aec0; text-align: left; padding: 12px 16px; font-weight: 600; text-transform: uppercase; font-size: 11px; letter-spacing: 0.5px; }
  td { padding: 12px 16px; border-bottom: 1px solid #2d3748; }
  tr:last-child td { border-bottom: none; }
  tr:hover td { background: #1a2035; }

  .latency-bar-wrap { background: #2d3748; border-radius: 4px; height: 8px; width: 100%; margin-top: 6px; }
  .latency-bar { border-radius: 4px; height: 8px; background: linear-gradient(90deg, #48bb78, #ed8936); }

  .bench-row { display: flex; align-items: center; justify-content: space-between; padding: 10px 0; border-bottom: 1px solid #2d3748; font-size: 14px; }
  .bench-row:last-child { border-bottom: none; }

  .verdict-box { border-radius: 12px; padding: 24px; margin-top: 24px; }
  .verdict-box.good { background: #1c4532; border: 1px solid #2f855a; }
  .verdict-box.warn { background: #744210; border: 1px solid #c05621; }
  .verdict-box.bad  { background: #63171b; border: 1px solid #9b2335; }
  .verdict-box h3   { font-size: 18px; margin-bottom: 8px; }
  .verdict-box p    { font-size: 14px; line-height: 1.6; color: #e2e8f0; }

  .timeline { display: flex; align-items: center; gap: 8px; margin-top: 16px; font-size: 13px; }
  .timeline-phase { background: #2d3748; border-radius: 6px; padding: 6px 12px; }
  .timeline-arrow { color: #4a5568; }

  .donut-labels { display: flex; flex-direction: column; gap: 8px; margin-top: 16px; }
  .donut-label-row { display: flex; justify-content: space-between; align-items: center; font-size: 13px; }
  .donut-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; margin-right: 6px; }
  .bar-chart { display: flex; flex-direction: column; gap: 12px; }
  .bar-row { display: flex; align-items: center; gap: 12px; font-size: 13px; }
  .bar-label { width: 80px; color: #a0aec0; text-align: right; flex-shrink: 0; }
  .bar-outer { flex: 1; background: #2d3748; border-radius: 4px; height: 20px; overflow: hidden; }
  .bar-inner { height: 100%; border-radius: 4px; display: flex; align-items: center; padding-left: 8px; font-size: 11px; font-weight: 600; color: #fff; min-width: 30px; }
  .bar-val { width: 70px; text-align: left; color: #e2e8f0; }
</style>
</head>
<body>

<div class="header">
  <h1>Load Test Report — <code>decide-gateway</code></h1>
  <p class="subtitle">Endpoint: <strong>${config.baseUrl}/decide-gateway</strong></p>
  <div class="meta">
    <div class="meta-item"><strong>Environment</strong> ${ENV.toUpperCase()}</div>
    <div class="meta-item"><strong>VUs</strong> ${VUS}</div>
    <div class="meta-item"><strong>Duration</strong> ${DURATION} (+ ${RAMP_DURATION} ramp)</div>
    <div class="meta-item"><strong>Generated</strong> ${now}</div>
    <div class="meta-item"><strong>Algorithm</strong> SR_BASED_ROUTING</div>
  </div>
  <div class="timeline">
    <div class="timeline-phase">Ramp up (${RAMP_DURATION})</div>
    <span class="timeline-arrow">→</span>
    <div class="timeline-phase">Steady load ${VUS} VUs (${DURATION})</div>
    <span class="timeline-arrow">→</span>
    <div class="timeline-phase">Ramp down (5s)</div>
  </div>
</div>

<div class="content">

  <!-- KPI Cards -->
  <div class="section">
    <div class="section-title">Key Metrics</div>
    <div class="grid-4">
      <div class="card">
        <div class="card-label">Total Requests</div>
        <div class="card-value">${totalReqs.toLocaleString()}</div>
        <div class="card-sub">${rps} req/s throughput</div>
      </div>
      <div class="card">
        <div class="card-label">p95 Latency</div>
        <div class="card-value ${parseFloat(p95) < 200 ? 'green' : parseFloat(p95) < 500 ? 'yellow' : 'red'}">${p95}ms</div>
        <div class="card-sub">p99: ${p99}ms</div>
      </div>
      <div class="card">
        <div class="card-label">Error Rate</div>
        <div class="card-value ${parseFloat(errRate) < 1 ? 'green' : 'red'}">${errRate}%</div>
        <div class="card-sub">${successPct}% success</div>
      </div>
      <div class="card">
        <div class="card-label">Throughput</div>
        <div class="card-value ${parseFloat(rps) > 100 ? 'green' : parseFloat(rps) > 30 ? 'yellow' : 'red'}">${rps}</div>
        <div class="card-sub">requests / second</div>
      </div>
    </div>
  </div>

  <div class="grid-2">
    <!-- Latency Breakdown -->
    <div class="section">
      <div class="section-title">Latency Distribution</div>
      <div class="card">
        <div class="bar-chart">
          ${[
            ["min",  minLat, 0],
            ["avg",  avg,    parseFloat(avg)/parseFloat(maxLat)*100],
            ["p50",  med,    parseFloat(med)/parseFloat(maxLat)*100],
            ["p90",  p90,    parseFloat(p90)/parseFloat(maxLat)*100],
            ["p95",  p95,    parseFloat(p95)/parseFloat(maxLat)*100],
            ["p99",  p99,    parseFloat(p99)/parseFloat(maxLat)*100],
            ["max",  maxLat, 100],
          ].map(([label, val, pct]) => `
            <div class="bar-row">
              <div class="bar-label">${label}</div>
              <div class="bar-outer">
                <div class="bar-inner" style="width:${Math.max(pct,3)}%; background: ${parseFloat(val) < 200 ? '#2f855a' : parseFloat(val) < 500 ? '#c05621' : '#9b2335'}">
                  ${pct > 20 ? val + 'ms' : ''}
                </div>
              </div>
              <div class="bar-val">${val}ms</div>
            </div>
          `).join('')}
        </div>
      </div>
    </div>

    <!-- Request Timing Breakdown -->
    <div class="section">
      <div class="section-title">Request Timing Breakdown (avg)</div>
      <div class="card">
        <table>
          <tr><th>Phase</th><th>Duration</th><th>% of avg</th></tr>
          ${[
            ["DNS Lookup",   (m.http_req_tls_handshaking?.values?.avg ?? 0).toFixed(1), "#63b3ed"],
            ["Connecting",   connTime, "#9f7aea"],
            ["Waiting (TTFB)", waitTime, "#48bb78"],
            ["Receiving",    recvTime, "#ed8936"],
            ["Sending",      sendTime, "#fc8181"],
          ].map(([phase, val, color]) => {
            const pct = avg > 0 ? Math.round(parseFloat(val)/parseFloat(avg)*100) : 0;
            return `<tr>
              <td><span style="display:inline-block;width:10px;height:10px;background:${color};border-radius:50%;margin-right:6px;"></span>${phase}</td>
              <td>${val}ms</td>
              <td>
                <div class="latency-bar-wrap" style="width:80px">
                  <div class="latency-bar" style="width:${Math.max(pct,1)}%;background:${color}"></div>
                </div>
              </td>
            </tr>`;
          }).join('')}
          <tr style="font-weight:600"><td>Total avg</td><td>${avg}ms</td><td>100%</td></tr>
        </table>
      </div>
    </div>
  </div>

  <div class="grid-2">
    <!-- Threshold Results -->
    <div class="section">
      <div class="section-title">Threshold Results</div>
      <div class="card">
        <div class="bench-row"><span>p95 latency &lt; 500ms</span>${passFailBadge(p95Pass)}</div>
        <div class="bench-row"><span>p99 latency &lt; 1000ms</span>${passFailBadge(p99Pass)}</div>
        <div class="bench-row"><span>Error rate &lt; 1%</span>${passFailBadge(errPass)}</div>
      </div>
    </div>

    <!-- Production Benchmarks -->
    <div class="section">
      <div class="section-title">Production Benchmark Comparison</div>
      <div class="card">
        ${benchmarks.map(b => `
          <div class="bench-row">
            <span>${b.label}</span>
            ${b.pass
              ? `<span class="badge badge-good">✓ PASS</span>`
              : `<span class="badge badge-bad">✗ FAIL</span>`}
          </div>
        `).join('')}
      </div>
    </div>
  </div>

  <!-- Gateway Distribution -->
  <div class="section">
    <div class="section-title">Gateway Selection Distribution</div>
    <div class="card">
      <table>
        <tr><th>Gateway</th><th>Selected</th><th>Share</th><th>Distribution</th></tr>
        ${[
          ["stripe",  gwStripeN, "#63b3ed"],
          ["adyen",   gwAdyenN,  "#48bb78"],
          ["other",   gwOtherN,  "#9f7aea"],
        ].map(([gw, count, color]) => {
          const pct = ((count / gwTotal) * 100).toFixed(1);
          return `<tr>
            <td><strong>${gw}</strong></td>
            <td>${count}</td>
            <td>${pct}%</td>
            <td>
              <div class="latency-bar-wrap" style="width:200px">
                <div class="latency-bar" style="width:${pct}%;background:${color}"></div>
              </div>
            </td>
          </tr>`;
        }).join('')}
      </table>
    </div>
  </div>

  <!-- Full Metrics Table -->
  <div class="section">
    <div class="section-title">Full Metrics</div>
    <div class="card">
      <table>
        <tr><th>Metric</th><th>Value</th><th>Assessment</th></tr>
        <tr><td>Total requests</td><td>${totalReqs}</td><td>—</td></tr>
        <tr><td>Throughput</td><td>${rps} req/s</td><td>${statusBadge(rps, 100, 30, " req/s", false)}</td></tr>
        <tr><td>avg latency</td><td>${avg} ms</td><td>${statusBadge(avg, 200, 500)}</td></tr>
        <tr><td>p50 (median)</td><td>${med} ms</td><td>${statusBadge(med, 200, 500)}</td></tr>
        <tr><td>p90 latency</td><td>${p90} ms</td><td>${statusBadge(p90, 300, 800)}</td></tr>
        <tr><td>p95 latency</td><td>${p95} ms</td><td>${statusBadge(p95, 300, 500)}</td></tr>
        <tr><td>p99 latency</td><td>${p99} ms</td><td>${statusBadge(p99, 500, 1000)}</td></tr>
        <tr><td>max latency</td><td>${maxLat} ms</td><td>—</td></tr>
        <tr><td>min latency</td><td>${minLat} ms</td><td>—</td></tr>
        <tr><td>Avg connect time</td><td>${connTime} ms</td><td>—</td></tr>
        <tr><td>Avg TTFB (waiting)</td><td>${waitTime} ms</td><td>—</td></tr>
        <tr><td>Error rate</td><td>${errRate}%</td><td>${statusBadge(errRate, 0.1, 1, "%")}</td></tr>
        <tr><td>Virtual Users</td><td>${VUS}</td><td>—</td></tr>
      </table>
    </div>
  </div>

  <!-- Verdict -->
  <div class="section">
    <div class="section-title">Production Readiness Verdict</div>
    ${parseFloat(p95) < 200 && parseFloat(errRate) < 0.1 && parseFloat(rps) > 100
      ? `<div class="verdict-box good"><h3>✅ Production Ready</h3><p>All key metrics are within acceptable production thresholds. The service handles load gracefully with low latency and negligible error rate.</p></div>`
      : parseFloat(p95) < 500 && parseFloat(errRate) < 1
      ? `<div class="verdict-box warn"><h3>⚠️ Conditionally Ready</h3><p>Error rate is acceptable but latency (p95=${p95}ms) or throughput (${rps} req/s) does not meet production-grade targets for a high-volume payment routing service. Investigate infrastructure scaling before production deployment.</p></div>`
      : `<div class="verdict-box bad"><h3>🔴 Not Production Ready</h3><p>Critical thresholds breached. p95=${p95}ms, throughput=${rps} req/s. This environment requires significant infrastructure scaling (connection pooling, horizontal scaling, caching) before handling production payment volumes.</p></div>`
    }
  </div>

</div>
</body>
</html>`;

  const filename = `scripts/load_test_report_${ENV}_${VUS}vu.html`;

  return {
    stdout: textSummary(data, { indent: " ", enableColors: true }),
    [filename]: html,
    [`scripts/load_test_raw_${ENV}_${VUS}vu.json`]: JSON.stringify(data, null, 2),
  };
}
