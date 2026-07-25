import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from check_thresholds import main


def coverage_file(filename: str, covered: int, count: int) -> dict:
    segments = []
    if covered > 0:
        segments.append([1, 1, 1, True, True, False])
    if covered < count:
        segments.append([covered + 1, 1, 0, True, True, False])
    segments.append([count + 1, 1, 0, False, False, False])
    return {
        "filename": filename,
        "segments": segments,
        "summary": {"lines": {"covered": covered, "count": count}},
    }


class CoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.repo_root = Path(self.temporary_directory.name)
        (self.repo_root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/*"]\n',
            encoding="utf-8",
        )

    def source(self, relative_path: str, line_count: int = 10_001) -> str:
        path = self.repo_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        parts = Path(relative_path).parts
        if len(parts) >= 2 and parts[0] == "crates":
            crate_manifest = self.repo_root / parts[0] / parts[1] / "Cargo.toml"
            crate_manifest.write_text(
                f'[package]\nname = "{parts[1]}"\nversion = "0.0.0"\n',
                encoding="utf-8",
            )
        path.write_text("// production source\n" * line_count, encoding="utf-8")
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
        adapter = self.source("crates/x/src/adapter.rs")
        windows_path = adapter.replace("/", "\\")
        self.assertEqual(
            self.check([coverage_file(windows_path, 9_500, 10_000)]),
            0,
        )

    def test_tests_examples_benches_fuzz_and_target_are_excluded(self) -> None:
        library = self.source("crates/x/src/lib.rs")
        files = [coverage_file(library, 8_000, 10_000)]
        for directory in ("tests", "examples", "benches", "fuzz", "target"):
            ignored = self.source(f"crates/x/{directory}/ignored.rs")
            files.append(
                coverage_file(
                    ignored,
                    0,
                    10_000,
                )
            )
        self.assertEqual(self.check(files), 0)

    def test_existing_unmeasured_critical_family_fails(self) -> None:
        self.source("crates/x/src/adapter.rs")
        library = self.source("crates/x/src/lib.rs")
        with self.assertRaisesRegex(SystemExit, "missing expected production source"):
            self.check([coverage_file(library, 10_000, 10_000)])

    def test_outside_repository_file_cannot_inflate_production_coverage(self) -> None:
        self.source("crates/real/src/lib.rs")
        outside_directory = tempfile.TemporaryDirectory()
        self.addCleanup(outside_directory.cleanup)
        outside = Path(outside_directory.name) / "crates/fake/src/lib.rs"
        outside.parent.mkdir(parents=True, exist_ok=True)
        outside.write_text("// external source\n", encoding="utf-8")

        with self.assertRaisesRegex(SystemExit, "outside repository"):
            self.check([coverage_file(str(outside), 10_000, 10_000)])

    def test_omitted_noncritical_source_cannot_shrink_the_denominator(self) -> None:
        covered = self.source("crates/high/src/lib.rs")
        self.source("crates/omitted/src/lib.rs")

        with self.assertRaisesRegex(SystemExit, "missing expected production source"):
            self.check([coverage_file(covered, 10_000, 10_000)])

    def test_non_workspace_source_cannot_enter_the_production_total(self) -> None:
        library = self.source("crates/x/src/lib.rs")
        unregistered = self.source("crates/not-a-member/src/lib.rs")
        (self.repo_root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/x"]\n',
            encoding="utf-8",
        )

        with self.assertRaisesRegex(SystemExit, "not a workspace production source"):
            self.check(
                [
                    coverage_file(library, 8_000, 10_000),
                    coverage_file(unregistered, 10_000, 10_000),
                ]
            )

    def test_cfg_test_lines_cannot_inflate_production_coverage(self) -> None:
        lines = ["fn production() {}" for _ in range(10)]
        lines.extend(["#[cfg(test)]", "mod tests {"])
        lines.extend(["    fn covered_test_helper() {}" for _ in range(98)])
        lines.append("}")
        library = Path(self.source("crates/x/src/lib.rs", line_count=1))
        library.write_text("\n".join(lines) + "\n", encoding="utf-8")
        item = coverage_file(str(library), 107, 110)
        item["segments"] = [
            [1, 1, 1, True, True, False],
            [8, 1, 0, True, True, False],
            [11, 1, 1, True, True, False],
            [111, 1, 0, False, False, False],
        ]

        self.assertEqual(self.check([item]), 1)

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
        self.source("crates/x/src/lib.rs")
        test_file = self.source("crates/x/tests/example.rs")
        with self.assertRaisesRegex(SystemExit, "missing expected production source"):
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
                item = coverage_file(library, 8, 10)
                item["summary"]["lines"]["covered"] = covered
                item["summary"]["lines"]["count"] = count
                with self.assertRaisesRegex(SystemExit, "invalid line summary"):
                    self.check([item])


if __name__ == "__main__":
    unittest.main()
