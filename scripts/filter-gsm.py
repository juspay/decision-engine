#!/usr/bin/env python3
"""Remove GSM rows whose connector,flow prefix doesn't match the allowed set."""

import csv
import re
import sys

PATTERN = re.compile(r'^(stripe|adyen|cybersource),(Payment,)?(Authorize)')

input_path = sys.argv[1] if len(sys.argv) > 1 else "crates/gsm/data/gsm.csv"
output_path = sys.argv[2] if len(sys.argv) > 2 else input_path

with open(input_path, newline="") as f:
    reader = csv.reader(f)
    header = next(reader)
    rows = [r for r in reader if PATTERN.match(f"{r[0]},{r[1]},{r[2]}")]

with open(output_path, "w", newline="") as f:
    writer = csv.writer(f, quoting=csv.QUOTE_MINIMAL)
    writer.writerow(header)
    writer.writerows(rows)

print(f"Kept {len(rows)} rows → {output_path}")
