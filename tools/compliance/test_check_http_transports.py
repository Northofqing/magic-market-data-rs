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
        self.write_workspace("crates/*")
        subprocess.run(
            ["git", "init", "-q", str(self.root)], check=True, capture_output=True
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_workspace(self, *members: str) -> None:
        quoted = ", ".join(f'"{member}"' for member in members)
        (self.root / "Cargo.toml").write_text(
            f"[workspace]\nmembers = [{quoted}]\nresolver = \"2\"\n",
            encoding="utf-8",
        )

    def manifest_at(
        self, relative: str, crate: str, dependencies: str = ""
    ) -> None:
        path = self.root / relative
        path.mkdir(parents=True, exist_ok=True)
        (path / "Cargo.toml").write_text(
            f'[package]\nname = "{crate}"\nversion = "0.1.0"\n'
            f"\n[dependencies]\n{dependencies}",
            encoding="utf-8",
        )

    def manifest(self, crate: str, dependencies: str = "") -> None:
        self.manifest_at(f"crates/{crate}", crate, dependencies)

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
            "workspace member manifest is not Git-tracked",
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

    def test_nested_target_specific_and_renamed_dependencies_are_discovered(
        self,
    ) -> None:
        self.write_workspace(
            "providers/region/nested",
            "crates/target-specific",
            "crates/renamed",
        )
        self.manifest_at(
            "providers/region/nested",
            "nested-provider",
            'reqwest = "1"\n',
        )
        self.manifest_at(
            "crates/target-specific",
            "target-provider",
            "\n[target.'cfg(unix)'.dependencies]\nureq = \"2\"\n",
        )
        self.manifest_at(
            "crates/renamed",
            "renamed-provider",
            'http = { package = "reqwest", version = "1" }\n',
        )
        self.write_rows()
        errors = "\n".join(self.errors())
        self.assertIn(
            "HTTP transport crate missing from registry: nested-provider",
            errors,
        )
        self.assertIn(
            "HTTP transport crate missing from registry: target-provider",
            errors,
        )
        self.assertIn(
            "HTTP transport crate missing from registry: renamed-provider",
            errors,
        )

    def test_extra_and_missing_registry_fields_are_diagnostics(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        values = [self.row()[field] for field in CHECKER.FIELDS]
        self.registry.write_text(
            "\t".join(CHECKER.FIELDS)
            + "\n"
            + "\t".join(values)
            + "\textra\n",
            encoding="utf-8",
        )
        self.assertIn("extra field", "\n".join(self.errors()))

        self.registry.write_text(
            "\t".join(CHECKER.FIELDS) + "\n" + "\t".join(values[:-1]) + "\n",
            encoding="utf-8",
        )
        self.assertIn("missing field", "\n".join(self.errors()))

    def test_invalid_utf8_registry_is_a_diagnostic(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        self.registry.write_bytes(b"\xff")
        self.assertIn("cannot read HTTP transport registry", "\n".join(self.errors()))

    def test_malformed_quoted_registry_is_a_diagnostic(self) -> None:
        self.manifest("provider", 'ureq = "2"\n')
        values = [self.row()[field] for field in CHECKER.FIELDS[:-1]]
        self.registry.write_text(
            "\t".join(CHECKER.FIELDS)
            + "\n"
            + "\t".join(values)
            + '\t"unterminated\n',
            encoding="utf-8",
        )
        self.assertIn("cannot parse HTTP transport registry", "\n".join(self.errors()))

    def test_implicit_in_tree_path_dependency_member_is_discovered(self) -> None:
        self.write_workspace("crates/application")
        self.manifest(
            "application",
            'nested = { path = "../../providers/nested" }\n',
        )
        self.manifest_at(
            "providers/nested",
            "implicit-provider",
            'reqwest = "1"\n',
        )
        self.write_rows()
        self.assertIn(
            "HTTP transport crate missing from registry: implicit-provider",
            "\n".join(self.errors()),
        )

    def test_root_package_workspace_does_not_require_members_key(self) -> None:
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "root-provider"\nversion = "0.1.0"\n'
            '\n[workspace]\nresolver = "2"\n'
            '\n[dependencies]\nureq = "2"\n',
            encoding="utf-8",
        )
        self.write_rows(self.row(crate="root-provider"))
        self.assertEqual(self.errors(), [])

    def test_workspace_inherited_path_dependency_member_is_discovered(self) -> None:
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/application"]\nresolver = "2"\n'
            '\n[workspace.dependencies]\n'
            'nested = { path = "providers/nested" }\n',
            encoding="utf-8",
        )
        self.manifest(
            "application",
            "nested = { workspace = true }\n",
        )
        self.manifest_at(
            "providers/nested",
            "inherited-provider",
            'rustls = "1"\n',
        )
        self.write_rows()
        self.assertIn(
            "HTTP transport crate missing from registry: inherited-provider",
            "\n".join(self.errors()),
        )

    def test_excluded_in_tree_path_dependency_is_not_a_workspace_member(self) -> None:
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/application"]\n'
            'exclude = ["providers/nested"]\nresolver = "2"\n',
            encoding="utf-8",
        )
        self.manifest(
            "application",
            'nested = { path = "../../providers/nested" }\n',
        )
        self.manifest_at(
            "providers/nested",
            "excluded-provider",
            'reqwest = "1"\n',
        )
        self.write_rows()
        self.assertEqual(self.errors(), [])

    def test_recursive_path_dependency_cycle_terminates_and_finds_deep_member(
        self,
    ) -> None:
        self.write_workspace("crates/application")
        self.manifest(
            "application",
            'middle = { path = "../../providers/middle" }\n',
        )
        self.manifest_at(
            "providers/middle",
            "middle",
            'deep = { path = "../deep" }\n',
        )
        self.manifest_at(
            "providers/deep",
            "deep-provider",
            'middle = { path = "../middle" }\nreqwest = "1"\n',
        )
        self.write_rows()
        self.assertIn(
            "HTTP transport crate missing from registry: deep-provider",
            "\n".join(self.errors()),
        )

    def test_dev_build_and_target_path_dependency_members_are_discovered(
        self,
    ) -> None:
        self.write_workspace(
            "crates/application-dev",
            "crates/application-build",
            "crates/application-target",
        )
        self.manifest_at(
            "crates/application-dev",
            "application-dev",
            '\n[dev-dependencies]\nprovider = { path = "../../providers/dev" }\n',
        )
        self.manifest_at(
            "crates/application-build",
            "application-build",
            '\n[build-dependencies]\nprovider = { path = "../../providers/build" }\n',
        )
        self.manifest_at(
            "crates/application-target",
            "application-target",
            "\n[target.'cfg(unix)'.dependencies]\n"
            'provider = { path = "../../providers/target" }\n',
        )
        for kind in ("dev", "build", "target"):
            self.manifest_at(
                f"providers/{kind}",
                f"{kind}-provider",
                'ureq = "2"\n',
            )
        self.write_rows()
        errors = "\n".join(self.errors())
        for kind in ("dev", "build", "target"):
            self.assertIn(
                f"HTTP transport crate missing from registry: {kind}-provider",
                errors,
            )

    def test_path_dependency_symlink_loop_is_a_controlled_diagnostic(self) -> None:
        self.write_workspace("crates/application")
        self.manifest(
            "application",
            'loop = { path = "../../providers/loop" }\n',
        )
        providers = self.root / "providers"
        providers.mkdir()
        (providers / "loop").symlink_to("loop", target_is_directory=True)
        self.write_rows()
        self.assertIn(
            "workspace member manifest cannot be resolved",
            "\n".join(self.errors()),
        )


if __name__ == "__main__":
    unittest.main()
