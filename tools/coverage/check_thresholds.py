#!/usr/bin/env python3
"""Check llvm-cov JSON against repository coverage thresholds."""

from __future__ import annotations

import json
import re
import sys
import tomllib
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
NON_EXECUTABLE_SOURCE_PATHS = frozenset(
    {
        "crates/magic-exchange-rs/src/lib.rs",
        "crates/magic-market-analysis/src/lib.rs",
        "crates/magic-market-core/src/error.rs",
        "crates/magic-market-core/src/lib.rs",
        "crates/magic-market-router/src/lib.rs",
        "crates/magic-tdx-rs/src/block/mod.rs",
        "crates/magic-tdx-rs/src/fund/mod.rs",
        "crates/magic-tdx-rs/src/lib.rs",
        "crates/magic-tdx-rs/src/net/mod.rs",
        "crates/magic-tdx-rs/src/profile/constants.rs",
        "crates/magic-tdx-rs/src/profile/mod.rs",
        "crates/magic-tdx-rs/src/protocol/constants.rs",
        "crates/magic-tdx-rs/src/protocol/mod.rs",
        "crates/magic-tdx-rs/src/protocol/packet.rs",
        "crates/magic-tdx-rs/src/reader/mod.rs",
        "crates/magic-tdx-rs/src/source/enums.rs",
        "crates/magic-tdx-rs/src/source/mod.rs",
    }
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


def rust_code_mask(source: str) -> str:
    output = list(source)
    index = 0
    length = len(source)

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if output[position] != "\n":
                output[position] = " "

    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = length if end == -1 else end
            blank(index, end)
            index = end
            continue
        if source.startswith("/*", index):
            start = index
            depth = 1
            index += 2
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            if depth:
                fail("contains an unterminated Rust block comment")
            blank(start, index)
            continue

        raw_prefix = None
        if source.startswith("br", index):
            raw_prefix = 2
        elif source[index] == "r":
            raw_prefix = 1
        if raw_prefix is not None and (
            index == 0 or not (source[index - 1].isalnum() or source[index - 1] == "_")
        ):
            marker = index + raw_prefix
            while marker < length and source[marker] == "#":
                marker += 1
            if marker < length and source[marker] == '"':
                hashes = source[index + raw_prefix : marker]
                terminator = '"' + hashes
                end = source.find(terminator, marker + 1)
                if end == -1:
                    fail("contains an unterminated Rust raw string")
                end += len(terminator)
                blank(index, end)
                index = end
                continue

        if source[index] == '"':
            start = index
            index += 1
            escaped = False
            while index < length:
                char = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            else:
                fail("contains an unterminated Rust string")
            blank(start, index)
            continue

        if source[index] == "'":
            next_char = source[index + 1] if index + 1 < length else ""
            lifetime = (next_char.isalpha() or next_char == "_") and (
                index + 2 >= length or source[index + 2] != "'"
            )
            if not lifetime:
                start = index
                index += 1
                escaped = False
                while index < length:
                    char = source[index]
                    index += 1
                    if escaped:
                        escaped = False
                    elif char == "\\":
                        escaped = True
                    elif char == "'":
                        break
                    elif char == "\n":
                        fail("contains an unterminated Rust character literal")
                else:
                    fail("contains an unterminated Rust character literal")
                blank(start, index)
                continue
        index += 1
    return "".join(output)


def attributed_item_end(mask: str, start: int) -> int:
    index = start
    parens = 0
    brackets = 0
    while index < len(mask):
        char = mask[index]
        if char == "(":
            parens += 1
        elif char == ")":
            parens = max(0, parens - 1)
        elif char == "[":
            brackets += 1
        elif char == "]":
            brackets = max(0, brackets - 1)
        elif char == ";" and parens == 0 and brackets == 0:
            return index + 1
        elif char == "{" and parens == 0 and brackets == 0:
            depth = 1
            index += 1
            while index < len(mask) and depth:
                if mask[index] == "{":
                    depth += 1
                elif mask[index] == "}":
                    depth -= 1
                index += 1
            if depth:
                fail("contains an unterminated #[cfg(test)] Rust item")
            return index
        index += 1
    fail("contains #[cfg(test)] without a complete attributed item")


def cfg_test_lines(source: str) -> set[int]:
    mask = rust_code_mask(source)
    cfg_attributes = re.compile(r"#\s*\[\s*cfg\s*\(([^]]*)\)\s*\]")
    excluded: set[int] = set()
    for match in cfg_attributes.finditer(mask):
        expression = re.sub(r"\s+", "", match.group(1))
        if "test" not in expression or expression == "not(test)":
            continue
        if expression != "test":
            fail(f"contains unsupported test cfg expression cfg({expression})")
        end = attributed_item_end(mask, match.end())
        first_line = source.count("\n", 0, match.start()) + 1
        last_line = source.count("\n", 0, end) + 1
        excluded.update(range(first_line, last_line + 1))
    return excluded


def segment_line_summary(
    item: dict,
    filename: str,
    source: str,
) -> tuple[int, int]:
    segments = item.get("segments")
    if not isinstance(segments, list) or not segments:
        fail(f"has invalid or missing coverage segments for {filename}")
    excluded = cfg_test_lines(source)
    line_count = source.count("\n") + int(not source.endswith("\n"))
    covered_by_line: dict[int, int] = {}
    previous_position = (0, 0)

    for index, segment in enumerate(segments):
        if (
            not isinstance(segment, list)
            or len(segment) != 6
            or type(segment[0]) is not int
            or type(segment[1]) is not int
            or type(segment[2]) is not int
            or any(type(value) is not bool for value in segment[3:])
        ):
            fail(f"has an invalid coverage segment for {filename}")
        line, column, count, has_count, _, _ = segment
        position = (line, column)
        if line < 1 or column < 1 or count < 0 or position <= previous_position:
            fail(f"has an invalid coverage segment for {filename}")
        previous_position = position
        if not has_count:
            continue
        if index + 1 >= len(segments):
            fail(f"has an unterminated counted coverage segment for {filename}")
        next_segment = segments[index + 1]
        if (
            not isinstance(next_segment, list)
            or len(next_segment) != 6
            or type(next_segment[0]) is not int
            or type(next_segment[1]) is not int
        ):
            fail(f"has an invalid coverage segment for {filename}")
        end_line, end_column = next_segment[0], next_segment[1]
        final_line = end_line if end_column > 1 else end_line - 1
        if final_line > line_count:
            fail(f"coverage segment exceeds source length for {filename}")
        for covered_line in range(line, final_line + 1):
            if covered_line not in excluded:
                covered_by_line[covered_line] = max(
                    covered_by_line.get(covered_line, 0),
                    count,
                )
    return (
        sum(count > 0 for count in covered_by_line.values()),
        len(covered_by_line),
    )


def workspace_layout(repo_root: Path) -> tuple[set[str], set[str]]:
    manifest = repo_root / "Cargo.toml"
    try:
        workspace = tomllib.loads(manifest.read_text(encoding="utf-8")).get("workspace")
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(f"cannot read workspace manifest: {error}")
    if not isinstance(workspace, dict):
        fail("workspace manifest has no [workspace] table")
    patterns = workspace.get("members")
    if not isinstance(patterns, list) or not patterns:
        fail("workspace manifest has no members")

    members: set[str] = set()
    sources: set[str] = set()
    for index, pattern in enumerate(patterns):
        if not isinstance(pattern, str) or not pattern:
            fail(f"workspace member {index} is invalid")
        matches = sorted(repo_root.glob(pattern))
        if not matches:
            fail(f"workspace member pattern {pattern} matches no directory")
        for member in matches:
            resolved = member.resolve()
            try:
                relative = resolved.relative_to(repo_root)
            except ValueError:
                fail(f"workspace member {pattern} resolves outside repository")
            if not (resolved / "Cargo.toml").is_file():
                fail(f"workspace member {relative.as_posix()} has no Cargo.toml")
            member_name = relative.as_posix()
            members.add(member_name)
            for source in (resolved / "src").glob("**/*.rs"):
                if source.is_file():
                    sources.add(source.resolve().relative_to(repo_root).as_posix())

    if not sources:
        fail("workspace contains no Rust production sources")
    return members, sources


def report_path(filename: str, repo_root: Path) -> str:
    portable = filename.replace("\\", "/")
    candidate = Path(portable)
    if not candidate.is_absolute():
        candidate = repo_root / candidate
    resolved = candidate.resolve()
    try:
        relative = resolved.relative_to(repo_root)
    except ValueError:
        fail(f"contains file outside repository: {filename}")
    if not resolved.is_file():
        fail(f"references missing file {relative.as_posix()}")
    return relative.as_posix()


def existing_critical_families(source_paths: set[str]) -> set[str]:
    existing: set[str] = set()
    for path in source_paths:
        selected_path = f"/{path}"
        for name, selector in CRITICAL_FAMILIES.items():
            if selector(selected_path):
                existing.add(name)
    return existing


def main(path: str, repo_root: Path | None = None) -> int:
    root = (repo_root or Path(__file__).resolve().parents[2]).resolve()
    export = read_export(path)
    members, workspace_sources = workspace_layout(root)
    known_non_executable = NON_EXECUTABLE_SOURCE_PATHS & workspace_sources
    expected_sources = workspace_sources - known_non_executable
    files = export["files"]
    seen: set[str] = set()
    production: list[tuple[str, int, int]] = []

    for index, item in enumerate(files):
        if not isinstance(item, dict):
            fail(f"file record {index} must be an object")
        filename = item.get("filename")
        if not isinstance(filename, str) or not filename:
            fail(f"file record {index} has an invalid filename")
        filename = report_path(filename, root)
        if filename in seen:
            fail(f"contains duplicate coverage file {filename}")
        seen.add(filename)
        line_summary(item, filename)
        path = Path(filename)
        looks_like_source = (
            len(path.parts) >= 4
            and path.parts[0] == "crates"
            and path.parts[2] == "src"
            and path.suffix == ".rs"
        )
        if looks_like_source and filename not in workspace_sources:
            fail(f"file is not a workspace production source: {filename}")
        if filename in workspace_sources:
            source = (root / filename).read_text(encoding="utf-8")
            covered, count = segment_line_summary(item, filename, source)
            production.append((filename, covered, count))

    measured_sources = {filename for filename, _, _ in production}
    missing_sources = sorted(expected_sources - measured_sources)
    if missing_sources:
        preview = ", ".join(missing_sources[:3])
        suffix = "" if len(missing_sources) <= 3 else ", ..."
        fail(f"is missing expected production source: {preview}{suffix}")

    for member in sorted(members):
        prefix = f"{member}/src/"
        if not any(filename.startswith(prefix) for filename in measured_sources):
            fail(f"is missing workspace target coverage for {member}")

    overall_covered = sum(covered for _, covered, _ in production)
    overall_total = sum(count for _, _, count in production)
    if not production or overall_total == 0:
        fail("contains no production lines")

    existing = existing_critical_families(expected_sources)
    measured_by_family: dict[str, list[tuple[int, int]]] = {
        name: [] for name in existing
    }
    for filename, covered, count in production:
        selected_path = f"/{filename}"
        for name in existing:
            if CRITICAL_FAMILIES[name](selected_path):
                measured_by_family[name].append((covered, count))

    for name, measurements in sorted(measured_by_family.items()):
        if not measurements or sum(total for _, total in measurements) == 0:
            fail(f"critical family {name} exists but has no measured file")

    critical_rows = [
        (covered, count)
        for filename, covered, count in production
        if any(CRITICAL_FAMILIES[name](f"/{filename}") for name in existing)
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
