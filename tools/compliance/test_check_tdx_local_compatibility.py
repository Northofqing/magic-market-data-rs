from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check_tdx_local_compatibility.py")
SPEC = importlib.util.spec_from_file_location("check_tdx_local_compatibility", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)


class TdxLocalCompatibilityCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path.cwd()
        self.registry = self.root / "docs/integrations/tdx-local-terminal-compatibility.tsv"

    def row(self, **overrides: str) -> dict[str, str]:
        row = {
            "profile_id": "fixture-observed",
            "status": "runtime-compatible",
            "tdx_exe_version": "1.0.0.1",
            "tdx_exe_sha256": "a" * 64,
            "pe_arch": "x86_64",
            "transport": "official-tq-local-http",
            "endpoint": "http://127.0.0.1:17709/",
            "method_set": "get_stock_list,get_pricevol,get_market_snapshot",
            "schema_revision": "1",
            "last_live_date": "2026-08-13",
            "live_probe_count": "2",
            "signature_status": "unverified",
            "evidence": "docs/integrations/tdx-local-terminal.md",
            "blocker": "-",
        }
        row.update(overrides)
        return row

    def errors(self, *rows: dict[str, str]) -> str:
        return "\n".join(CHECKER.validate_rows(self.root, self.registry, list(rows)))

    def test_observed_runtime_compatibility_is_valid(self) -> None:
        self.assertEqual(self.errors(self.row()), "")

    def test_empty_registry_and_duplicate_profiles_fail(self) -> None:
        self.assertIn("explicit version evidence", self.errors())
        self.assertIn("duplicate profile_id", self.errors(self.row(), self.row()))

    def test_runtime_compatibility_requires_live_evidence_and_no_blocker(self) -> None:
        errors = self.errors(
            self.row(last_live_date="-", live_probe_count="0", blocker="pending")
        )
        self.assertIn("ISO live date", errors)
        self.assertIn("positive live probes", errors)
        self.assertIn("must not declare a blocker", errors)

    def test_origin_method_hash_architecture_and_evidence_are_strict(self) -> None:
        errors = self.errors(
            self.row(
                tdx_exe_sha256="ABC",
                pe_arch="x86",
                endpoint="http://localhost:17709/",
                method_set="free-form",
                evidence="../outside.md",
            )
        )
        self.assertIn("64 lowercase hex", errors)
        self.assertIn("only the evidenced x86_64", errors)
        self.assertIn("fixed TQ-Local loopback", errors)
        self.assertIn("implemented read-only set", errors)
        self.assertIn("Markdown under docs/integrations", errors)

    def test_blocked_row_requires_blocker(self) -> None:
        self.assertIn(
            "blocked row requires an explicit blocker",
            self.errors(self.row(status="blocked", blocker="-")),
        )


if __name__ == "__main__":
    unittest.main()
