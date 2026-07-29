from __future__ import annotations

import csv
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check_admissions.py")
SPEC = importlib.util.spec_from_file_location("check_admissions", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)


class AdmissionCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "crates/provider/src").mkdir(parents=True)
        (self.root / "docs/integrations").mkdir(parents=True)
        (self.root / "docs/integrations/provider.md").write_text(
            "# Evidence\n", encoding="utf-8"
        )
        self.registry = self.root / "docs/integrations/admissions.tsv"
        subprocess.run(
            ["git", "init", "-q", str(self.root)],
            check=True,
            capture_output=True,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def source(self, value: str = "true", constant: str = "DATA_ADMITTED") -> None:
        (self.root / "crates/provider/src/lib.rs").write_text(
            f"pub const {constant}: bool = {value};\n", encoding="utf-8"
        )

    def row(self, **overrides: str) -> dict[str, str]:
        row = {
            "crate": "provider",
            "provider": "Provider",
            "constant": "DATA_ADMITTED",
            "admitted": "true",
            "evidence": "docs/integrations/provider.md",
            "status": "admitted",
            "last_live_date": "2026-07-29",
            "live_probe_count": "2",
            "serial_load_count": "3",
            "blocker": "-",
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

    def test_valid_admitted_and_blocked_rows(self) -> None:
        self.source()
        blocked = self.row(
            constant="OTHER_ADMITTED",
            admitted="false",
            status="blocked",
            last_live_date="",
            live_probe_count="0",
            serial_load_count="0",
            blocker="official endpoint returns no structured unit",
        )
        with (self.root / "crates/provider/src/lib.rs").open(
            "a", encoding="utf-8"
        ) as handle:
            handle.write("pub const OTHER_ADMITTED: bool = false;\n")
        self.write_rows(self.row(), blocked)
        self.assertEqual(self.errors(), [])

    def test_missing_boolean_drift_and_unknown_rows_are_reported(self) -> None:
        self.source()
        self.write_rows(self.row(admitted="false", status="blocked", blocker="x"))
        errors = "\n".join(self.errors())
        self.assertIn("boolean drift", errors)

        self.write_rows()
        self.assertIn("missing from registry", "\n".join(self.errors()))

        self.source(constant="OTHER_ADMITTED")
        self.write_rows(self.row())
        errors = "\n".join(self.errors())
        self.assertIn("no Rust admission constant", errors)

    def test_duplicate_identity_and_bad_evidence_are_reported(self) -> None:
        self.source()
        duplicate = self.row(crate="another")
        self.write_rows(self.row(), duplicate)
        errors = "\n".join(self.errors())
        self.assertIn("duplicate Provider/capability identity", errors)

        self.write_rows(self.row(evidence="../outside.md"))
        self.assertIn("under docs/integrations", "\n".join(self.errors()))

    def test_untracked_source_and_evidence_are_rejected(self) -> None:
        self.source()
        self.write_rows(self.row())
        self.track()

        (self.root / "crates/provider/src/untracked.rs").write_text(
            "pub const EXTRA_ADMITTED: bool = true;\n", encoding="utf-8"
        )
        self.assertIn("Rust admission source is not Git-tracked", "\n".join(self.errors(track=False)))

        (self.root / "crates/provider/src/untracked.rs").unlink()
        untracked_evidence = self.root / "docs/integrations/untracked.md"
        untracked_evidence.write_text("# Untracked\n", encoding="utf-8")
        self.write_rows(self.row(evidence="docs/integrations/untracked.md"))
        subprocess.run(
            ["git", "-C", str(self.root), "add", str(self.registry)],
            check=True,
            capture_output=True,
        )
        self.assertIn("evidence document is not Git-tracked", "\n".join(self.errors(track=False)))

    def test_symlinked_evidence_is_rejected_even_when_tracked(self) -> None:
        self.source()
        outside = self.root / "outside.md"
        outside.write_text("# Outside\n", encoding="utf-8")
        link = self.root / "docs/integrations/escape.md"
        link.symlink_to(outside)
        self.write_rows(self.row(evidence="docs/integrations/escape.md"))
        self.track()
        self.assertIn("must not be a symbolic link", "\n".join(self.errors(track=False)))

    def test_admitted_thresholds_and_blocked_reason_are_enforced(self) -> None:
        self.source()
        self.write_rows(self.row(live_probe_count="1", serial_load_count="2"))
        errors = "\n".join(self.errors())
        self.assertIn("at least two live probes", errors)
        self.assertIn("at least three serial loads", errors)

        self.source(value="false")
        self.write_rows(
            self.row(
                admitted="false",
                status="blocked",
                last_live_date="",
                live_probe_count="0",
                serial_load_count="0",
            )
        )
        self.assertIn("explicit blocker", "\n".join(self.errors()))


if __name__ == "__main__":
    unittest.main()
