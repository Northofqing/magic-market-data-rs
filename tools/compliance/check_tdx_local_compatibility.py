#!/usr/bin/env python3
"""Validate TDX executable provenance and official loopback schema evidence."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import re
import sys
from pathlib import Path

FIELDS = [
    "profile_id",
    "status",
    "tdx_exe_version",
    "tdx_exe_sha256",
    "pe_arch",
    "transport",
    "endpoint",
    "method_set",
    "schema_revision",
    "last_live_date",
    "live_probe_count",
    "signature_status",
    "evidence",
    "blocker",
]
SHA256 = re.compile(r"[0-9a-f]{64}")
PROFILE_ID = re.compile(r"[a-z0-9][a-z0-9-]*")
VERSION = re.compile(r"[0-9]+(?:[.,][0-9]+)*")


def read_rows(path: Path) -> tuple[list[dict[str, str]], list[str]]:
    try:
        with path.open(encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle, delimiter="\t", strict=True)
            if reader.fieldnames != FIELDS:
                return [], [
                    "compatibility registry header mismatch: "
                    f"expected {FIELDS!r}, got {reader.fieldnames!r}"
                ]
            rows = list(reader)
    except (OSError, UnicodeError, csv.Error) as error:
        return [], [f"cannot read compatibility registry: {error}"]
    return rows, []


def validate(root: Path, registry: Path) -> list[str]:
    rows, errors = read_rows(registry)
    if errors:
        return errors
    return validate_rows(root, registry, rows)


def validate_rows(
    root: Path, registry: Path, rows: list[dict[str, str]]
) -> list[str]:
    errors: list[str] = []
    if not rows:
        return ["compatibility registry must retain explicit version evidence"]

    seen: set[str] = set()
    for line_number, row in enumerate(rows, start=2):
        label = f"{registry}:{line_number}"
        if None in row:
            errors.append(f"{label}: extra fields are not allowed")
            continue
        missing = [field for field in FIELDS if row.get(field) is None]
        if missing:
            errors.append(f"{label}: missing fields: {', '.join(missing)}")
            continue
        if any(value != value.strip() for value in row.values()):
            errors.append(f"{label}: fields must not contain outer whitespace")
        if any("\n" in value or "\r" in value for value in row.values()):
            errors.append(f"{label}: fields must be single-line")

        profile_id = row["profile_id"]
        if not PROFILE_ID.fullmatch(profile_id):
            errors.append(f"{label}: profile_id must be lowercase kebab-case")
        if profile_id in seen:
            errors.append(f"{label}: duplicate profile_id {profile_id}")
        seen.add(profile_id)

        if not VERSION.fullmatch(row["tdx_exe_version"]):
            errors.append(f"{label}: tdx_exe_version is not a bounded version identity")
        if not SHA256.fullmatch(row["tdx_exe_sha256"]):
            errors.append(f"{label}: tdx_exe_sha256 must be 64 lowercase hex characters")
        if row["pe_arch"] != "x86_64":
            errors.append(f"{label}: only the evidenced x86_64 architecture is allowed")
        if row["transport"] != "official-tq-local-http":
            errors.append(f"{label}: transport must be official-tq-local-http")
        if row["endpoint"] != "http://127.0.0.1:17709/":
            errors.append(f"{label}: endpoint must be the fixed TQ-Local loopback origin")
        if row["method_set"] != "get_stock_list,get_pricevol,get_market_snapshot":
            errors.append(f"{label}: method_set must equal the implemented read-only set")
        if row["schema_revision"] != "1":
            errors.append(f"{label}: schema_revision must equal 1")

        evidence = Path(row["evidence"])
        if (
            evidence.is_absolute()
            or evidence.suffix != ".md"
            or evidence.parts[:2] != ("docs", "integrations")
            or ".." in evidence.parts
        ):
            errors.append(f"{label}: evidence must be Markdown under docs/integrations")
        else:
            path = root / evidence
            if not path.is_file() or path.is_symlink():
                errors.append(f"{label}: evidence must be a regular repository file")

        try:
            live_count = int(row["live_probe_count"])
        except ValueError:
            live_count = -1
            errors.append(f"{label}: live_probe_count must be an integer")

        if row["status"] == "runtime-compatible":
            try:
                dt.date.fromisoformat(row["last_live_date"])
            except ValueError:
                errors.append(f"{label}: runtime-compatible row requires an ISO live date")
            if live_count <= 0:
                errors.append(f"{label}: runtime-compatible row requires positive live probes")
            if row["blocker"] != "-":
                errors.append(f"{label}: runtime-compatible row must not declare a blocker")
        elif row["status"] == "blocked":
            if row["blocker"] in {"", "-"}:
                errors.append(f"{label}: blocked row requires an explicit blocker")
        else:
            errors.append(f"{label}: status must be runtime-compatible or blocked")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path("docs/integrations/tdx-local-terminal-compatibility.tsv"),
    )
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    registry = arguments.registry
    if not registry.is_absolute():
        registry = root / registry
    errors = validate(root, registry)
    if errors:
        for error in errors:
            print(f"TDX local compatibility error: {error}", file=sys.stderr)
        return 1
    print("TDX local compatibility registry passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
