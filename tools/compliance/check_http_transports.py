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
from typing import Any

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


def dependency_package_name(
    alias: str,
    specification: Any,
    workspace_dependencies: dict[str, Any],
    label: str,
) -> tuple[str | None, list[str]]:
    if isinstance(specification, str):
        return alias, []
    if not isinstance(specification, dict):
        return None, [f"{label}: dependency {alias} must be a TOML string or table"]
    package = specification.get("package")
    if package is not None:
        if isinstance(package, str) and package:
            return package, []
        return None, [f"{label}: dependency {alias}.package must be a non-empty string"]
    if specification.get("workspace") is True:
        inherited = workspace_dependencies.get(alias)
        if inherited is None:
            return None, [
                f"{label}: dependency {alias} inherits a missing "
                "[workspace.dependencies] entry"
            ]
        return dependency_package_name(
            alias,
            inherited,
            {},
            f"{label} inherited workspace dependency",
        )
    return alias, []


def production_dependency_names(
    document: dict[str, Any],
    workspace_dependencies: dict[str, Any],
    manifest: Path,
) -> tuple[set[str], list[str]]:
    dependency_tables: list[tuple[str, Any]] = [
        ("[dependencies]", document.get("dependencies", {}))
    ]
    errors: list[str] = []
    targets = document.get("target", {})
    if not isinstance(targets, dict):
        return set(), [f"{manifest}: [target] must be a TOML table"]
    for target, target_table in targets.items():
        if not isinstance(target_table, dict):
            errors.append(f"{manifest}: [target.{target}] must be a TOML table")
            continue
        dependency_tables.append(
            (
                f"[target.{target}.dependencies]",
                target_table.get("dependencies", {}),
            )
        )

    names: set[str] = set()
    for table_label, dependencies in dependency_tables:
        if not isinstance(dependencies, dict):
            errors.append(f"{manifest}: {table_label} must be a TOML table")
            continue
        for alias, specification in dependencies.items():
            package, dependency_errors = dependency_package_name(
                alias,
                specification,
                workspace_dependencies,
                f"{manifest}: {table_label}",
            )
            errors.extend(dependency_errors)
            if package is not None:
                names.add(package)
    return names, errors


def expand_workspace_paths(
    root: Path, patterns: Any, label: str
) -> tuple[set[Path], list[str]]:
    if not isinstance(patterns, list) or any(
        not isinstance(pattern, str) or not pattern for pattern in patterns
    ):
        return set(), [f"{label} must be an array of non-empty strings"]
    root_resolved = root.resolve(strict=True)
    manifests: set[Path] = set()
    errors: list[str] = []
    for pattern in patterns:
        candidate = root / pattern
        try:
            candidate.resolve(strict=False).relative_to(root_resolved)
        except ValueError:
            errors.append(f"{label} path escapes the repository: {pattern}")
            continue
        try:
            matches = sorted(root.glob(pattern))
        except (NotImplementedError, OSError, ValueError) as error:
            errors.append(f"{label} pattern cannot be expanded ({pattern}): {error}")
            continue
        if not matches:
            errors.append(f"{label} pattern matches no workspace member: {pattern}")
            continue
        for match in matches:
            manifest = match if match.name == "Cargo.toml" else match / "Cargo.toml"
            manifests.add(manifest)
    return manifests, errors


def workspace_manifests(
    root: Path, tracked: set[Path]
) -> tuple[set[Path], dict[str, Any], list[str]]:
    root_manifest = root / "Cargo.toml"
    errors = safe_repository_file(
        root, root_manifest, tracked, "workspace Cargo manifest"
    )
    try:
        document = tomllib.loads(root_manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        return set(), {}, errors + [f"cannot parse {root_manifest}: {error}"]
    workspace = document.get("workspace")
    if not isinstance(workspace, dict):
        return set(), {}, errors + [
            f"{root_manifest}: [workspace] must be a TOML table"
        ]
    members, member_errors = expand_workspace_paths(
        root, workspace.get("members"), "workspace.members"
    )
    errors.extend(member_errors)
    excluded, exclude_errors = expand_workspace_paths(
        root, workspace.get("exclude", []), "workspace.exclude"
    )
    errors.extend(exclude_errors)
    members.difference_update(excluded)
    if isinstance(document.get("package"), dict):
        members.add(root_manifest)
    for manifest in sorted(members):
        errors.extend(
            safe_repository_file(root, manifest, tracked, "workspace member manifest")
        )

    dependencies = workspace.get("dependencies", {})
    if not isinstance(dependencies, dict):
        errors.append(
            f"{root_manifest}: [workspace.dependencies] must be a TOML table"
        )
        dependencies = {}
    return members, dependencies, errors


def discover_boundaries(
    root: Path, tracked: set[Path]
) -> tuple[dict[str, HttpBoundary], list[str]]:
    discovered: dict[str, HttpBoundary] = {}
    manifests, workspace_dependencies, errors = workspace_manifests(root, tracked)
    for manifest in sorted(manifests):
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            errors.append(f"cannot parse {manifest}: {error}")
            continue
        package = document.get("package", {})
        crate = package.get("name") if isinstance(package, dict) else None
        if not isinstance(crate, str) or not crate:
            errors.append(f"{manifest}: package.name must be a non-empty string")
            continue
        dependencies, dependency_errors = production_dependency_names(
            document, workspace_dependencies, manifest
        )
        errors.extend(dependency_errors)
        direct = tuple(sorted(DIRECT_HTTP_DEPENDENCIES & dependencies))
        shared = SHARED_TRANSPORT in dependencies
        if crate != INFRASTRUCTURE_CRATE and not direct and not shared:
            continue
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


def read_registry(
    path: Path,
) -> tuple[list[dict[str | None, str | list[str] | None]], list[str]]:
    if not path.is_file():
        return [], [f"missing HTTP transport registry: {path}"]
    try:
        with path.open(encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle, delimiter="\t", strict=True)
            if reader.fieldnames != FIELDS:
                return [], [
                    "HTTP transport registry header mismatch: "
                    f"expected {FIELDS!r}, got {reader.fieldnames!r}"
                ]
            return list(reader), []
    except csv.Error as error:
        return [], [f"cannot parse HTTP transport registry {path}: {error}"]
    except (OSError, UnicodeError) as error:
        return [], [f"cannot read HTTP transport registry {path}: {error}"]


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


def validate_row(
    row: dict[str | None, str | list[str] | None], label: str
) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if None in row:
        errors.append(f"{label}: extra field(s) are not allowed")
    missing = [field for field in FIELDS if not isinstance(row.get(field), str)]
    if missing:
        errors.append(f"{label}: missing field(s): {', '.join(missing)}")
    if errors:
        return None, errors
    normalized = {field: row[field] for field in FIELDS}
    if any(
        normalized[field] != normalized[field].strip() for field in FIELDS
    ):
        errors.append(f"{label}: fields must not contain outer whitespace")
    if not normalized["crate"]:
        errors.append(f"{label}: crate is required")
    if normalized["mode"] not in MODES:
        errors.append(f"{label}: mode must be one of {sorted(MODES)!r}")
    if normalized["shared_transport"] not in {"true", "false"}:
        errors.append(f"{label}: shared_transport must be true or false")
    if normalized["migration_status"] not in MIGRATION_STATUSES:
        errors.append(
            f"{label}: migration_status must be one of "
            f"{sorted(MIGRATION_STATUSES)!r}"
        )
    _, dependency_errors = parse_direct_dependencies(
        normalized["direct_dependencies"], label
    )
    errors.extend(dependency_errors)
    if normalized["mode"] in {"infrastructure", "shared"}:
        if normalized["migration_status"] != "target":
            errors.append(
                f"{label}: infrastructure/shared mode requires target status"
            )
        if normalized["reason"] != "-":
            errors.append(
                f"{label}: infrastructure/shared target reason must be '-'"
            )
    elif normalized["mode"] in {"legacy-direct", "hybrid"}:
        if normalized["migration_status"] not in {"legacy", "reviewed-exception"}:
            errors.append(
                f"{label}: legacy-direct/hybrid status must be legacy or "
                "reviewed-exception"
            )
        if normalized["reason"] in {"", "-"}:
            errors.append(
                f"{label}: legacy-direct/hybrid mode requires an explicit reason"
            )
    return normalized, errors


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
        normalized, row_errors = validate_row(row, label)
        errors.extend(row_errors)
        if normalized is None:
            continue
        crate = normalized["crate"]
        if crate in registered:
            errors.append(f"{label}: duplicate registry key {crate}")
        else:
            registered[crate] = normalized

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
