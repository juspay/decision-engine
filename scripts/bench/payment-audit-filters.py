#!/usr/bin/env python3
"""Benchmarks the Decision Audit list's filters against a running server and its ClickHouse.

For each window and filter, it calls GET /analytics/payment-audit once to capture the exact
queries the server sends (from system.query_log), then replays each query RUNS times:

  * uncapped, as the local server runs them;
  * under the sandbox profile (SANDBOX_SETTINGS): a 50 MB memory cap, 2 threads, and spilling
    GROUP BY / ORDER BY to disk past 20 MB. The sandbox user is readonly, so the app cannot
    send settings of its own; these are the profile's.

Every replay runs with the query condition cache off and the mark / uncompressed caches dropped
first, so a repeat cannot skip work the previous run did. The OS page cache stays warm.

A page load's time is the sum of its queries' medians (and p95s); memory is the largest query's
peak; "scanned" is the merchant's rows in the partitions the window touches.

  API_KEY=DE_... MERCHANT_ID=bench_audit scripts/bench/payment-audit-filters.py

Optional env: BASE_URL, END_MS (default: the merchant's last event), WINDOWS ("15m 1h 1d 7d"),
RUNS (5), PROFILES ("uncapped sandbox"), CASES (indices into CASES), CLICKHOUSE (the
clickhouse-client command), and
REPLAY_RUN (a run id printed by an earlier run): replay that run's captured queries instead of
calling the server, to compare an earlier build with this one on the same data, and
REPLAY_MERCHANT (the merchant that run used, when it differs from MERCHANT_ID).
"""
import json
import os
import shlex
import statistics
import subprocess
import sys
import time
import urllib.parse
import urllib.request

BASE_URL = os.environ.get("BASE_URL", "http://localhost:8080")
API_KEY = os.environ["API_KEY"]
MERCHANT_ID = os.environ["MERCHANT_ID"]
RUNS = int(os.environ.get("RUNS", "5"))
REPLAY_RUN = os.environ.get("REPLAY_RUN")
REPLAY_MERCHANT = os.environ.get("REPLAY_MERCHANT")
# Which of CASES to run, by index (default: all), e.g. "0 1 4".
ONLY_CASES = {int(index) for index in os.environ.get("CASES", "").split()}
WINDOWS = os.environ.get("WINDOWS", "15m 1h 1d 7d").split()
PROFILES = os.environ.get("PROFILES", "uncapped sandbox").split()
CLICKHOUSE = shlex.split(
    os.environ.get("CLICKHOUSE", "docker exec -i decision-engine-clickhouse-1 clickhouse-client")
)

SANDBOX_SETTINGS = {
    "max_memory_usage": 50_000_000,
    "max_threads": 2,
    "max_bytes_before_external_group_by": 20_000_000,
    "max_bytes_before_external_sort": 20_000_000,
}
NO_CACHE = {"use_query_condition_cache": 0}
UNITS = {"m": 60_000, "h": 3_600_000, "d": 86_400_000}

CASES = [
    ("no filter", {}),
    ("currency=USD (attribute filter)", {"currency": "USD"}),
    ("card_bin, rare (~0.1% of payments)", {"payment_fields": {"card_bin": "407363"}}),
    ("card_bin, top BIN (~20%)", {"payment_fields": {"card_bin": "400000"}}),
    ("metadata.segment (~25%)", {"payment_fields": {"metadata.segment": "premium"}}),
    ("card_bin + segment", {"payment_fields": {"card_bin": "400000", "metadata.segment": "premium"}}),
]


def ch(query, settings=None, check=True):
    """Runs a query; its text output. A replayed query's own failure is read from the query log."""
    args = CLICKHOUSE + [f"--{name}={value}" for name, value in (settings or {}).items()]
    result = subprocess.run(args + ["-q", query], capture_output=True)
    if check and result.returncode != 0:
        raise RuntimeError(result.stderr.decode(errors="replace").strip())
    return result.stdout.decode(errors="replace")


def scalar(query):
    return ch(query).strip()


def query_log(where):
    ch("SYSTEM FLUSH LOGS")
    rows = ch(
        "SELECT query, query_duration_ms, memory_usage, read_rows, read_bytes, exception_code "
        f"FROM system.query_log WHERE {where} AND type != 'QueryStart' "
        "ORDER BY event_time_microseconds FORMAT JSONEachRow"
    )
    return [json.loads(line) for line in rows.splitlines() if line]


def call_api(params):
    query = dict(params)
    if "payment_fields" in query:
        query["payment_fields"] = json.dumps(query["payment_fields"])
    url = f"{BASE_URL}/analytics/payment-audit?{urllib.parse.urlencode(query)}"
    request = urllib.request.Request(url, headers={"x-api-key": API_KEY, "x-tenant-id": "public"})
    started = time.monotonic()
    try:
        with urllib.request.urlopen(request) as response:
            response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        status = error.code
    return status, (time.monotonic() - started) * 1000


def capture(params):
    """The server's queries for one call, in order."""
    since = scalar("SELECT now64(6)")
    status, wall_ms = call_api(params)
    queries = [
        row["query"]
        for row in query_log(
            f"event_time_microseconds >= '{since}' AND query LIKE '%analytics_%' "
            "AND query NOT LIKE '%system.query_log%' AND type = 'QueryFinish'"
        )
    ]
    return status, wall_ms, queries


def captured(run_id, window, case_index):
    """An earlier run's queries for one case, in order: the first replay of each, under whichever
    profile that run used."""
    rows = query_log(
        f"log_comment LIKE 'bench-{run_id}-{window}-{case_index}-%-%-0' AND type != 'QueryStart'"
    )
    by_index = {}
    for row in rows:
        query = row["query"]
        if REPLAY_MERCHANT:
            query = query.replace(f"'{REPLAY_MERCHANT}'", f"'{MERCHANT_ID}'")
        by_index.setdefault(query, None)
    return list(by_index)


def replay(query, settings, tag):
    """One run of a captured query; its cost from the query log."""
    ch("SYSTEM DROP MARK CACHE")
    ch("SYSTEM DROP UNCOMPRESSED CACHE")
    # The captured text ends in the client's binary output format; the result is discarded.
    ch(query, {**settings, **NO_CACHE, "log_comment": tag}, check=False)
    rows = query_log(f"log_comment = '{tag}' AND event_date >= yesterday()")
    return rows[-1] if rows else None


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(fraction * (len(ordered) - 1)))]


def size(value):
    for unit in ["B", "KiB", "MiB", "GiB"]:
        if abs(value) < 1024 or unit == "GiB":
            return f"{value:.0f} {unit}" if unit == "B" else f"{value:.1f} {unit}"
        value /= 1024


def main():
    end_ms = int(
        os.environ.get("END_MS")
        or scalar(f"SELECT max(created_at_ms) FROM analytics_domain_events WHERE merchant_id = '{MERCHANT_ID}'")
    ) + 1
    run_id = int(time.time())
    print(f"run {run_id}" + (f", replaying run {REPLAY_RUN}" if REPLAY_RUN else ""))
    header = f"{'window':>6} {'events':>9} {'scanned':>9}  {'case':<36} {'profile':<9} {'api ms':>7} {'p50 ms':>7} {'p95 ms':>7} {'peak mem':>10} {'rows read':>10} {'read':>10}  result"
    print(header)
    for window in WINDOWS:
        start_ms = end_ms - int(window[:-1]) * UNITS[window[-1]]
        in_window = f"merchant_id = '{MERCHANT_ID}' AND created_at_ms BETWEEN {start_ms} AND {end_ms}"
        events = scalar(f"SELECT count() FROM analytics_domain_events WHERE {in_window}")
        scanned = scalar(
            "SELECT count() FROM analytics_domain_events WHERE merchant_id = "
            f"'{MERCHANT_ID}' AND _partition_id IN (SELECT DISTINCT _partition_id "
            f"FROM analytics_domain_events WHERE {in_window})"
        )
        for case_index, (name, filters) in enumerate(CASES):
            if ONLY_CASES and case_index not in ONLY_CASES:
                continue
            params = {"range": "1h", "start_ms": start_ms, "end_ms": end_ms, "page": 1, "page_size": 20, **filters}
            if REPLAY_RUN:
                status, wall_ms, queries = 200, 0, captured(REPLAY_RUN, window, case_index)
            else:
                status, wall_ms, queries = capture(params)
            for profile in PROFILES:
                settings = SANDBOX_SETTINGS if profile == "sandbox" else {}
                p50 = p95 = peak = rows_read = bytes_read = 0
                failures = set()
                for index, query in enumerate(queries):
                    runs = [
                        replay(query, settings, f"bench-{run_id}-{window}-{case_index}-{profile}-{index}-{run}")
                        for run in range(RUNS)
                    ]
                    runs = [run for run in runs if run]
                    failed = [run for run in runs if run["exception_code"]]
                    failures.update(str(run["exception_code"]) for run in failed)
                    durations = [run["query_duration_ms"] for run in runs]
                    p50 += statistics.median(durations)
                    p95 += percentile(durations, 0.95)
                    peak = max([peak] + [run["memory_usage"] for run in runs])
                    rows_read += statistics.median(run["read_rows"] for run in runs)
                    bytes_read += statistics.median(run["read_bytes"] for run in runs)
                result = "ok" if status == 200 and not failures else (
                    f"HTTP {status}" if status != 200 else "fails: code " + ",".join(sorted(failures))
                )
                print(
                    f"{window:>6} {events:>9} {scanned:>9}  {name:<36} {profile:<9} {wall_ms:>7.0f} {p50:>7.0f} "
                    f"{p95:>7.0f} {size(peak):>10} {rows_read:>10.0f} {size(bytes_read):>10}  {result}",
                    flush=True,
                )


if __name__ == "__main__":
    sys.exit(main())
