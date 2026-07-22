#!/usr/bin/env python3
"""Check llvm-cov JSON against repository coverage thresholds."""
from __future__ import annotations
import json, sys
from pathlib import Path

def main(path: str) -> int:
    data = json.loads(Path(path).read_text())
    files = data.get("data", [{}])[0].get("files", [])
    covered = total = 0
    for item in files:
        name = item.get("filename", "").replace("\\", "/")
        if "/tests/" in name or "/examples/" in name or "/benches/" in name or "/fuzz/" in name:
            continue
        summary = item.get("summary", {}).get("lines", {})
        covered += int(summary.get("covered", 0)); total += int(summary.get("count", 0))
    if total == 0:
        raise SystemExit("coverage report contains no production lines")
    percent = covered * 100.0 / total
    print(f"covered={covered} total={total} percent={percent:.2f} required=80.00")
    return 0 if percent >= 80.0 else 1

if __name__ == "__main__":
    if len(sys.argv) != 2: raise SystemExit("usage: check_thresholds.py coverage.json")
    raise SystemExit(main(sys.argv[1]))
