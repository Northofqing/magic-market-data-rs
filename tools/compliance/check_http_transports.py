#!/usr/bin/env python3
"""Validate reviewed HTTP transport boundaries against production manifests."""

from __future__ import annotations

import argparse
import csv
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

FIELDS = [
    "crate",
    "mode",
    "direct_dependencies",
    "shared_transport",
    "migration_status",
    "reason",
]
DIRECT_HTTP_DEPENDENCIES = frozenset(
    {"reqwest", "ureq", "rustls", "native-tls", "ring"}
)
SHARED_TRANSPORT = "magic-market-transport"
INFRASTRUCTURE_CRATE = "magic-market-transport"
MODES = {"infrastructure", "shared", "legacy-direct", "hybrid"}
MIGRATION_STATUSES = {"target", "legacy", "reviewed-exception"}


@dataclass(frozen=True)
class HttpBoundary:
    crate: str
    mode: str
    direct_dependencies: tuple[str, ...]
    shared_transport: bool
    path: Path


def git_tracked_files(root: Path) -> tuple[set[Path], list[str]]:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        return set(), [
            f"cannot enumerate Git-tracked files: {message or result.returncode}"
        ]
    try:
        names = result.stdout.decode("utf-8").split("\0")
    except UnicodeDecodeError:
        return set(), ["Git-tracked file names must be valid UTF-8"]
    return {Path(name) for name in names if name}, []


def safe_repository_file(
    root: Path, path: Path, tracked: set[Path], label: str
) -> list[str]:
    try:
        relative = path.relative_to(root)
    except ValueError:
        return [f"{label} is outside the repository: {path}"]
    errors: list[str] = []
    if relative not in tracked:
        errors.append(f"{label} is not Git-tracked: {relative}")
    if path.is_symlink():
        errors.append(f"{label} must not be a symbolic link: {relative}")
        return errors
    try:
        path.resolve(strict=True).relative_to(root.resolve(strict=True))
    except (FileNotFoundError, ValueError):
        errors.append(
            f"{label} escapes or does not exist in the repository: {relative}"
        )
    if not path.is_file():
        errors.append(f"{label} is not a regular file: {relative}")
    return errors


def classify(crate: str, direct: tuple[str, ...], shared: bool) -> str:
    if crate == INFRASTRUCTURE_CRATE:
        return "infrastructure"
    if direct and shared:
        return "hybrid"
    if direct:
        return "legacy-direct"
    return "shared"


def discover_boundaries(
    root: Path, tracked: set[Path]
) -> tuple[dict[str, HttpBoundary], list[str]]:
    discovered: dict[str, HttpBoundary] = {}
    errors: list[str] = []
    crates = root / "crates"
    if not crates.is_dir():
        return {}, [f"missing crates directory: {crates}"]
    for manifest in sorted(crates.glob("*/Cargo.toml")):
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            errors.append(f"cannot parse {manifest}: {error}")
            continue
        dependencies = document.get("dependencies", {})
        if not isinstance(dependencies, dict):
            errors.append(f"{manifest}: [dependencies] must be a TOML table")
            continue
        package = document.get("package", {})
        crate = package.get("name") if isinstance(package, dict) else None
        if not isinstance(crate, str) or not crate:
            errors.append(f"{manifest}: package.name must be a non-empty string")
            continue
        direct = tuple(sorted(DIRECT_HTTP_DEPENDENCIES & dependencies.keys()))
        shared = SHARED_TRANSPORT in dependencies
        if crate != INFRASTRUCTURE_CRATE and not direct and not shared:
            continue
        errors.extend(
            safe_repository_file(root, manifest, tracked, "HTTP transport manifest")
        )
        boundary = HttpBoundary(
            crate=crate,
            mode=classify(crate, direct, shared),
            direct_dependencies=direct,
            shared_transport=shared,
            path=manifest,
        )
        if crate in discovered:
            errors.append(
                f"duplicate HTTP transport crate {crate}: "
                f"{discovered[crate].path} and {manifest}"
            )
        else:
            discovered[crate] = boundary
    return discovered, errors


def read_registry(path: Path) -> tuple[list[dict[str, str]], list[str]]:
    if not path.is_file():
        return [], [f"missing HTTP transport registry: {path}"]
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames != FIELDS:
            return [], [
                "HTTP transport registry header mismatch: "
                f"expected {FIELDS!r}, got {reader.fieldnames!r}"
            ]
        return list(reader), []


def parse_direct_dependencies(
    value: str, label: str
) -> tuple[tuple[str, ...], list[str]]:
    if value == "-":
        return (), []
    parts = value.split(",")
    errors: list[str] = []
    if (
        not parts
        or any(not part for part in parts)
        or tuple(parts) != tuple(sorted(set(parts)))
        or any(part not in DIRECT_HTTP_DEPENDENCIES for part in parts)
    ):
        errors.append(
            f"{label}: direct_dependencies must be '-' or a sorted, unique "
            "comma-separated subset of "
            f"{sorted(DIRECT_HTTP_DEPENDENCIES)!r}"
        )
    return tuple(parts), errors


def validate_row(row: dict[str, str], label: str) -> list[str]:
    errors: list[str] = []
    if any(row[field] != row[field].strip() for field in FIELDS):
        errors.append(f"{label}: fields must not contain outer whitespace")
    if not row["crate"]:
        errors.append(f"{label}: crate is required")
    if row["mode"] not in MODES:
        errors.append(f"{label}: mode must be one of {sorted(MODES)!r}")
    if row["shared_transport"] not in {"true", "false"}:
        errors.append(f"{label}: shared_transport must be true or false")
    if row["migration_status"] not in MIGRATION_STATUSES:
        errors.append(
            f"{label}: migration_status must be one of "
            f"{sorted(MIGRATION_STATUSES)!r}"
        )
    _, dependency_errors = parse_direct_dependencies(
        row["direct_dependencies"], label
    )
    errors.extend(dependency_errors)
    if row["mode"] in {"infrastructure", "shared"}:
        if row["migration_status"] != "target":
            errors.append(
                f"{label}: infrastructure/shared mode requires target status"
            )
        if row["reason"] != "-":
            errors.append(
                f"{label}: infrastructure/shared target reason must be '-'"
            )
    elif row["mode"] in {"legacy-direct", "hybrid"}:
        if row["migration_status"] not in {"legacy", "reviewed-exception"}:
            errors.append(
                f"{label}: legacy-direct/hybrid status must be legacy or "
                "reviewed-exception"
            )
        if row["reason"] in {"", "-"}:
            errors.append(
                f"{label}: legacy-direct/hybrid mode requires an explicit reason"
            )
    return errors


def validate(root: Path, registry_path: Path) -> list[str]:
    tracked, errors = git_tracked_files(root)
    if errors:
        return errors
    discovered, discovery_errors = discover_boundaries(root, tracked)
    errors.extend(discovery_errors)
    errors.extend(
        safe_repository_file(root, registry_path, tracked, "HTTP transport registry")
    )
    rows, registry_errors = read_registry(registry_path)
    errors.extend(registry_errors)
    if registry_errors:
        return errors

    registered: dict[str, dict[str, str]] = {}
    for line_number, row in enumerate(rows, start=2):
        label = f"{registry_path}:{line_number}"
        errors.extend(validate_row(row, label))
        crate = row["crate"]
        if crate in registered:
            errors.append(f"{label}: duplicate registry key {crate}")
        else:
            registered[crate] = row

    for crate in sorted(discovered.keys() - registered.keys()):
        errors.append(f"HTTP transport crate missing from registry: {crate}")
    for crate in sorted(registered.keys() - discovered.keys()):
        errors.append(f"registry row has no discovered HTTP transport: {crate}")
    for crate in sorted(discovered.keys() & registered.keys()):
        actual = discovered[crate]
        row = registered[crate]
        registered_direct, _ = parse_direct_dependencies(
            row["direct_dependencies"], f"registry row {crate}"
        )
        if row["mode"] != actual.mode:
            errors.append(
                f"HTTP transport mode drift for {crate}: "
                f"manifest={actual.mode} registry={row['mode']}"
            )
        if registered_direct != actual.direct_dependencies:
            errors.append(
                f"HTTP direct dependency drift for {crate}: "
                f"manifest={','.join(actual.direct_dependencies) or '-'} "
                f"registry={row['direct_dependencies']}"
            )
        if row["shared_transport"] in {"true", "false"}:
            registered_shared = row["shared_transport"] == "true"
            if registered_shared != actual.shared_transport:
                errors.append(
                    f"shared transport drift for {crate}: "
                    f"manifest={str(actual.shared_transport).lower()} "
                    f"registry={row['shared_transport']}"
                )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[2]
    )
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path("docs/integrations/http-transports.tsv"),
    )
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    registry = arguments.registry
    if not registry.is_absolute():
        registry = root / registry
    errors = validate(root, registry)
    if errors:
        for error in errors:
            print(f"HTTP transport boundary error: {error}", file=sys.stderr)
        return 1
    count = len(discover_boundaries(root, git_tracked_files(root)[0])[0])
    print(f"HTTP transport boundary passed: {count} registered crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
