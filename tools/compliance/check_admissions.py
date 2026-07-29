#!/usr/bin/env python3
"""Validate BR-009 capability constants against tracked admission evidence."""

from __future__ import annotations

import argparse
import csv
import re
import sys
from dataclasses import dataclass
from datetime import date
from pathlib import Path

FIELDS = [
    "crate",
    "provider",
    "constant",
    "admitted",
    "evidence",
    "status",
    "last_live_date",
    "live_probe_count",
    "serial_load_count",
    "blocker",
]
CONSTANT = re.compile(
    r"(?m)^\s*pub\s+const\s+([A-Z][A-Z0-9_]*_ADMITTED)"
    r"\s*:\s*bool\s*=\s*(true|false)\s*;"
)


@dataclass(frozen=True)
class SourceCapability:
    crate: str
    constant: str
    admitted: bool
    path: Path

    @property
    def key(self) -> tuple[str, str]:
        return (self.crate, self.constant)


def discover_constants(root: Path) -> tuple[dict[tuple[str, str], SourceCapability], list[str]]:
    discovered: dict[tuple[str, str], SourceCapability] = {}
    errors: list[str] = []
    crates = root / "crates"
    if not crates.is_dir():
        return {}, [f"missing crates directory: {crates}"]
    for path in sorted(crates.rglob("*.rs")):
        relative = path.relative_to(crates)
        if len(relative.parts) < 2:
            continue
        crate = relative.parts[0]
        text = path.read_text(encoding="utf-8")
        for name, value in CONSTANT.findall(text):
            capability = SourceCapability(crate, name, value == "true", path)
            if capability.key in discovered:
                errors.append(
                    f"duplicate Rust admission constant {crate}/{name}: "
                    f"{discovered[capability.key].path} and {path}"
                )
            else:
                discovered[capability.key] = capability
    return discovered, errors


def read_registry(path: Path) -> tuple[list[dict[str, str]], list[str]]:
    if not path.is_file():
        return [], [f"missing admission registry: {path}"]
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames != FIELDS:
            return [], [
                "admission registry header mismatch: "
                f"expected {FIELDS!r}, got {reader.fieldnames!r}"
            ]
        return list(reader), []


def validate(root: Path, registry_path: Path) -> list[str]:
    source, errors = discover_constants(root)
    rows, registry_errors = read_registry(registry_path)
    errors.extend(registry_errors)
    if registry_errors:
        return errors

    registered: dict[tuple[str, str], dict[str, str]] = {}
    provider_capabilities: set[tuple[str, str]] = set()
    for line_number, row in enumerate(rows, start=2):
        label = f"{registry_path}:{line_number}"
        if any(row[field] != row[field].strip() for field in FIELDS):
            errors.append(f"{label}: fields must not contain outer whitespace")
        if any("\n" in row[field] or "\r" in row[field] for field in FIELDS):
            errors.append(f"{label}: fields must be single-line")
        key = (row["crate"], row["constant"])
        if key in registered:
            errors.append(f"{label}: duplicate registry key {key[0]}/{key[1]}")
        else:
            registered[key] = row
        provider_key = (row["provider"], row["constant"])
        if provider_key in provider_capabilities:
            errors.append(
                f"{label}: duplicate Provider/capability identity "
                f"{provider_key[0]}/{provider_key[1]}"
            )
        provider_capabilities.add(provider_key)
        errors.extend(validate_row(root, row, label))

    missing = sorted(source.keys() - registered.keys())
    unknown = sorted(registered.keys() - source.keys())
    for crate, constant in missing:
        errors.append(f"Rust admission constant missing from registry: {crate}/{constant}")
    for crate, constant in unknown:
        errors.append(f"registry row has no Rust admission constant: {crate}/{constant}")
    for key in sorted(source.keys() & registered.keys()):
        expected = source[key].admitted
        actual = registered[key]["admitted"] == "true"
        if registered[key]["admitted"] not in {"true", "false"}:
            continue
        if actual != expected:
            errors.append(
                f"admission boolean drift for {key[0]}/{key[1]}: "
                f"Rust={str(expected).lower()} registry={str(actual).lower()}"
            )
    return errors


def validate_row(root: Path, row: dict[str, str], label: str) -> list[str]:
    errors: list[str] = []
    if not row["crate"] or not row["provider"] or not row["constant"]:
        errors.append(f"{label}: crate, provider and constant are required")
    if row["admitted"] not in {"true", "false"}:
        errors.append(f"{label}: admitted must be true or false")
        return errors

    evidence = row["evidence"]
    evidence_path = Path(evidence)
    if (
        evidence_path.is_absolute()
        or evidence_path.suffix != ".md"
        or len(evidence_path.parts) < 3
        or evidence_path.parts[:2] != ("docs", "integrations")
        or ".." in evidence_path.parts
    ):
        errors.append(f"{label}: evidence must be a Markdown path under docs/integrations/")
    elif not (root / evidence_path).is_file():
        errors.append(f"{label}: evidence document does not exist: {evidence}")

    counts: dict[str, int] = {}
    for field in ("live_probe_count", "serial_load_count"):
        try:
            counts[field] = int(row[field])
            if counts[field] < 0:
                raise ValueError
        except ValueError:
            errors.append(f"{label}: {field} must be a non-negative integer")

    live_date = row["last_live_date"]
    if live_date:
        try:
            parsed = date.fromisoformat(live_date)
            if parsed.isoformat() != live_date:
                raise ValueError
        except ValueError:
            errors.append(f"{label}: last_live_date must be canonical YYYY-MM-DD")

    if row["admitted"] == "true":
        if row["status"] != "admitted":
            errors.append(f"{label}: admitted capability status must be admitted")
        if not live_date:
            errors.append(f"{label}: admitted capability requires last_live_date")
        if counts.get("live_probe_count", -1) < 2:
            errors.append(f"{label}: admitted capability requires at least two live probes")
        if counts.get("serial_load_count", -1) < 3:
            errors.append(f"{label}: admitted capability requires at least three serial loads")
        if row["blocker"]:
            errors.append(f"{label}: admitted capability must not declare a blocker")
    else:
        if row["status"] != "blocked":
            errors.append(f"{label}: unadmitted capability status must be blocked")
        if not row["blocker"]:
            errors.append(f"{label}: unadmitted capability requires an explicit blocker")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path("docs/integrations/admissions.tsv"),
    )
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    registry = arguments.registry
    if not registry.is_absolute():
        registry = root / registry
    errors = validate(root, registry)
    if errors:
        for error in errors:
            print(f"BR-009 admission error: {error}", file=sys.stderr)
        return 1
    print(f"BR-009 admission registry passed: {len(discover_constants(root)[0])} capabilities")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
