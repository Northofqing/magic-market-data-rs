#!/usr/bin/env python3
"""Fail closed when the registered gRPC transport boundary drifts."""

from __future__ import annotations

import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = ROOT / "docs" / "integrations" / "grpc-services.tsv"
PROTO = ROOT / "crates" / "magic-market-grpc-contracts" / "proto" / "magic" / "market" / "v1" / "market.proto"

HEADER = [
    "crate", "role", "protocol", "default_bind", "remote_bind", "tls", "auth",
    "bounds", "service_set", "status", "reason",
]
SERVICES = {
    "magic-market-grpc-server": {
        "role": "inbound",
        "default_bind": "127.0.0.1",
        "remote_bind": "explicit-only",
        "tls": "required-remote-mtls",
        "auth": "required",
        "service_set": "SystemService,MarketDataService,MarketEventService,TdxAgentService",
    },
    "magic-market-tdx-agent": {
        "role": "outbound",
        "default_bind": "-",
        "remote_bind": "configured-origin",
        "tls": "required-remote-mtls",
        "auth": "required",
        "service_set": "TdxAgentService",
    },
}


def validate() -> None:
    with REGISTRY.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames != HEADER:
            raise SystemExit("grpc-services.tsv header drift")
        rows = list(reader)
    if len(rows) != len(SERVICES):
        raise SystemExit("grpc-services.tsv must contain exactly the two approved boundaries")
    by_crate = {row["crate"]: row for row in rows}
    if set(by_crate) != set(SERVICES):
        raise SystemExit("grpc-services.tsv crate set drift")
    for crate, expected in SERVICES.items():
        row = by_crate[crate]
        if row["protocol"] != "grpc-h2" or row["bounds"] != "operator-injected-positive":
            raise SystemExit(f"{crate} protocol/bounds drift")
        if not row["reason"].strip() or not row["status"].strip():
            raise SystemExit(f"{crate} status/reason must be explicit")
        for field, value in expected.items():
            if row[field] != value:
                raise SystemExit(f"{crate} {field} drift: expected {value!r}")
    proto = PROTO.read_text(encoding="utf-8")
    for service in SERVICES["magic-market-grpc-server"]["service_set"].split(","):
        if f"service {service} {{" not in proto:
            raise SystemExit(f"registered gRPC service missing from proto: {service}")
    forbidden = ("PlaceOrder", "CancelOrder", "AccountService", "PortfolioService")
    for token in forbidden:
        if token in proto:
            raise SystemExit(f"trading/account gRPC surface is forbidden: {token}")


if __name__ == "__main__":
    validate()
    print("gRPC service registry: passed")
