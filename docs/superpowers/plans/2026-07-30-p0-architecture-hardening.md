# P0 Architecture Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore real concurrency in the synchronous TDX connection pool, document safe use of blocking HTTP providers from Tokio, and add a deterministic compliance boundary that prevents unreviewed HTTP-stack proliferation.

**Architecture:** Keep the public Rust API unchanged. The TDX fix shortens only the lifetime of the outer pool-handle mutex by cloning its `Arc` before socket I/O, with a loopback regression proving two requests can be in flight. HTTP migration remains a later Gate A program; this slice records the current infrastructure/shared/legacy/hybrid topology in a tracked TSV and verifies production manifest dependencies with a standard-library Python checker.

**Tech Stack:** Rust standard library TCP/thread synchronization, existing `magic-tdx-rs` pool, Python 3 `csv`/`tomllib`/`unittest`, Bash compliance gates, Markdown documentation.

---

## File map

- Modify `crates/magic-tdx-rs/src/net/client.rs`: add the deterministic
  loopback regression and release the outer pool-handle lock before I/O.
- Create `tools/compliance/test_check_http_transports.py`: deterministic unit
  tests for discovery, registry validation, tracked-file safety, and drift.
- Create `tools/compliance/check_http_transports.py`: read-only manifest/TSV
  checker.
- Create `docs/integrations/http-transports.tsv`: reviewed current HTTP backend
  topology.
- Modify `tools/compliance/check.sh`: require the new artifacts and run the
  checker.
- Create `docs/integrations/async-blocking.md`: Tokio `spawn_blocking`
  integration contract.
- Modify `docs/integrations/README.md`: link the blocking guide and architecture
  registry separately from admission evidence.
- Modify `README.md`: add a concise blocking-I/O warning and guide link.
- Modify `CONTRIBUTING.md`: describe preflight as rolling stable plus locked
  dependencies.
- Modify `AGENTS.md`: point agents to the governing rules and forbid
  unregistered provider-local HTTP stacks.

### Task 1: Prove and fix synchronous TDX pool concurrency

**Files:**

- Modify: `crates/magic-tdx-rs/src/net/client.rs:594-620`
- Test: `crates/magic-tdx-rs/src/net/client.rs` inline `tests` module

- [ ] **Step 1: Add a bounded loopback regression**

Add these imports and helpers to the inline `tests` module:

```rust
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Barrier;

fn write_test_response(stream: &mut TcpStream) {
    let mut response = [0_u8; RSP_HEADER_LEN + 1];
    response[12..14].copy_from_slice(&1_u16.to_le_bytes());
    response[14..16].copy_from_slice(&1_u16.to_le_bytes());
    response[RSP_HEADER_LEN] = 7;
    stream.write_all(&response).unwrap();
}

fn accept_before(
    listener: &TcpListener,
    deadline: Instant,
) -> Option<(TcpStream, std::net::SocketAddr)> {
    loop {
        match listener.accept() {
            Ok(connection) => return Some(connection),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::yield_now();
            }
            Err(error) => panic!("loopback accept failed: {error}"),
        }
    }
}
```

Add the test:

```rust
#[test]
fn blocking_pool_allows_two_in_flight_requests() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();

    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut first, _) =
            accept_before(&listener, deadline).expect("first request must connect");
        first.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut packet = [0_u8; 1];
        first.read_exact(&mut packet).unwrap();

        let second_before_first_response =
            accept_before(&listener, Instant::now() + Duration::from_millis(500));
        if let Some((mut second, _)) = second_before_first_response {
            second.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            second.read_exact(&mut packet).unwrap();
            write_test_response(&mut first);
            write_test_response(&mut second);
            true
        } else {
            write_test_response(&mut first);
            let (mut second, _) = accept_before(
                &listener,
                Instant::now() + Duration::from_secs(2),
            )
            .expect("serialized second request must eventually connect");
            second.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            second.read_exact(&mut packet).unwrap();
            write_test_response(&mut second);
            false
        }
    });

    let client = TdxHqClient::new();
    client.set_auto_retry(false);
    client.set_rate_limit(0);
    client.set_rate_limit_daily(0);
    client.rate_limiter_minute.set_rps(0);
    client.connected.store(true, Ordering::SeqCst);
    *sync::lock(&client.last_server, "test last server").unwrap() =
        Some((address.ip().to_string(), address.port()));
    let config = PoolConfig {
        max_size: 2,
        connect_timeout: 2.0,
        handshake_fn: None,
    };
    *sync::lock(&client.pool, "test pool").unwrap() = Arc::new(
        ConnectionPool::new_single((address.ip().to_string(), address.port()), config),
    );

    let client = Arc::new(client);
    let start = Arc::new(Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let client = Arc::clone(&client);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                client.send_raw_and_recv(&[1])
            })
        })
        .collect();
    start.wait();

    for worker in workers {
        assert_eq!(worker.join().unwrap().unwrap(), vec![7]);
    }
    assert!(
        server.join().unwrap(),
        "the second pool slot was not usable while the first request awaited a response"
    );
}
```

- [ ] **Step 2: Run the regression and verify the current implementation fails**

Run:

```bash
cargo test -p magic-tdx-rs net::client::tests::blocking_pool_allows_two_in_flight_requests -- --exact
```

Expected: FAIL at the final assertion because the second request cannot borrow a
connection until the first response releases `TdxHqClient.pool`.

- [ ] **Step 3: Make the minimal lock-lifetime fix**

Replace the pool acquisition in `try_send_and_recv`:

```rust
let pool = Arc::clone(&sync::lock(&self.pool, "connection pool handle")?);
let mut guard = pool.borrow(&server)?;
```

The temporary outer `MutexGuard` now ends at the semicolon. The
`PooledConnGuard` borrows the independently owned local `Arc`, which remains
alive until the request completes.

- [ ] **Step 4: Run focused TDX checks**

Run:

```bash
cargo test -p magic-tdx-rs net::client::tests::blocking_pool_allows_two_in_flight_requests -- --exact
cargo test -p magic-tdx-rs net::client::tests
cargo clippy -p magic-tdx-rs --all-targets -- -D warnings
```

Expected: all commands pass; the new regression returns body `[7]` from both
requests and observes the second request before the first response.

- [ ] **Step 5: Commit the concurrency fix**

```bash
git add crates/magic-tdx-rs/src/net/client.rs
git commit -m "fix: restore blocking TDX pool concurrency"
```

### Task 2: Test and implement the HTTP transport boundary checker

**Files:**

- Create: `tools/compliance/test_check_http_transports.py`
- Create: `tools/compliance/check_http_transports.py`

- [ ] **Step 1: Write checker tests before the checker exists**

Create `tools/compliance/test_check_http_transports.py`:

```python
from __future__ import annotations

import csv
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check_http_transports.py")
SPEC = importlib.util.spec_from_file_location("check_http_transports", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)


class HttpTransportCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "crates").mkdir()
        (self.root / "docs/integrations").mkdir(parents=True)
        self.registry = self.root / "docs/integrations/http-transports.tsv"
        subprocess.run(
            ["git", "init", "-q", str(self.root)], check=True, capture_output=True
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def manifest(self, crate: str, dependencies: str = "") -> None:
        path = self.root / "crates" / crate
        path.mkdir(parents=True, exist_ok=True)
        (path / "Cargo.toml").write_text(
            f'[package]\nname = "{crate}"\nversion = "0.1.0"\n'
            f"\n[dependencies]\n{dependencies}",
            encoding="utf-8",
        )

    def row(self, **overrides: str) -> dict[str, str]:
        row = {
            "crate": "provider",
            "mode": "legacy-direct",
            "direct_dependencies": "ureq",
            "shared_transport": "false",
            "migration_status": "legacy",
            "reason": "existing provider-local ureq stack; migrate separately",
        }
        row.update(overrides)
        return row

    def write_rows(self, *rows: dict[str, str]) -> None:
        with self.registry.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.DictWriter(
                handle, fieldnames=CHECKER.FIELDS, delimiter="\t"
            )
            writer.writeheader()
            writer.writerows(rows)

    def track(self) -> None:
        subprocess.run(
            ["git", "-C", str(self.root), "add", "-A"],
            check=True,
            capture_output=True,
        )

    def errors(self, *, track: bool = True) -> list[str]:
        if track:
            self.track()
        return CHECKER.validate(self.root, self.registry)

    def test_valid_infrastructure_shared_legacy_and_hybrid_rows(self) -> None:
        self.manifest(
            "magic-market-transport",
            'reqwest = "1"\nrustls = "1"\n',
        )
        self.manifest(
            "shared-provider",
            'magic-market-transport = { path = "../magic-market-transport" }\n',
        )
        self.manifest("provider", 'ureq = "2"\n')
        self.manifest(
            "hybrid-provider",
            'magic-market-transport = { path = "../magic-market-transport" }\n'
            'ureq = "2"\n',
        )
        self.write_rows(
            self.row(
                crate="magic-market-transport",
                mode="infrastructure",
                direct_dependencies="reqwest,rustls",
                migration_status="target",
                reason="-",
            ),
            self.row(
                crate="shared-provider",
                mode="hybrid",
                direct_dependencies="-",
                shared_transport="true",
                migration_status="target",
                reason="-",
            ),
            self.row(),
            self.row(
                crate="hybrid-provider",
                mode="hybrid",
                shared_transport="true",
                reason="existing split stack; consolidate separately",
            ),
        )
        self.assertEqual(self.errors(), [])

    def test_dependency_drift_missing_unknown_and_duplicate_rows_are_reported(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        self.write_rows(self.row(direct_dependencies="ring,ureq"))
        self.assertIn("direct dependency drift", "\n".join(self.errors()))

        self.write_rows()
        self.assertIn("missing from registry", "\n".join(self.errors()))

        self.manifest("unrelated")
        self.write_rows(self.row(crate="unrelated", direct_dependencies="-"))
        self.assertIn("no discovered HTTP transport", "\n".join(self.errors()))

        self.write_rows(self.row(), self.row())
        self.assertIn("duplicate registry key", "\n".join(self.errors()))

    def test_mode_boolean_status_reason_and_dependency_format_are_validated(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        self.write_rows(
            self.row(
                mode="shared",
                direct_dependencies="ureq,ureq",
                shared_transport="yes",
                migration_status="target",
                reason="-",
            )
        )
        errors = "\n".join(self.errors())
        self.assertIn("direct_dependencies", errors)
        self.assertIn("shared_transport must be true or false", errors)
        self.assertIn("mode drift", errors)
        self.assertIn("legacy or reviewed-exception", errors)
        self.assertIn("explicit reason", errors)

    def test_untracked_manifest_and_registry_symlink_are_rejected(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        self.write_rows(self.row())
        self.track()
        self.manifest("untracked", 'ureq = "2"\n')
        self.assertIn(
            "HTTP transport manifest is not Git-tracked",
            "\n".join(self.errors(track=False)),
        )

        self.temporary.cleanup()
        self.setUp()
        self.manifest("provider", 'ureq = "2"\n')
        outside = self.root / "outside.tsv"
        outside.write_text("\t".join(CHECKER.FIELDS) + "\n", encoding="utf-8")
        self.registry.symlink_to(outside)
        self.track()
        self.assertIn(
            "must not be a symbolic link", "\n".join(self.errors(track=False))
        )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the test and verify the missing checker fails**

Run:

```bash
python3 -m unittest tools.compliance.test_check_http_transports
```

Expected: ERROR with `FileNotFoundError` for
`tools/compliance/check_http_transports.py`.

- [ ] **Step 3: Implement the read-only checker**

Create `tools/compliance/check_http_transports.py`:

```python
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


def classify(
    crate: str, direct: tuple[str, ...], shared: bool
) -> str:
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


def parse_direct_dependencies(value: str, label: str) -> tuple[tuple[str, ...], list[str]]:
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
        safe_repository_file(
            root, registry_path, tracked, "HTTP transport registry"
        )
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
```

- [ ] **Step 4: Run checker unit tests**

Run:

```bash
python3 -m unittest tools.compliance.test_check_http_transports
```

Expected: four tests pass without network access.

- [ ] **Step 5: Commit the checker and tests**

```bash
git add tools/compliance/check_http_transports.py tools/compliance/test_check_http_transports.py
git commit -m "build: add HTTP transport boundary checker"
```

### Task 3: Register the current HTTP topology and wire the gate

**Files:**

- Create: `docs/integrations/http-transports.tsv`
- Modify: `tools/compliance/check.sh:4-85`

- [ ] **Step 1: Create the reviewed registry**

Create `docs/integrations/http-transports.tsv` with literal tab separators:

```tsv
crate	mode	direct_dependencies	shared_transport	migration_status	reason
magic-baidu-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-cfets-rs	shared	-	true	target	-
magic-cls-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-cninfo-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-eastmoney-rs	legacy-direct	ring,ureq	false	legacy	existing provider-local ureq and ring stack; migrate behind shared transport
magic-exchange-rs	hybrid	ureq	true	legacy	official exchange paths are split between shared transport and legacy ureq
magic-fred-rs	shared	-	true	target	-
magic-gov-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-imf-rs	shared	-	true	target	-
magic-iwencai-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-jin10-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-market-transport	infrastructure	reqwest,rustls	false	target	-
magic-nbs-rs	shared	-	true	target	-
magic-pbc-rs	shared	-	true	target	-
magic-sec-rs	shared	-	true	target	-
magic-sina-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-stcn-rs	shared	-	true	target	-
magic-tencent-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-thepaper-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-ths-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-wallstreetcn-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
magic-worldbank-rs	shared	-	true	target	-
magic-xinhua-rs	shared	-	true	target	-
magic-yicai-rs	shared	-	true	target	-
magic-yonhap-rs	legacy-direct	ureq	false	legacy	existing provider-local ureq stack; migrate behind shared transport
```

- [ ] **Step 2: Verify the checker rejects the untracked registry**

Run:

```bash
python3 tools/compliance/check_http_transports.py
```

Expected: FAIL with `HTTP transport registry is not Git-tracked`.

- [ ] **Step 3: Wire the registry and checker into compliance**

Add these required files after the existing integration index entries in
`tools/compliance/check.sh`:

```bash
  docs/integrations/async-blocking.md
  docs/integrations/http-transports.tsv
```

Invoke the checker immediately after the admissions checker:

```bash
python3 tools/compliance/check_admissions.py
python3 tools/compliance/check_http_transports.py
```

Stage the registry temporarily so the track-safety contract can be exercised:

```bash
git add docs/integrations/http-transports.tsv
```

- [ ] **Step 4: Run the registry and compliance checks**

The full compliance script will initially fail because Task 4 has not yet
created `async-blocking.md`. First run the direct deterministic checker:

```bash
python3 tools/compliance/check_http_transports.py
python3 -m unittest tools.compliance.test_check_http_transports
```

Expected: checker reports 25 registered crates and all unit tests pass.

- [ ] **Step 5: Commit the registry gate**

```bash
git add docs/integrations/http-transports.tsv tools/compliance/check.sh
git commit -m "build: enforce reviewed HTTP transport boundaries"
```

### Task 4: Document blocking-provider integration and rolling stable

**Files:**

- Create: `docs/integrations/async-blocking.md`
- Modify: `docs/integrations/README.md`
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add the Tokio blocking-integration guide**

Create `docs/integrations/async-blocking.md`:

````markdown
# 在异步服务中调用同步 Provider

当前 HTTP Provider 客户端执行同步阻塞 I/O：共享
`magic-market-transport` 使用 `reqwest::blocking`，其余已登记 Provider 仍可能
使用同步 `ureq`；transport 节流也会阻塞当前线程。完整边界见
[`http-transports.tsv`](docs/integrations/http-transports.tsv)。

不要在 Tokio executor worker 上直接调用这些客户端。把客户端 clone、请求数据和
阻塞调用一起移入 `tokio::task::spawn_blocking`：

```rust
use magic_market_core::{
    AssetClass, Exchange, InstrumentId, RealtimeQuotes,
};
use magic_tencent_rs::{TencentClient, TencentError};

async fn tencent_quote(
    client: TencentClient,
    instrument: InstrumentId,
) -> Result<magic_market_core::DataBatch<magic_market_core::Quote>, Box<dyn std::error::Error>> {
    let batch = tokio::task::spawn_blocking(move || {
        client.realtime_quotes(&[instrument])
    })
    .await? // JoinError：任务 panic 或 runtime 关闭
    .map_err(|error: TencentError| -> Box<dyn std::error::Error> {
        Box::new(error)
    })?; // Provider typed error
    Ok(batch)
}

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = TencentClient::new()?;
let instrument =
    InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity)?;
let batch = tencent_quote(client, instrument).await?;
# let _ = batch;
# Ok(())
# }
```

`spawn_blocking` 只保护异步 executor，不提供请求取消：future 被丢弃后，已经开始的
socket 调用仍会运行到完成或超时。必须给客户端配置有界连接、读、写超时。

常驻服务还应在外层用 `Semaphore`、有界工作队列或服务级并发限制约束
`spawn_blocking` 数量；不要把 Tokio 的 blocking 线程池当作 Provider 限频器。业务
服务继续负责熔断、缓存、持久化和指标，本仓库保留 typed failure 与 provenance。
````

- [ ] **Step 2: Link the guide and registry from the integration index**

Append to `docs/integrations/README.md`:

```markdown
## Runtime and architecture boundaries

- [`async-blocking.md`](docs/integrations/async-blocking.md) explains how to call the current
  synchronous HTTP providers from Tokio without blocking executor workers.
- [`http-transports.tsv`](docs/integrations/http-transports.tsv) records the reviewed production
  HTTP dependency boundary and is checked against tracked Cargo manifests.

The HTTP transport registry is an architecture-control inventory. It does not
grant provider admission; `admissions.tsv` remains the BR-009 capability
evidence index.
```

- [ ] **Step 3: Add the concise root README warning**

After the paragraph ending in “本项目负责源适配、数据校验、证据保留和显式切源。” add:

```markdown
当前 HTTP Provider API 是同步阻塞接口；在 Tokio/Axum 服务中必须通过
`spawn_blocking` 和服务级并发预算调用。参见
[异步服务集成指南](docs/integrations/async-blocking.md)；当前共享、遗留与混合
HTTP 栈由 [transport registry](docs/integrations/http-transports.tsv) 机械校验。
```

- [ ] **Step 4: Correct rolling-stable wording**

Replace the preflight introduction in `CONTRIBUTING.md` with:

```markdown
The same deterministic gates can be run with the current default/rolling-stable
toolchain and locked, pre-fetched dependencies:
```

- [ ] **Step 5: Expand the agent instructions**

Replace `AGENTS.md` with:

```markdown
# Engineering rules

Run formatting, tests, Clippy, compliance, and documentation checks before
release. Preserve explicit failures and provenance; do not add downstream path
dependencies. Changes follow Gates A through D and registered business rules.

Before changing contracts or architecture, read
[`docs/ENGINEERING_RULES.md`](docs/ENGINEERING_RULES.md) and
[`docs/business_rules.md`](docs/business_rules.md). Provider admission evidence
is governed by
[`docs/integrations/admissions.tsv`](docs/integrations/admissions.tsv).

HTTP dependencies are governed by
[`docs/integrations/http-transports.tsv`](docs/integrations/http-transports.tsv).
Do not add or widen a provider-local HTTP/TLS dependency, bypass endpoint
allowlists, or weaken timeout/body/redirect policy without an approved Gate A
design and matching registry update. HTTP Provider calls are currently blocking;
follow [`docs/integrations/async-blocking.md`](docs/integrations/async-blocking.md)
when integrating them with an async runtime.
```

- [ ] **Step 6: Run documentation and compliance checks**

Stage the new guide because both checkers intentionally reject untracked
architecture/evidence files:

```bash
git add AGENTS.md CONTRIBUTING.md README.md docs/integrations/README.md docs/integrations/async-blocking.md
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
```

Expected: both scripts pass; compliance prints the BR-009 result and
`HTTP transport boundary passed: 25 registered crates`.

- [ ] **Step 7: Commit the documentation**

```bash
git commit -m "docs: document blocking provider integration"
```

### Task 5: Run the complete deterministic release gate

**Files:**

- Verify all files changed in Tasks 1–4

- [ ] **Step 1: Format and verify formatting**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
```

Expected: formatting check passes with no diff.

- [ ] **Step 2: Run all workspace tests**

Run:

```bash
cargo test --workspace --all-targets --all-features --locked --offline -- --test-threads=1
```

Expected: all deterministic tests pass without live network access.

- [ ] **Step 3: Run workspace Clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Build documentation**

Run:

```bash
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked --offline
cargo test --workspace --all-features --doc --locked --offline -- --test-threads=1
bash tools/docs/check_links.sh
```

Expected: rustdoc and repository link checks pass.

- [ ] **Step 5: Run compliance and release preflight**

Run:

```bash
python3 -m unittest discover -s tools/compliance -p 'test_*.py'
bash tools/compliance/check.sh
bash tools/release/preflight.sh
```

Expected: Python checker tests, deterministic compliance, and the
rolling-stable locked/offline preflight all pass. Do not run live probes.

- [ ] **Step 6: Inspect the final diff and commit any formatting-only changes**

Run:

```bash
git status --short
git diff --check
git diff --stat
```

Expected: no unstaged implementation changes, no whitespace errors, and only
the approved P0 files plus local untracked planning state.

If `cargo fmt` changed the TDX file after its task commit:

```bash
git add crates/magic-tdx-rs/src/net/client.rs
git commit -m "style: format P0 architecture hardening"
```

Do not stage `.planning/` session files.
