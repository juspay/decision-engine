#!/usr/bin/env bash
# Unified load-test benchmark for local (container) and sandbox environments.
#
# Runs each algorithm (SR, PL — extensible) as a separate stepped VU run,
# then prints a side-by-side comparison table.
#
# Usage:
#   bash scripts/load-test/benchmark.sh                              # local, SR + PL
#   bash scripts/load-test/benchmark.sh --algos SR_BASED_ROUTING     # single algo
#   bash scripts/load-test/benchmark.sh --env sandbox -t <TOKEN> -m <MERCHANT_ID>
#   bash scripts/load-test/benchmark.sh --vus 1,5,10,20 --duration 20s
#   bash scripts/load-test/benchmark.sh --saturation                 # find throughput ceiling
#   bash scripts/load-test/benchmark.sh --saturation --max-vus 300 --step-dur 30s

set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────
ENV="local"
VUS="5,20,50"
DURATION="30s"
RAMP="10s"
TOKEN=""
MERCHANT_ID=""
ALGOS="SR_BASED_ROUTING,RULE_BASED_ROUTING"
CONTAINER="decision-engine-open-router-pg-ghcr-1"
REDIS_PORT="6379"
SATURATION_MODE=false
MAX_VUS_SAT=200
STEP_DUR_SAT=20s
RAMP_DUR_SAT=5s

# sandbox baseline (100m CPU provisioning — used for before/after diff)
SANDBOX_BASELINE='{
  "5":  {"rps":18.5,"rt_p95":200.3,"svr_avg":38.6,"svr_p95":108.8},
  "12": {"rps":22.5,"rt_p95":509.8,"svr_avg":226.0,"svr_p95":398.4},
  "20": {"rps":null,"rt_p95":null,"svr_avg":null,"svr_p95":null}
}'

while [[ $# -gt 0 ]]; do
  case $1 in
    --env|-e)      ENV="$2";             shift 2 ;;
    --vus)         VUS="$2";             shift 2 ;;
    --duration)    DURATION="$2";        shift 2 ;;
    --ramp)        RAMP="$2";            shift 2 ;;
    --algos)       ALGOS="$2";           shift 2 ;;
    -t|--token)    TOKEN="$2";           shift 2 ;;
    -m|--merchant) MERCHANT_ID="$2";     shift 2 ;;
    --saturation)  SATURATION_MODE=true; shift   ;;
    --max-vus)     MAX_VUS_SAT="$2";     shift 2 ;;
    --step-dur)    STEP_DUR_SAT="$2";    shift 2 ;;
    --ramp-dur)    RAMP_DUR_SAT="$2";    shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

# ── saturation mode — runs saturation_test.js and exits ──────────────────────
if [[ "$SATURATION_MODE" == "true" ]]; then
  SAT_OUT="scripts/load-test/profile_results/saturation_$(date +%Y%m%d_%H%M%S)"
  mkdir -p "$SAT_OUT"

  echo ""
  echo "════════════════════════════════════════════════════════════════"
  echo "  Saturation Test"
  echo "  max VUs=$MAX_VUS_SAT  step=$STEP_DUR_SAT  ramp=$RAMP_DUR_SAT"
  echo "  (no sleep — finds raw throughput ceiling + CPU saturation)"
  echo "════════════════════════════════════════════════════════════════"
  echo ""

  # Start CPU monitor: poll docker stats every ~1s (local only)
  SAT_CPU_FILE="$SAT_OUT/cpu_timeseries.csv"
  MON_CPU=""
  if [[ "$ENV" == "local" ]]; then
    echo "ts,cpu_pct" > "$SAT_CPU_FILE"
    (while true; do
      cpu=$(docker stats "$CONTAINER" --no-stream --format "{{.CPUPerc}}" 2>/dev/null | tr -d '%')
      [[ -n "$cpu" ]] && printf '%s,%s\n' "$(date +%s)" "$cpu"
    done) >> "$SAT_CPU_FILE" &
    MON_CPU=$!
  fi

  k6 run \
    -e MAX_VUS="$MAX_VUS_SAT" \
    -e STEP_DUR="$STEP_DUR_SAT" \
    -e RAMP_DUR="$RAMP_DUR_SAT" \
    --out "json=$SAT_OUT/k6_timeseries.json" \
    --summary-export "$SAT_OUT/saturation_summary.json" \
    scripts/load-test/saturation_test.js || true

  # Stop CPU monitor
  if [[ -n "$MON_CPU" ]]; then
    kill "$MON_CPU" 2>/dev/null || true
    wait "$MON_CPU" 2>/dev/null || true
  fi

  # Correlate per-step throughput (from k6 time-series) with CPU% (from docker stats)
  python3 - "$SAT_OUT" "$MAX_VUS_SAT" "$STEP_DUR_SAT" "$RAMP_DUR_SAT" <<'PYEOF'
import json, csv, sys, os
from collections import defaultdict
from datetime import datetime

sat_out, max_vus_str, step_dur_str, ramp_dur_str = sys.argv[1:]
max_vus = int(max_vus_str)

def parse_dur(s):
    s = s.strip()
    if s.endswith('s'): return int(s[:-1])
    if s.endswith('m'): return int(s[:-1]) * 60
    return int(s)

step_sec = parse_dur(step_dur_str)
ramp_sec = parse_dur(ramp_dur_str)

# Mirror saturation_test.js VU step logic
_DEFAULT = [5, 10, 20, 30, 50, 75, 100, 150, 200, 300, 400, 500]
vu_steps = [v for v in _DEFAULT if v <= max_vus]
if not vu_steps or vu_steps[-1] < max_vus:
    vu_steps.append(max_vus)

# Load k6 NDJSON: bucket http_reqs by second, find actual start from first request
req_by_sec = defaultdict(int)
actual_start = None
k6_path = os.path.join(sat_out, "k6_timeseries.json")
if os.path.exists(k6_path):
    with open(k6_path) as f:
        for line in f:
            try:
                obj = json.loads(line)
                if obj.get("type") == "Point" and obj.get("metric") == "http_reqs":
                    raw = obj["data"]["time"].replace("Z", "+00:00")
                    ts  = int(datetime.fromisoformat(raw).timestamp())
                    req_by_sec[ts] += 1
                    if actual_start is None or ts < actual_start:
                        actual_start = ts
            except:
                pass

if actual_start is None:
    print("  No k6 time-series data found.")
    sys.exit(0)

# Load docker CPU timeseries
cpu_by_sec = {}
cpu_path = os.path.join(sat_out, "cpu_timeseries.csv")
if os.path.exists(cpu_path):
    with open(cpu_path) as f:
        for row in csv.DictReader(f):
            try:
                cpu_by_sec[int(row["ts"])] = float(row["cpu_pct"])
            except:
                pass

# Compute hold-phase window for each step, relative to actual first request
windows = []
t = actual_start
for vu in vu_steps:
    windows.append((vu, t + ramp_sec, t + ramp_sec + step_sec))
    t += ramp_sec + step_sec

# Build rows
rows = []
peak_rps, peak_vu, prev_rps = 0, 0, None
for vu, w_start, w_end in windows:
    reqs = sum(req_by_sec[s] for s in range(w_start, w_end))
    rps  = reqs / step_sec
    cpu_samples = [cpu_by_sec[s] for s in range(w_start, w_end) if s in cpu_by_sec]
    cpu_avg = sum(cpu_samples) / len(cpu_samples) if cpu_samples else None
    if rps > peak_rps:
        peak_rps, peak_vu = rps, vu
    note = ""
    if prev_rps is not None and prev_rps > 0:
        pct = (rps - prev_rps) / prev_rps * 100
        if pct <= -10:
            note = "<-- saturation knee"
        elif cpu_avg is not None and cpu_avg >= 185:
            note = "<-- CPU pinned"
    rows.append((vu, rps, cpu_avg, note))
    prev_rps = rps

# Print
W = 62
print()
print("=" * W)
print("  Saturation Proof — Throughput + CPU% by VU Level")
print("=" * W)
cpu_hdr = "CPU%     " if cpu_by_sec else "CPU%     "
print(f"  {'VUs':<7} {'req/s':<9} {cpu_hdr} Note")
print(f"  {'-----':<7} {'-------':<9} {'-------':<9} -----")
for vu, rps, cpu_avg, note in rows:
    cpu_str = f"{cpu_avg:.1f}%" if cpu_avg is not None else "n/a"
    print(f"  {vu:<7} {rps:<9.0f} {cpu_str:<9} {note}")
print("=" * W)
print(f"  Peak: {peak_rps:.0f} req/s at {peak_vu} in-flight requests")
if cpu_by_sec:
    print("  CPU%: 200% = 2 cores fully utilized — throughput plateau")
    print("        = textbook CPU saturation")
else:
    print("  (re-run with --env local to capture Docker CPU stats)")
print("=" * W)
print()
PYEOF

  echo "  Results saved → $SAT_OUT/"
  echo "════════════════════════════════════════════════════════════════"
  exit 0
fi

OUT_DIR="scripts/load-test/profile_results/${ENV}_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUT_DIR"

# ── sandbox: validate token ───────────────────────────────────────────────────
if [[ "$ENV" == "sandbox" ]]; then
  [[ -z "$TOKEN" ]]       && { echo "ERROR: --token/-t is required for sandbox"; exit 1; }
  [[ -z "$MERCHANT_ID" ]] && { echo "ERROR: --merchant/-m is required for sandbox"; exit 1; }

  python3 - "$TOKEN" <<'PYEOF'
import sys, base64, json, time
tok = sys.argv[1]
part = tok.split('.')[1]
part += '=' * (4 - len(part) % 4)
exp = json.loads(base64.urlsafe_b64decode(part)).get('exp', 0)
now = int(time.time())
if now >= exp:
    print(f"ERROR: Token expired {(now-exp)//60} min ago. Pass a fresh token with -t <TOKEN>")
    sys.exit(1)
print(f"Token valid for {(exp-now)//60} min — OK")
PYEOF
fi

# ── local: start docker/redis monitors (run once across all algo runs) ────────
CPU_FILE="$OUT_DIR/cpu.txt"
THREAD_FILE="$OUT_DIR/threads.txt"
REDIS_FILE="$OUT_DIR/redis.txt"
STOP_FILE="$OUT_DIR/.stop"
rm -f "$STOP_FILE"

if [[ "$ENV" == "local" ]]; then
  (while [[ ! -f "$STOP_FILE" ]]; do
    docker stats "$CONTAINER" --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" 2>/dev/null
    sleep 0.5
  done) > "$CPU_FILE" &
  MON_CPU=$!

  (while [[ ! -f "$STOP_FILE" ]]; do
    THREADS=$(docker exec "$CONTAINER" sh -c "cat /proc/7/status 2>/dev/null | grep ^Threads | awk '{print \$2}'" 2>/dev/null || echo "?")
    echo "$(date +%H:%M:%S) threads=$THREADS"
    sleep 1
  done) > "$THREAD_FILE" &
  MON_THREAD_COUNT=$!

  (while [[ ! -f "$STOP_FILE" ]]; do
    OPS=$(redis-cli -p "$REDIS_PORT" INFO stats 2>/dev/null | grep instantaneous_ops_per_sec | grep -oE '[0-9]+' || echo "?")
    CONN=$(redis-cli -p "$REDIS_PORT" INFO clients 2>/dev/null | grep connected_clients: | grep -oE '[0-9]+' || echo "?")
    echo "$(date +%H:%M:%S) ops=$OPS conn=$CONN"
    sleep 1
  done) > "$REDIS_FILE" &
  MON_REDIS=$!
fi

# ── run each algorithm ────────────────────────────────────────────────────────
IFS=',' read -ra ALGO_LIST <<< "$ALGOS"
IFS=',' read -ra VU_LIST   <<< "$VUS"

for ALGO in "${ALGO_LIST[@]}"; do
  echo ""
  echo "════════════════════════════════════════════════════════════════"
  echo "  $ALGO — env=$ENV  vus=$VUS  duration=$DURATION"
  echo "════════════════════════════════════════════════════════════════"

  mkdir -p "$OUT_DIR/$ALGO"

  for VU in "${VU_LIST[@]}"; do
    echo ""
    echo "── ${VU} VUs ──────────────────────────────────────────────────"

    K6_ARGS="-e VUS=$VU -e DURATION=$DURATION -e RAMP_DURATION=$RAMP -e ALGORITHM=$ALGO"
    if [[ "$ENV" == "sandbox" ]]; then
      K6_ARGS="$K6_ARGS -e ENV=sandbox -e TOKEN=$TOKEN -e MERCHANT_ID=$MERCHANT_ID"
    fi

    # shellcheck disable=SC2086
    k6 run scripts/load-test/load_test.js $K6_ARGS 2>&1 | grep -E "╔|║|╠|╚" || true

    cp "scripts/load-test/load_test_results_${ALGO}.json" "$OUT_DIR/$ALGO/k6_${VU}vu.json" 2>/dev/null || true
    sleep 3
  done
done

# ── stop local monitors ───────────────────────────────────────────────────────
if [[ "$ENV" == "local" ]]; then
  touch "$STOP_FILE"
  wait $MON_CPU $MON_THREAD_COUNT $MON_REDIS 2>/dev/null || true
fi

# ── comparison summary ────────────────────────────────────────────────────────
python3 - "$OUT_DIR" "$ENV" "$VUS" "$ALGOS" "$SANDBOX_BASELINE" <<'PYEOF'
import json, sys, os

out_dir, env, vus_str, algos_str, baseline_json = sys.argv[1:]
vu_list   = [int(v) for v in vus_str.split(',')]
algo_list = algos_str.split(',')
sandbox_baseline = json.loads(baseline_json)

def parse_summary(path):
    with open(path) as f:
        d = json.load(f)
    m = d.get("metrics", {})
    def v(k, s): return m.get(k, {}).get("values", {}).get(s)
    return {
        "rps":     round(v("decide_reqs", "rate") or v("http_reqs", "rate") or 0, 1),
        "rt_avg":  round(v("http_req_duration", "avg") or 0, 1),
        "rt_p50":  round(v("http_req_duration", "med") or 0, 1),
        "rt_p95":  round(v("http_req_duration", "p(95)") or 0, 1),
        "svr_avg": round(v("server_latency_ms", "avg") or 0, 1),
        "svr_p50": round(v("server_latency_ms", "med") or 0, 1),
        "svr_p95": round(v("server_latency_ms", "p(95)") or 0, 1),
        "net_avg": round(v("network_latency_ms", "avg") or 0, 1),
        "net_p95": round(v("network_latency_ms", "p(95)") or 0, 1),
        "fb_avg":  round(v("feedback_latency_ms", "avg") or 0, 1),
        "fb_p95":  round(v("feedback_latency_ms", "p(95)") or 0, 1),
        "err_pct": round((v("http_req_failed", "rate") or 0) * 100, 2),
    }

results = {}
for algo in algo_list:
    results[algo] = {}
    for vu in vu_list:
        path = os.path.join(out_dir, algo, f"k6_{vu}vu.json")
        results[algo][vu] = parse_summary(path) if os.path.exists(path) else None

def fmt(v, unit=""):
    return f"{v}{unit}" if v is not None else "—"

def delta(b, a, higher_better=False):
    if b is None or a is None or b == 0: return ""
    pct = (a - b) / b * 100
    symbol = "+" if pct > 0 else ""
    marker = "✓" if (pct > 0) == higher_better else "↑" if not higher_better else "↓"
    return f"{marker} {symbol}{pct:.0f}%"

# ── local: resource summary ───────────────────────────────────────────────────
if env == "local":
    cpus, redis_ops, threads_list = [], [], []
    for fname, lst, parser in [
        ("cpu.txt",     cpus,       lambda l: float(l.split()[0].replace('%',''))),
        ("redis.txt",   redis_ops,  lambda l: int(l.split('ops=')[1].split()[0])),
        ("threads.txt", threads_list, lambda l: int(l.split('threads=')[1])),
    ]:
        fpath = os.path.join(out_dir, fname)
        if os.path.exists(fpath):
            for line in open(fpath):
                try: lst.append(parser(line))
                except: pass

    print()
    print("── Container Resource Usage (across all algorithm runs) ──────")
    if cpus:       print(f"  CPU      avg={sum(cpus)/len(cpus):.1f}%  peak={max(cpus):.1f}%  (200% = 2 cores)")
    if threads_list: print(f"  Threads  peak={max(threads_list)}")
    if redis_ops:  print(f"  Redis    avg={sum(redis_ops)/len(redis_ops):.0f} ops/s  peak={max(redis_ops):.0f} ops/s")

# ── per-VU comparison table ───────────────────────────────────────────────────
print()
print("════════════════════════════════════════════════════════════════════════")
print("  Results by Algorithm")
print("════════════════════════════════════════════════════════════════════════")

for vu in vu_list:
    print(f"\n  {vu} VUs")
    header = f"  {'Metric':<20}"
    divider = f"  {'-'*20}"
    rows = {
        "Throughput":    [],
        "RT avg":        [],
        "RT p50":        [],
        "RT p95":        [],
        "Server avg":    [],
        "Server p95":    [],
        "Network avg":   [],
        "Network p95":   [],
        "Feedback avg":  [],
        "Feedback p95":  [],
        "Error rate":    [],
    }

    for algo in algo_list:
        r = results[algo].get(vu)
        short = algo.replace("_BASED_ROUTING","").replace("_ROUTING","")
        header  += f"  {short:<16}"
        divider += f"  {'-'*16}"
        if r:
            p95_flag = " ❌" if r["rt_p95"] > 500 else ""
            rows["Throughput"].append(fmt(r["rps"], "req/s"))
            rows["RT avg"].append(fmt(r["rt_avg"], "ms"))
            rows["RT p50"].append(fmt(r["rt_p50"], "ms"))
            rows["RT p95"].append(fmt(r["rt_p95"], "ms") + p95_flag)
            rows["Server avg"].append(fmt(r["svr_avg"], "ms"))
            rows["Server p95"].append(fmt(r["svr_p95"], "ms"))
            rows["Network avg"].append(fmt(r["net_avg"], "ms"))
            rows["Network p95"].append(fmt(r["net_p95"], "ms"))
            rows["Feedback avg"].append(fmt(r["fb_avg"] or None, "ms") if r.get("fb_avg") else "—")
            rows["Feedback p95"].append(fmt(r["fb_p95"] or None, "ms") if r.get("fb_p95") else "—")
            rows["Error rate"].append(fmt(r["err_pct"], "%"))
        else:
            for k in rows: rows[k].append("—")

    print(header)
    print(divider)
    for label, vals in rows.items():
        row = f"  {label:<20}"
        for v in vals:
            row += f"  {v:<16}"
        print(row)

# ── algo comparison delta (first algo as baseline) ────────────────────────────
if len(algo_list) > 1:
    print()
    print("── Algorithm delta (vs first algo as baseline) ──────────────")
    base_algo = algo_list[0]
    for algo in algo_list[1:]:
        short = algo.replace("_BASED_ROUTING","").replace("_ROUTING","")
        base_short = base_algo.replace("_BASED_ROUTING","").replace("_ROUTING","")
        print(f"\n  {short} vs {base_short}:")
        for vu in vu_list:
            b = results[base_algo].get(vu)
            a = results[algo].get(vu)
            if b and a:
                rps_d   = delta(b["rps"],     a["rps"],     True)
                p95_d   = delta(b["rt_p95"],  a["rt_p95"],  False)
                svr_d   = delta(b["svr_avg"], a["svr_avg"], False)
                print(f"    {vu:3} VUs  throughput {rps_d:<12}  rt_p95 {p95_d:<12}  server_avg {svr_d}")

print()
print(f"  Results saved → {out_dir}/")
print("════════════════════════════════════════════════════════════════════════")
PYEOF
