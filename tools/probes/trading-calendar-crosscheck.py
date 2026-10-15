#!/usr/bin/env python3
"""Fail closed when the checked-in SSE trading calendar and the local TDX
terminal disagree.

This is the review-time cross-check tool for the `TradingCalendar` family.
It reads the sealed vector from the repository source of truth
(`crates/magic-exchange-rs/src/trading_calendar.rs`) and asks the running TDX
terminal (TQ-Local, method `get_trading_dates`) for the same window. The
terminal list lags by its own after-market download, so it must be an exact
contiguous prefix of the sealed vector. Read-only loopback only; no
credentials; exit code 1 on any mismatch.
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates" / "magic-exchange-rs" / "src" / "trading_calendar.rs"
ENDPOINT = os.environ.get("TQ_ENDPOINT", "http://127.0.0.1:17709/")
ISO_DATE = re.compile(r'"(20\d{2}-\d{2}-\d{2})"')


def sealed_vector(year: int) -> list[str]:
    text = SOURCE.read_text(encoding="utf-8")
    name = f"FIXED_TRADING_DAYS_{year}"
    start = text.find(name)
    if start < 0:
        raise SystemExit(f"{name} not found in {SOURCE}")
    block_end = text.find("];", start)
    if block_end < 0:
        raise SystemExit(f"{name} table is unterminated")
    dates = ISO_DATE.findall(text[start:block_end])
    if not dates:
        raise SystemExit(f"no sealed dates parsed for {year}")
    return dates


def terminal_trading_days(year: int) -> list[str]:
    body = json.dumps(
        {
            "id": 1,
            "method": "get_trading_dates",
            "params": {
                "market": "SH",
                "start_time": f"{year}0101",
                "end_time": f"{year}1231",
            },
        }
    ).encode()
    request = urllib.request.Request(
        ENDPOINT, data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=15) as response:
        payload = json.loads(response.read().decode())
    result = payload.get("result") or {}
    dates = result.get("Date")
    if not isinstance(dates, list) or not dates:
        raise SystemExit(f"TDX terminal returned no dates: {payload}")
    if str(result.get("ErrorId", "0")) != "0":
        raise SystemExit(f"TDX terminal reported an error: {payload}")
    return [f"{d[0:4]}-{d[4:6]}-{d[6:8]}" for d in dates]


def main() -> int:
    year = int(sys.argv[1]) if len(sys.argv) > 1 else 2026
    sealed = sealed_vector(year)
    observed = terminal_trading_days(year)
    if len(observed) > len(sealed):
        print(f"FAIL: terminal returned {len(observed)} days; sealed has {len(sealed)}")
        return 1
    mismatch = [
        (index, sealed[index], observed_value)
        for index, observed_value in enumerate(observed)
        if sealed[index] != observed_value
    ]
    if mismatch:
        for index, sealed_value, observed_value in mismatch[:10]:
            print(
                f"FAIL at index {index}: sealed={sealed_value} terminal={observed_value}"
            )
        return 1
    print(
        f"OK: terminal list is an exact prefix of the sealed {year} vector "
        f"({len(observed)}/{len(sealed)} days, terminal through {observed[-1]})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
