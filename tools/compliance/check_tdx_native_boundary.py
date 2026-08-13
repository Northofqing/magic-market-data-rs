#!/usr/bin/env python3
"""Validate the read-only Rust-native TDX discovery boundary.

This checker is deliberately independent from Cargo compilation. It protects
the architectural boundary even on non-Windows CI runners. The native helper
may discover a terminal process but may not load vendor modules or issue HTTP.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable, Mapping


NATIVE_CRATE = "magic-tdx-native-bridge"
NATIVE_RELATIVE = Path("crates") / NATIVE_CRATE
ALLOWED_UNSAFE_RELATIVE = NATIVE_RELATIVE / "src" / "discovery.rs"
SAFE_CRATE_ROOTS = (
    Path("crates/magic-market-core/src/lib.rs"),
    Path("crates/magic-market-monitor/src/lib.rs"),
    Path("crates/magic-tdx-local-rs/src/lib.rs"),
    Path("crates/magic-market-monitor-server/src/main.rs"),
)
FORBIDDEN_NATIVE_LOADING_TOKENS = (
    "LoadLibrary",
    "GetProcAddress",
)
FORBIDDEN_LEGACY_NATIVE_TOKENS = (
    "PYPlugins",
    "TPythClient",
    "tdxrpc",
    "APPROVED_PROFILES",
    "CompatibilityProfile",
    "compatibility_admitted",
    "abi_profile",
    "BlockedCandidateDigest",
)
FORBIDDEN_NATIVE_NETWORK_TOKENS = (
    "TcpStream",
    "UdpSocket",
    "WinHttp",
    "WinSock",
    "127.0.0.1",
    "http://",
    "https://",
)
# Terminal version provenance must come from the language-neutral numeric
# VS_FIXEDFILEINFO root. Localized string tables are intentionally excluded.
FORBIDDEN_LOCALIZED_VERSION_TOKENS = (
    "StringFileInfo",
    "VarFileInfo",
)
ALLOWED_NATIVE_DEPENDENCIES = frozenset(("serde", "serde_json", "windows-sys"))
ALLOWED_NATIVE_SOURCES = frozenset(
    (
        (NATIVE_RELATIVE / "src" / "main.rs").as_posix(),
        (NATIVE_RELATIVE / "src" / "discovery.rs").as_posix(),
    )
)

# Exact native entry-point names are kept in one checker-owned table. Tests
# place violating fixture source below target/, outside the production scan.
FORBIDDEN_ACCOUNT_OR_TRADING_NAMES = (
    "GetOrderStr",
    "SendOrder",
    "CancelOrder",
    "QueryAccount",
    "QueryPosition",
    "QueryOrders",
    "QueryTrades",
    "GetAccount",
    "GetPosition",
)

UNSAFE_TOKEN = re.compile(r"(?<![A-Za-z0-9_])unsafe(?![A-Za-z0-9_])")


def validate_boundary_data(
    *,
    native_manifest: Mapping[str, Any],
    native_main: str,
    native_has_lib: bool,
    rust_sources: Mapping[str, str],
    safe_roots: Mapping[str, str],
    native_sources: Mapping[str, str],
    reverse_manifests: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate an in-memory repository snapshot with production rules."""

    errors: list[str] = []
    _check_native_manifest_data(
        native_manifest, native_main, native_has_lib, errors
    )
    _check_unsafe_sources(rust_sources, errors)
    _check_safe_root_sources(safe_roots, errors)
    _check_native_source_layout(native_sources, errors)
    _check_forbidden_source_map(native_sources, errors)
    _check_reverse_manifest_data(reverse_manifests, errors)
    return errors


def validate_repository(repo_root: Path) -> list[str]:
    """Return every native-boundary violation under ``repo_root``."""

    root = repo_root.resolve()
    errors: list[str] = []
    native_root = root / NATIVE_RELATIVE
    manifest_path = native_root / "Cargo.toml"
    if not manifest_path.is_file():
        return [f"missing native bridge manifest: {_display(root, manifest_path)}"]

    manifest = _load_toml(root, manifest_path, errors)
    if manifest is not None:
        _check_native_manifest(root, native_root, manifest, errors)
        _check_reverse_dependencies(root, errors)

    _check_unsafe_boundary(root, errors)
    _check_safe_crate_roots(root, errors)
    _check_forbidden_native_names(root, native_root, errors)
    return errors


def _load_toml(
    root: Path, path: Path, errors: list[str]
) -> Mapping[str, Any] | None:
    try:
        with path.open("rb") as source:
            value = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        errors.append(f"unable to parse {_display(root, path)}: {error}")
        return None
    if not isinstance(value, dict):
        errors.append(f"manifest is not a TOML table: {_display(root, path)}")
        return None
    return value


def _check_native_manifest(
    root: Path,
    native_root: Path,
    manifest: Mapping[str, Any],
    errors: list[str],
) -> None:
    main_path = native_root / "src/main.rs"
    main_source = _read_text(root, main_path, errors)
    if main_source is None:
        return
    _check_native_manifest_data(
        manifest, main_source, (native_root / "src/lib.rs").exists(), errors
    )


def _check_native_manifest_data(
    manifest: Mapping[str, Any],
    main_source: str,
    native_has_lib: bool,
    errors: list[str],
) -> None:
    package = manifest.get("package")
    if not isinstance(package, dict):
        errors.append("native bridge manifest is missing [package]")
        return
    if package.get("name") != NATIVE_CRATE:
        errors.append("native bridge package name is not exact")
    if package.get("publish") is not False:
        errors.append("native bridge package must set publish = false")

    bins = manifest.get("bin")
    if not isinstance(bins, list) or len(bins) != 1 or not isinstance(bins[0], dict):
        errors.append("native bridge must declare exactly one [[bin]] target")
    else:
        binary = bins[0]
        if binary.get("name") != NATIVE_CRATE or binary.get("path") != "src/main.rs":
            errors.append("native bridge [[bin]] name/path must be exact")
    if "lib" in manifest or native_has_lib:
        errors.append("native bridge must remain bin-only")

    rust_lints = manifest.get("lints", {}).get("rust", {})
    if not isinstance(rust_lints, dict):
        rust_lints = {}
    if rust_lints.get("unsafe_code") != "deny":
        errors.append("native bridge manifest must set unsafe_code = \"deny\"")
    if rust_lints.get("unsafe_op_in_unsafe_fn") != "deny":
        errors.append(
            "native bridge manifest must set unsafe_op_in_unsafe_fn = \"deny\""
        )

    for dependency_name, specification in _dependency_entries(manifest):
        if dependency_name not in ALLOWED_NATIVE_DEPENDENCIES:
            errors.append(
                "native discovery helper dependency is not allowed: "
                + dependency_name
            )
        if isinstance(specification, dict) and "path" in specification:
            errors.append(
                "native bridge must not use path dependencies: " + dependency_name
            )

    if "#![deny(unsafe_code)]" not in main_source:
        errors.append("native bridge main.rs must deny unsafe_code")
    if "#![deny(unsafe_op_in_unsafe_fn)]" not in main_source:
        errors.append("native bridge main.rs must deny unsafe_op_in_unsafe_fn")


def _check_unsafe_boundary(root: Path, errors: list[str]) -> None:
    crates_root = root / "crates"
    if not crates_root.is_dir():
        errors.append("missing crates directory")
        return
    sources: dict[str, str] = {}
    for path in sorted(crates_root.rglob("*.rs")):
        source = _read_text(root, path, errors)
        if source is None:
            continue
        sources[_display(root, path)] = source
    _check_unsafe_sources(sources, errors)


def _check_unsafe_sources(sources: Mapping[str, str], errors: list[str]) -> None:
    allowed = ALLOWED_UNSAFE_RELATIVE.as_posix()
    for path, source in sorted(sources.items()):
        sanitized = _rust_code_without_comments_or_literals(source)
        if UNSAFE_TOKEN.search(sanitized) and path.replace("\\", "/") != allowed:
            errors.append("unsafe Rust is outside the approved discovery boundary: " + path)

    discovery_source = sources.get(allowed)
    if discovery_source is None:
        errors.append("missing approved discovery boundary: " + allowed)
        return
    if discovery_source is not None and (
        "#![cfg_attr(windows, allow(unsafe_code))]" not in discovery_source
    ):
        errors.append(
            "approved discovery boundary must scope allow(unsafe_code) to Windows"
        )


def _check_safe_crate_roots(root: Path, errors: list[str]) -> None:
    sources: dict[str, str] = {}
    for relative in SAFE_CRATE_ROOTS:
        path = root / relative
        if not path.exists():
            continue
        source = _read_text(root, path, errors)
        if source is not None:
            sources[relative.as_posix()] = source
    _check_safe_root_sources(sources, errors)


def _check_safe_root_sources(
    sources: Mapping[str, str], errors: list[str]
) -> None:
    for path, source in sources.items():
        if "#![forbid(unsafe_code)]" not in source:
            errors.append("safe crate root must forbid unsafe_code: " + path)


def _check_forbidden_native_names(
    root: Path, native_root: Path, errors: list[str]
) -> None:
    sources: dict[str, str] = {}
    for path in sorted((native_root / "src").rglob("*.rs")):
        source = _read_text(root, path, errors)
        if source is None:
            continue
        sources[_display(root, path)] = source
    _check_native_source_layout(sources, errors)
    _check_forbidden_source_map(sources, errors)


def _check_native_source_layout(
    sources: Mapping[str, str], errors: list[str]
) -> None:
    for path in sorted(sources):
        normalized = path.replace("\\", "/")
        if normalized not in ALLOWED_NATIVE_SOURCES:
            errors.append(
                "native discovery helper has an unapproved source file: " + path
            )


def _check_forbidden_source_map(
    sources: Mapping[str, str], errors: list[str]
) -> None:
    for path, source in sorted(sources.items()):
        for token in FORBIDDEN_NATIVE_LOADING_TOKENS:
            if token in source:
                errors.append("native module loading is prohibited: " + token + " in " + path)
        for token in FORBIDDEN_LEGACY_NATIVE_TOKENS:
            if token in source:
                errors.append(
                    "legacy native compatibility artifact is prohibited: "
                    + token
                    + " in "
                    + path
                )
        for token in FORBIDDEN_NATIVE_NETWORK_TOKENS:
            if token in source:
                errors.append(
                    "network access is prohibited in the discovery helper: "
                    + token
                    + " in "
                    + path
                )
        for token in FORBIDDEN_LOCALIZED_VERSION_TOKENS:
            if token in source:
                errors.append(
                    "localized executable version lookup is prohibited: "
                    + token
                    + " in "
                    + path
                )
        for forbidden in FORBIDDEN_ACCOUNT_OR_TRADING_NAMES:
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(forbidden)}(?![A-Za-z0-9_])", source):
                errors.append(
                    "forbidden account/trading native name "
                    + forbidden
                    + " in "
                    + path
                )


def _check_reverse_dependencies(root: Path, errors: list[str]) -> None:
    manifests: dict[str, Mapping[str, Any]] = {}
    for crate_name in (
        "magic-market-core",
        "magic-market-monitor",
        "magic-tdx-local-rs",
        "magic-market-monitor-server",
    ):
        manifest_path = root / "crates" / crate_name / "Cargo.toml"
        if not manifest_path.exists():
            continue
        manifest = _load_toml(root, manifest_path, errors)
        if manifest is None:
            continue
        manifests[crate_name] = manifest
    _check_reverse_manifest_data(manifests, errors)


def _check_reverse_manifest_data(
    manifests: Mapping[str, Mapping[str, Any]], errors: list[str]
) -> None:
    for crate_name, manifest in manifests.items():
        for dependency_name, specification in _dependency_entries(manifest):
            package_name = (
                specification.get("package")
                if isinstance(specification, dict)
                else None
            )
            path_value = (
                specification.get("path") if isinstance(specification, dict) else None
            )
            path_targets_native = (
                isinstance(path_value, str)
                and Path(path_value.replace("\\", "/")).name == NATIVE_CRATE
            )
            if (
                dependency_name == NATIVE_CRATE
                or package_name == NATIVE_CRATE
                or path_targets_native
            ):
                errors.append(
                    f"{crate_name} must not depend on the native bridge binary"
                )


def _dependency_entries(manifest: Mapping[str, Any]) -> Iterable[tuple[str, Any]]:
    for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = manifest.get(table_name)
        if isinstance(table, dict):
            yield from table.items()
    targets = manifest.get("target")
    if isinstance(targets, dict):
        for target in targets.values():
            if not isinstance(target, dict):
                continue
            for table_name in (
                "dependencies",
                "dev-dependencies",
                "build-dependencies",
            ):
                table = target.get(table_name)
                if isinstance(table, dict):
                    yield from table.items()


def _read_text(root: Path, path: Path, errors: list[str]) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"unable to read {_display(root, path)}: {error}")
        return None


def _display(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root).as_posix()
    except ValueError:
        return str(path)


def _rust_code_without_comments_or_literals(source: str) -> str:
    """Return Rust code with comments and string/character contents blanked."""

    output = list(source)
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = length if end < 0 else end
            _blank(output, index, end)
            index = end
            continue
        if source.startswith("/*", index):
            start = index
            index += 2
            depth = 1
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            _blank(output, start, index)
            continue

        raw = _raw_string_delimiter(source, index)
        if raw is not None:
            content_start, closing = raw
            end = source.find(closing, content_start)
            end = length if end < 0 else end + len(closing)
            _blank(output, index, end)
            index = end
            continue
        if source[index] == '"':
            start = index
            index += 1
            while index < length:
                if source[index] == "\\":
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            _blank(output, start, min(index, length))
            continue
        if source[index] == "'" and _looks_like_character_literal(source, index):
            start = index
            index += 1
            while index < length:
                if source[index] == "\\":
                    index += 2
                elif source[index] == "'":
                    index += 1
                    break
                else:
                    index += 1
            _blank(output, start, min(index, length))
            continue
        index += 1
    return "".join(output)


def _raw_string_delimiter(source: str, index: int) -> tuple[int, str] | None:
    raw_start = index
    if source.startswith("br", index):
        index += 2
    elif source.startswith("r", index):
        index += 1
    else:
        return None
    hashes = 0
    while index < len(source) and source[index] == "#":
        hashes += 1
        index += 1
    if index >= len(source) or source[index] != '"':
        return None
    if raw_start > 0 and (source[raw_start - 1].isalnum() or source[raw_start - 1] == "_"):
        return None
    return index + 1, '"' + ("#" * hashes)


def _looks_like_character_literal(source: str, index: int) -> bool:
    if index + 2 >= len(source):
        return False
    if source[index + 1] == "\\":
        return source.find("'", index + 3, min(len(source), index + 12)) >= 0
    return source[index + 2] == "'"


def _blank(output: list[str], start: int, end: int) -> None:
    for index in range(start, end):
        if output[index] != "\n":
            output[index] = " "


def main(arguments: list[str]) -> int:
    if len(arguments) > 1:
        print("usage: check_tdx_native_boundary.py [repository-root]", file=sys.stderr)
        return 2
    default_root = Path(__file__).resolve().parents[2]
    root = Path(arguments[0]) if arguments else default_root
    errors = validate_repository(root)
    if errors:
        for error in errors:
            print(f"tdx-native-boundary: {error}", file=sys.stderr)
        return 1
    print("tdx-native-boundary: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
