import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from check_thresholds import main


def coverage_file(filename: str, covered: int, count: int) -> dict:
    return {
        "filename": filename,
        "summary": {"lines": {"covered": covered, "count": count}},
    }


class CoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.repo_root = Path(self.temporary_directory.name)

    def source(self, relative_path: str) -> str:
        path = self.repo_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("// production source\n", encoding="utf-8")
        return str(path)

    def report(self, files: list[dict]) -> Path:
        path = self.repo_root / "coverage.json"
        path.write_text(json.dumps({"data": [{"files": files}]}), encoding="utf-8")
        return path

    def check(self, files: list[dict]) -> int:
        return main(str(self.report(files)), self.repo_root)

    def test_overall_79_99_fails(self) -> None:
        source = self.source("crates/x/src/lib.rs")
        self.assertEqual(self.check([coverage_file(source, 7_999, 10_000)]), 1)

    def test_overall_80_00_passes(self) -> None:
        source = self.source("crates/x/src/lib.rs")
        self.assertEqual(self.check([coverage_file(source, 8_000, 10_000)]), 0)

    def test_critical_94_99_fails(self) -> None:
        adapter = self.source("crates/x/src/adapter.rs")
        library = self.source("crates/x/src/lib.rs")
        files = [
            coverage_file(adapter, 9_499, 10_000),
            coverage_file(library, 10_000, 10_000),
        ]
        self.assertEqual(self.check(files), 1)

    def test_critical_95_00_passes(self) -> None:
        adapter = self.source("crates/x/src/adapter.rs")
        library = self.source("crates/x/src/lib.rs")
        files = [
            coverage_file(adapter, 9_500, 10_000),
            coverage_file(library, 10_000, 10_000),
        ]
        self.assertEqual(self.check(files), 0)

    def test_windows_paths_are_normalized(self) -> None:
        self.source("crates/x/src/adapter.rs")
        windows_path = r"C:\repo\crates\x\src\adapter.rs"
        self.assertEqual(
            self.check([coverage_file(windows_path, 9_500, 10_000)]),
            0,
        )

    def test_tests_examples_benches_fuzz_and_target_are_excluded(self) -> None:
        library = self.source("crates/x/src/lib.rs")
        files = [coverage_file(library, 8_000, 10_000)]
        for directory in ("tests", "examples", "benches", "fuzz", "target"):
            files.append(
                coverage_file(
                    f"{self.repo_root}/crates/x/{directory}/ignored.rs",
                    0,
                    10_000,
                )
            )
        self.assertEqual(self.check(files), 0)

    def test_existing_unmeasured_critical_family_fails(self) -> None:
        self.source("crates/x/src/adapter.rs")
        library = self.source("crates/x/src/lib.rs")
        with self.assertRaisesRegex(SystemExit, "adapter.rs.*no measured file"):
            self.check([coverage_file(library, 10_000, 10_000)])

    def test_duplicate_file_record_fails(self) -> None:
        library = self.source("crates/x/src/lib.rs")
        row = coverage_file(library, 10_000, 10_000)
        with self.assertRaisesRegex(SystemExit, "duplicate coverage file"):
            self.check([row, row])

    def test_malformed_json_fails(self) -> None:
        path = self.repo_root / "coverage.json"
        path.write_text("{", encoding="utf-8")
        with self.assertRaisesRegex(SystemExit, "invalid coverage JSON"):
            main(str(path), self.repo_root)

    def test_empty_production_report_fails(self) -> None:
        test_file = f"{self.repo_root}/crates/x/tests/example.rs"
        with self.assertRaisesRegex(SystemExit, "no production lines"):
            self.check([coverage_file(test_file, 1, 1)])

    def test_missing_or_multiple_export_objects_fail(self) -> None:
        for exports in ([], [{}, {}]):
            with self.subTest(exports=len(exports)):
                path = self.repo_root / f"coverage-{len(exports)}.json"
                path.write_text(json.dumps({"data": exports}), encoding="utf-8")
                with self.assertRaisesRegex(SystemExit, "exactly one export object"):
                    main(str(path), self.repo_root)

    def test_non_integer_and_invalid_line_summaries_fail(self) -> None:
        library = self.source("crates/x/src/lib.rs")
        invalid_values = [
            ("covered", "8", 10),
            ("count", 8, 10.0),
            ("negative", -1, 10),
            ("over-covered", 11, 10),
        ]
        for label, covered, count in invalid_values:
            with self.subTest(label=label):
                with self.assertRaisesRegex(SystemExit, "invalid line summary"):
                    self.check([coverage_file(library, covered, count)])


if __name__ == "__main__":
    unittest.main()
