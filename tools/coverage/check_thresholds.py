#!/usr/bin/env python3
"""Validate llvm-cov JSON against the repository's release thresholds."""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

OVERALL_REQUIRED = 80
CRITICAL_REQUIRED = 95
CRITICAL_GLOBS = (
    "crates/magic-market-core/src/*.rs",
    "crates/magic-market-router/src/*.rs",
    "crates/magic-tdx-rs/src/net/packet.rs",
    "crates/magic-tdx-rs/src/net/utils.rs",
    "crates/magic-tdx-rs/src/protocol/*.rs",
    "crates/magic-tdx-rs/src/adapter.rs",
    "crates/magic-tdx-rs/src/service/mod.rs",
    "crates/magic-eastmoney-rs/src/*.rs",
    "crates/magic-cninfo-rs/src/*.rs",
    "crates/magic-ths-rs/src/*.rs",
    "crates/magic-cls-rs/src/*.rs",
    "crates/magic-baidu-rs/src/*.rs",
    "crates/magic-iwencai-rs/src/*.rs",
)

_EXCLUDED_COMPONENTS = {"tests", "examples", "benches", "fuzz", "target"}
_WINDOWS_ABSOLUTE = re.compile(r"^[A-Za-z]:/")
_INLINE_TEST_MODULE = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]"
    r"(?:\s*#\s*\[[^\]]+\]\s*)*"
    r"\s*mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{",
    re.MULTILINE,
)
_INLINE_TEST_FUNCTION = re.compile(r"#\s*\[\s*test\s*\]")


class CoverageReportError(ValueError):
    """The report cannot be trusted as release evidence."""


@dataclass(frozen=True)
class LineCounts:
    covered: int = 0
    total: int = 0

    def add(self, other: "LineCounts") -> "LineCounts":
        return LineCounts(
            covered=self.covered + other.covered,
            total=self.total + other.total,
        )

    def passes(self, required_percent: int) -> bool:
        return (
            self.total > 0
            and self.covered * 100 >= self.total * required_percent
        )

    def percent(self) -> float:
        return self.covered * 100.0 / self.total


@dataclass(frozen=True)
class CoverageResult:
    overall: LineCounts
    critical: LineCounts


def _normalize_production_path(filename: str, repo_root: Path) -> str | None:
    normalized = filename.replace("\\", "/")
    if not normalized or "\x00" in normalized:
        raise CoverageReportError("coverage filename must be non-empty text")

    root = repo_root.resolve()
    if _WINDOWS_ABSOLUTE.match(normalized):
        marker = f"/{root.name}/"
        marker_index = normalized.find(marker)
        if marker_index < 0:
            return None
        relative = normalized[marker_index + len(marker) :]
    elif normalized.startswith("/"):
        absolute = Path(normalized).resolve()
        try:
            relative = absolute.relative_to(root).as_posix()
        except ValueError:
            return None
    else:
        relative = normalized
        while relative.startswith("./"):
            relative = relative[2:]

    pure = PurePosixPath(relative)
    parts = pure.parts
    if (
        len(parts) < 4
        or parts[0] != "crates"
        or parts[2] != "src"
        or pure.suffix != ".rs"
        or any(part in _EXCLUDED_COMPONENTS for part in parts)
        or any(part in {".", ".."} for part in parts)
    ):
        return None

    source = (root / Path(*parts)).resolve()
    try:
        source.relative_to(root)
    except ValueError:
        return None
    if not source.is_file():
        return None
    return pure.as_posix()


def _integer(value: Any, field: str) -> int:
    if type(value) is not int:
        raise CoverageReportError(f"{field} must be an integer")
    if value < 0:
        raise CoverageReportError(f"{field} must be non-negative")
    return value


def _line_counts(item: dict[str, Any], filename: str) -> LineCounts:
    summary = item.get("summary")
    if not isinstance(summary, dict):
        raise CoverageReportError(f"{filename}: summary must be an object")
    lines = summary.get("lines")
    if not isinstance(lines, dict):
        raise CoverageReportError(f"{filename}: summary.lines must be an object")
    covered = _integer(lines.get("covered"), f"{filename}: covered")
    total = _integer(lines.get("count"), f"{filename}: count")
    if covered > total:
        raise CoverageReportError(
            f"{filename}: covered lines cannot exceed counted lines"
        )
    return LineCounts(covered=covered, total=total)


def _report_files(payload: Any) -> list[dict[str, Any]]:
    if not isinstance(payload, dict):
        raise CoverageReportError("coverage root must be an object")
    data = payload.get("data")
    if not isinstance(data, list) or not data:
        raise CoverageReportError("coverage data must be a non-empty array")

    files: list[dict[str, Any]] = []
    for index, export in enumerate(data):
        if not isinstance(export, dict):
            raise CoverageReportError(f"coverage data[{index}] must be an object")
        export_files = export.get("files")
        if not isinstance(export_files, list):
            raise CoverageReportError(
                f"coverage data[{index}].files must be an array"
            )
        for file_index, item in enumerate(export_files):
            if not isinstance(item, dict):
                raise CoverageReportError(
                    f"coverage data[{index}].files[{file_index}] must be an object"
                )
            files.append(item)
    return files


def _reject_inline_critical_tests(repo_root: Path) -> None:
    for pattern in CRITICAL_GLOBS:
        for source in sorted(repo_root.glob(pattern)):
            text = source.read_text(encoding="utf-8")
            if _INLINE_TEST_MODULE.search(text) or _INLINE_TEST_FUNCTION.search(text):
                relative = source.relative_to(repo_root).as_posix()
                raise CoverageReportError(
                    "critical source contains inline test bodies; move them "
                    f"to a path-based external test module: {relative}"
                )


def evaluate(path: str | Path, repo_root: str | Path | None = None) -> CoverageResult:
    root = (
        Path(repo_root)
        if repo_root is not None
        else Path(__file__).resolve().parents[2]
    )
    root = root.resolve()
    _reject_inline_critical_tests(root)
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    files = _report_files(payload)

    measured: dict[str, LineCounts] = {}
    for item in files:
        filename = item.get("filename")
        if not isinstance(filename, str):
            raise CoverageReportError("coverage filename must be text")
        production_path = _normalize_production_path(filename, root)
        if production_path is None:
            continue
        if production_path in measured:
            raise CoverageReportError(
                f"duplicate production filename: {production_path}"
            )
        measured[production_path] = _line_counts(item, production_path)

    overall = LineCounts()
    for counts in measured.values():
        overall = overall.add(counts)
    if overall.total == 0:
        raise CoverageReportError("coverage report contains no production lines")

    critical_paths: set[str] = set()
    for pattern in CRITICAL_GLOBS:
        matches = {
            filename
            for filename, counts in measured.items()
            if counts.total > 0 and PurePosixPath(filename).match(pattern)
        }
        if not matches:
            raise CoverageReportError(
                f"critical coverage glob has no measured file: {pattern}"
            )
        critical_paths.update(matches)

    critical = LineCounts()
    for filename in sorted(critical_paths):
        critical = critical.add(measured[filename])
    return CoverageResult(overall=overall, critical=critical)


def _print_counts(label: str, counts: LineCounts, required: int) -> None:
    print(
        f"{label}: covered={counts.covered} total={counts.total} "
        f"percent={counts.percent():.2f} required={required}.00"
    )


def main(path: str, repo_root: str | Path | None = None) -> int:
    try:
        result = evaluate(path, repo_root=repo_root)
    except (CoverageReportError, json.JSONDecodeError, OSError, UnicodeError) as error:
        print(f"coverage report invalid: {error}", file=sys.stderr)
        return 2

    _print_counts("overall", result.overall, OVERALL_REQUIRED)
    _print_counts("critical", result.critical, CRITICAL_REQUIRED)
    if not result.overall.passes(OVERALL_REQUIRED):
        return 1
    if not result.critical.passes(CRITICAL_REQUIRED):
        return 1
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_thresholds.py coverage.json")
    raise SystemExit(main(sys.argv[1]))
