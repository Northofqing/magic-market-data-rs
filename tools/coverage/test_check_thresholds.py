from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from check_thresholds import CRITICAL_GLOBS, main


def coverage_file(filename: str, covered: int, count: int) -> dict[str, object]:
    return {
        "filename": filename,
        "summary": {"lines": {"covered": covered, "count": count}},
    }


class CoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "magic-market-data-rs"
        self.root.mkdir()
        self.report = self.root / "coverage.json"
        self.critical_paths = [
            "crates/magic-market-core/src/batch.rs",
            "crates/magic-market-router/src/router.rs",
            "crates/magic-tdx-rs/src/net/packet.rs",
            "crates/magic-tdx-rs/src/net/utils.rs",
            "crates/magic-tdx-rs/src/protocol/parsers.rs",
            "crates/magic-tdx-rs/src/adapter.rs",
            "crates/magic-tdx-rs/src/service/mod.rs",
            "crates/magic-eastmoney-rs/src/lib.rs",
            "crates/magic-cninfo-rs/src/lib.rs",
            "crates/magic-ths-rs/src/lib.rs",
            "crates/magic-cls-rs/src/lib.rs",
            "crates/magic-baidu-rs/src/lib.rs",
            "crates/magic-iwencai-rs/src/lib.rs",
        ]
        self.noncritical_path = "crates/magic-market-analysis/src/lib.rs"
        for relative in [*self.critical_paths, self.noncritical_path]:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("// measured source\n", encoding="utf-8")

    def write_report(self, files: list[dict[str, object]]) -> None:
        self.report.write_text(
            json.dumps({"data": [{"files": files}]}),
            encoding="utf-8",
        )

    def passing_critical(self) -> list[dict[str, object]]:
        return [coverage_file(path, 95, 100) for path in self.critical_paths]

    def assert_report_status(
        self,
        files: list[dict[str, object]],
        expected: int,
    ) -> None:
        self.write_report(files)
        self.assertEqual(main(str(self.report), repo_root=self.root), expected)

    def test_configured_critical_globs_are_the_reviewed_minimum(self) -> None:
        self.assertEqual(
            CRITICAL_GLOBS,
            (
                "crates/magic-market-core/src/*.rs",
                "crates/magic-market-router/src/*.rs",
                "crates/magic-tdx-rs/src/net/packet.rs",
                "crates/magic-tdx-rs/src/net/utils.rs",
                "crates/magic-tdx-rs/src/protocol/*.rs",
                "crates/magic-tdx-rs/src/adapter.rs",
                "crates/magic-tdx-rs/src/service/mod.rs",
                "crates/magic-eastmoney-rs/src/*.rs",
                "crates/magic-cninfo-rs/src/*.rs",
                "crates/magic-ths-rs/src/*.rs",
                "crates/magic-cls-rs/src/*.rs",
                "crates/magic-baidu-rs/src/*.rs",
                "crates/magic-iwencai-rs/src/*.rs",
            ),
        )

    def test_exact_overall_and_critical_boundaries_pass(self) -> None:
        files = self.passing_critical()
        files.append(coverage_file(self.noncritical_path, 6_765, 8_700))
        self.assert_report_status(files, 0)

    def test_overall_79_99_percent_fails(self) -> None:
        files = self.passing_critical()
        files.append(coverage_file(self.noncritical_path, 6_764, 8_700))
        self.assert_report_status(files, 1)

    def test_critical_94_99_percent_fails(self) -> None:
        files = [
            coverage_file(path, 950, 1_000) for path in self.critical_paths
        ]
        files[0]["summary"]["lines"]["covered"] = 949  # type: ignore[index]
        self.assert_report_status(files, 1)

    def test_every_critical_glob_requires_a_measured_file(self) -> None:
        for missing in range(len(self.critical_paths)):
            with self.subTest(glob=CRITICAL_GLOBS[missing]):
                files = self.passing_critical()
                del files[missing]
                self.assert_report_status(files, 2)

    def test_relative_posix_absolute_and_windows_paths_normalize(self) -> None:
        path_styles = ("relative", "absolute", "windows")
        for style in path_styles:
            with self.subTest(style=style):
                files: list[dict[str, object]] = []
                for relative in self.critical_paths:
                    if style == "relative":
                        filename = relative
                    elif style == "absolute":
                        filename = str(self.root / relative)
                    else:
                        filename = (
                            f"C:\\work\\{self.root.name}\\"
                            + relative.replace("/", "\\")
                        )
                    files.append(coverage_file(filename, 95, 100))
                self.assert_report_status(files, 0)

    def test_nonproduction_and_external_files_cannot_inflate_coverage(self) -> None:
        excluded = [
            "crates/x/tests/fake.rs",
            "crates/x/examples/fake.rs",
            "crates/x/benches/fake.rs",
            "crates/x/fuzz/fake.rs",
            "target/generated.rs",
            "README.rs",
            "crates/x/src/tests/fake.rs",
        ]
        outside = Path(self.temporary.name) / "outside/crates/x/src/fake.rs"
        outside.parent.mkdir(parents=True)
        outside.write_text("// outside\n", encoding="utf-8")
        files = self.passing_critical()
        files.append(coverage_file(self.noncritical_path, 0, 8_800))
        files.extend(coverage_file(path, 10_000, 10_000) for path in excluded)
        files.append(coverage_file(str(outside), 10_000, 10_000))
        self.assert_report_status(files, 1)

    def test_zero_line_critical_file_does_not_satisfy_presence(self) -> None:
        files = self.passing_critical()
        files[0] = coverage_file(self.critical_paths[0], 0, 0)
        self.assert_report_status(files, 2)

    def test_inline_critical_test_bodies_are_invalid_evidence(self) -> None:
        source = self.root / self.critical_paths[0]
        source.write_text(
            "#[cfg(test)]\nmod tests {\n#[test]\nfn inflates() {}\n}\n",
            encoding="utf-8",
        )
        self.assert_report_status(self.passing_critical(), 2)

    def test_path_based_external_critical_tests_are_allowed(self) -> None:
        source = self.root / self.critical_paths[0]
        source.write_text(
            '#[cfg(test)]\n#[path = "../../../tests/batch.rs"]\nmod tests;\n',
            encoding="utf-8",
        )
        self.assert_report_status(self.passing_critical(), 0)

    def test_duplicate_normalized_production_filename_is_invalid(self) -> None:
        files = self.passing_critical()
        files.append(
            coverage_file(str(self.root / self.critical_paths[0]), 95, 100)
        )
        self.assert_report_status(files, 2)

    def test_malformed_empty_and_wrong_typed_reports_are_invalid(self) -> None:
        malformed_payloads: list[object] = [
            None,
            {},
            {"data": []},
            {"data": {}},
            {"data": [{}]},
            {"data": [{"files": {}}]},
            {"data": [{"files": [None]}]},
            {"data": [{"files": [{"filename": 7}]}]},
            {
                "data": [
                    {
                        "files": [
                            {
                                "filename": self.critical_paths[0],
                                "summary": {"lines": {"covered": "95", "count": 100}},
                            }
                        ]
                    }
                ]
            },
            {
                "data": [
                    {
                        "files": [
                            {
                                "filename": self.critical_paths[0],
                                "summary": {"lines": {"covered": True, "count": 100}},
                            }
                        ]
                    }
                ]
            },
            {
                "data": [
                    {
                        "files": [
                            coverage_file(self.critical_paths[0], -1, 100)
                        ]
                    }
                ]
            },
            {
                "data": [
                    {
                        "files": [
                            coverage_file(self.critical_paths[0], 101, 100)
                        ]
                    }
                ]
            },
        ]
        for payload in malformed_payloads:
            with self.subTest(payload=payload):
                self.report.write_text(json.dumps(payload), encoding="utf-8")
                self.assertEqual(main(str(self.report), repo_root=self.root), 2)

        self.report.write_text("{not-json", encoding="utf-8")
        self.assertEqual(main(str(self.report), repo_root=self.root), 2)

    def test_report_with_no_production_lines_is_invalid(self) -> None:
        self.assert_report_status(
            [coverage_file("crates/x/tests/fake.rs", 100, 100)],
            2,
        )


if __name__ == "__main__":
    unittest.main()
