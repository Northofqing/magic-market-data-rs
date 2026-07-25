#!/usr/bin/env python3
"""Check llvm-cov JSON against repository coverage thresholds."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Callable

OVERALL_REQUIRED = 80.0
CRITICAL_REQUIRED = 95.0
CRITICAL_FAMILIES: dict[str, Callable[[str], bool]] = {
    "codec": lambda path: "/codec/" in path,
    "protocol": lambda path: "/protocol/" in path,
    "adjustment": lambda path: "/adjustment/" in path,
    "service/common.rs": lambda path: path.endswith("/service/common.rs"),
    "adapter.rs": lambda path: path.endswith("/adapter.rs"),
}
EXCLUDED_MARKERS = ("/tests/", "/examples/", "/benches/", "/fuzz/", "/target/")


def normalized(path: str) -> str:
    return "/" + path.replace("\\", "/").lstrip("/")


def is_production(path: str) -> bool:
    return (
        "/crates/" in path
        and "/src/" in path
        and all(marker not in path for marker in EXCLUDED_MARKERS)
    )


def fail(message: str) -> None:
    raise SystemExit(f"coverage report {message}")


def read_export(path: str) -> dict:
    try:
        report = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"is invalid coverage JSON: {error}")

    if not isinstance(report, dict):
        fail("root must be an object")
    exports = report.get("data")
    if not isinstance(exports, list) or len(exports) != 1:
        fail("must contain exactly one export object")
    export = exports[0]
    if not isinstance(export, dict):
        fail("export must be an object")
    files = export.get("files")
    if not isinstance(files, list):
        fail("export files must be an array")
    return export


def line_summary(item: dict, filename: str) -> tuple[int, int]:
    try:
        lines = item["summary"]["lines"]
        covered = lines["covered"]
        count = lines["count"]
    except (KeyError, TypeError):
        fail(f"has an invalid line summary for {filename}")

    if (
        type(covered) is not int
        or type(count) is not int
        or covered < 0
        or count < 0
        or covered > count
    ):
        fail(f"has an invalid line summary for {filename}")
    return covered, count


def existing_critical_families(repo_root: Path) -> set[str]:
    source_paths = (
        normalized(str(path))
        for path in repo_root.glob("crates/*/src/**/*")
        if path.is_file()
    )
    existing: set[str] = set()
    for path in source_paths:
        for name, selector in CRITICAL_FAMILIES.items():
            if selector(path):
                existing.add(name)
    return existing


def main(path: str, repo_root: Path | None = None) -> int:
    root = repo_root or Path(__file__).resolve().parents[2]
    export = read_export(path)
    files = export["files"]
    seen: set[str] = set()
    production: list[tuple[str, int, int]] = []

    for index, item in enumerate(files):
        if not isinstance(item, dict):
            fail(f"file record {index} must be an object")
        filename = item.get("filename")
        if not isinstance(filename, str) or not filename:
            fail(f"file record {index} has an invalid filename")
        filename = normalized(filename)
        if filename in seen:
            fail(f"contains duplicate coverage file {filename}")
        seen.add(filename)
        covered, count = line_summary(item, filename)
        if is_production(filename):
            production.append((filename, covered, count))

    overall_covered = sum(covered for _, covered, _ in production)
    overall_total = sum(count for _, _, count in production)
    if not production or overall_total == 0:
        fail("contains no production lines")

    existing = existing_critical_families(root)
    measured_by_family: dict[str, list[tuple[int, int]]] = {
        name: [] for name in existing
    }
    for filename, covered, count in production:
        for name in existing:
            if CRITICAL_FAMILIES[name](filename):
                measured_by_family[name].append((covered, count))

    for name, measurements in sorted(measured_by_family.items()):
        if not measurements or sum(total for _, total in measurements) == 0:
            fail(f"critical family {name} exists but has no measured file")

    critical_rows = [
        (covered, count)
        for filename, covered, count in production
        if any(CRITICAL_FAMILIES[name](filename) for name in existing)
    ]
    critical_covered = sum(covered for covered, _ in critical_rows)
    critical_total = sum(count for _, count in critical_rows)

    overall_percent = overall_covered * 100.0 / overall_total
    critical_percent = (
        critical_covered * 100.0 / critical_total if critical_total else 100.0
    )
    print(
        f"overall covered={overall_covered} total={overall_total} "
        f"percent={overall_percent:.2f} required={OVERALL_REQUIRED:.2f}"
    )
    print(
        f"critical covered={critical_covered} total={critical_total} "
        f"percent={critical_percent:.2f} required={CRITICAL_REQUIRED:.2f}"
    )
    return int(
        overall_percent < OVERALL_REQUIRED or critical_percent < CRITICAL_REQUIRED
    )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_thresholds.py coverage.json")
    raise SystemExit(main(sys.argv[1]))
