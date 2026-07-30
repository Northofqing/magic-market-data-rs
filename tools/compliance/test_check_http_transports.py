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
            writer = csv.DictWriter(handle, fieldnames=CHECKER.FIELDS, delimiter="\t")
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
                mode="shared",
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

    def test_dependency_drift_missing_unknown_and_duplicate_rows_are_reported(
        self,
    ) -> None:
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

    def test_mode_boolean_status_reason_and_dependency_format_are_validated(
        self,
    ) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        self.write_rows(
            self.row(
                mode="hybrid",
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
