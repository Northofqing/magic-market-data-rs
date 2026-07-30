from __future__ import annotations

import os
import shutil
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


SOURCE_DIR = Path(__file__).resolve().parent


class ReleaseProfileRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        bench = self.repo / "tools/bench"
        bench.mkdir(parents=True)
        shutil.copy2(SOURCE_DIR / "release_profile.sh", bench / "release_profile.sh")
        shutil.copy2(
            SOURCE_DIR / "compare_release_profiles.py",
            bench / "compare_release_profiles.py",
        )
        (self.repo / "Cargo.toml").write_text(
            "[workspace]\nresolver = \"2\"\n", encoding="utf-8"
        )
        (self.repo / "bench-input.txt").write_text("committed\n", encoding="utf-8")
        (self.repo / ".gitignore").write_text("/target\n", encoding="utf-8")
        subprocess.run(
            ["git", "init", "-q", str(self.repo)], check=True, capture_output=True
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "config", "user.name", "Runner Test"],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repo),
                "config",
                "user.email",
                "runner@example.invalid",
            ],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "add", "-A"], check=True, capture_output=True
        )
        subprocess.run(
            ["git", "-C", str(self.repo), "commit", "-qm", "fixture"],
            check=True,
            capture_output=True,
        )

        self.fake_bin = self.root / "bin"
        self.fake_bin.mkdir()
        self._write_executable(
            self.fake_bin / "rustc",
            "#!/bin/sh\nprintf '%s\\n' 'rustc 1.95.0 (59807616e 2026-04-14)'\n",
        )
        self._write_executable(
            self.fake_bin / "cargo",
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import json
                import os
                import pathlib
                import stat
                import sys

                if sys.argv[1:] == ["--version"]:
                    print("cargo 1.95.0 (f2d3ce0bd 2026-03-21)")
                    raise SystemExit(0)

                manifest_index = sys.argv.index("--manifest-path") + 1
                manifest = pathlib.Path(sys.argv[manifest_index])
                source_root = manifest.parent
                if pathlib.Path.cwd() != pathlib.Path("/"):
                    print("build did not use the controlled root directory", file=sys.stderr)
                    raise SystemExit(11)
                for path in (source_root, manifest, source_root / "bench-input.txt"):
                    if path.stat().st_mode & (stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH):
                        print(f"build source remains writable: {path}", file=sys.stderr)
                        raise SystemExit(12)

                if os.environ.get("FAKE_TRANSIENT_MUTATION") == "1":
                    original = pathlib.Path(os.environ["FAKE_REPO_ROOT"]) / "bench-input.txt"
                    original.write_text("tampered\\n", encoding="utf-8")
                    build_input = (source_root / "bench-input.txt").read_text(
                        encoding="utf-8"
                    )
                    original.write_text("committed\\n", encoding="utf-8")
                    if build_input != "committed\\n":
                        print("build consumed transient worktree mutation", file=sys.stderr)
                        raise SystemExit(9)

                if os.environ.get("FAKE_REQUIRE_ISOLATED_CARGO_HOME") == "1":
                    raw_cargo_home = os.environ.get("CARGO_HOME")
                    if not raw_cargo_home:
                        print("build did not isolate Cargo home", file=sys.stderr)
                        raise SystemExit(10)
                    cargo_home = pathlib.Path(raw_cargo_home)
                    inherited_home = pathlib.Path(os.environ["HOME"]) / ".cargo"
                    if (
                        cargo_home == inherited_home
                        or (cargo_home / "config").exists()
                        or (cargo_home / "config.toml").exists()
                        or cargo_home.stat().st_mode
                        & (stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH)
                    ):
                        print("build consumed inherited Cargo configuration", file=sys.stderr)
                        raise SystemExit(10)

                if (
                    os.environ.get("FAKE_CREATE_ISOLATED_HOME_CONFIG") == "1"
                    and os.environ.get("CARGO_PROFILE_RELEASE_LTO") != "thin"
                ):
                    cargo_home = pathlib.Path(os.environ["CARGO_HOME"])
                    cargo_home.chmod(cargo_home.stat().st_mode | stat.S_IWUSR)
                    (cargo_home / "config.toml").write_text(
                        "[build]\\nrustflags=[]\\n", encoding="utf-8"
                    )

                if (
                    os.environ.get("FAKE_MUTATE_SNAPSHOT") == "1"
                    and os.environ.get("CARGO_PROFILE_RELEASE_LTO") != "thin"
                ):
                    manifest.chmod(manifest.stat().st_mode | stat.S_IWUSR)
                    manifest.write_text("[workspace]\\nmembers = []\\n", encoding="utf-8")

                if (
                    os.environ.get("FAKE_CREATE_ANCESTOR_CONFIG") == "1"
                    and os.environ.get("CARGO_PROFILE_RELEASE_LTO") != "thin"
                ):
                    config = source_root.parent / ".cargo/config.toml"
                    config.parent.mkdir(parents=True, exist_ok=True)
                    config.write_text("[build]\\nrustflags=[]\\n", encoding="utf-8")

                candidate = os.environ.get("CARGO_PROFILE_RELEASE_LTO") == "thin"
                elapsed = 90 if candidate else 100
                workloads = [
                    ("tdx_bar_parse", 20_000, 11),
                    ("json_normalize", 10_000, 12),
                    ("zlib_decompress", 5_000, 13),
                    ("zlib_roundtrip", 2_000, 14),
                ]
                document = {
                    "schema": 1,
                    "workloads": [
                        {
                            "workload": name,
                            "iterations": iterations,
                            "elapsed_ns": elapsed,
                            "throughput_per_second": iterations * 1_000_000_000 / elapsed,
                            "checksum": checksum,
                        }
                        for name, iterations, checksum in workloads
                    ],
                }
                target = pathlib.Path(os.environ["CARGO_TARGET_DIR"])
                binary = target / "release/examples/parse_bench"
                binary.parent.mkdir(parents=True, exist_ok=True)
                encoded = json.dumps(document, separators=(",", ":"))
                binary.write_text(
                    "#!/usr/bin/env python3\\n"
                    f"print({encoded!r})\\n",
                    encoding="utf-8",
                )
                binary.chmod(0o755)

                if candidate and os.environ.get("FAKE_CREATE_UNTRACKED") == "1":
                    config = pathlib.Path(os.environ["FAKE_REPO_ROOT"]) / ".cargo/config.toml"
                    config.parent.mkdir(parents=True, exist_ok=True)
                    config.write_text("[build]\\nrustflags=[]\\n", encoding="utf-8")
                """
            ),
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _write_executable(path: Path, content: str) -> None:
        path.write_text(content, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def run_runner(
        self, artifact: Path, extra_environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            name: value
            for name, value in os.environ.items()
            if not (
                name.startswith("CARGO_")
                or name.startswith("RUST")
                or name.startswith("SCCACHE_")
            )
        }
        environment.update(
            {
                "PATH": f"{self.fake_bin}{os.pathsep}{environment['PATH']}",
                "MAGIC_RELEASE_BENCH_DIR": str(artifact),
                "FAKE_REPO_ROOT": str(self.repo),
            }
        )
        if extra_environment:
            environment.update(extra_environment)
        return subprocess.run(
            ["bash", "tools/bench/release_profile.sh"],
            cwd=self.repo,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_clean_external_artifact_run_is_qualified(self) -> None:
        result = self.run_runner(self.root / "artifacts")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / "artifacts/evidence.json").is_file())
        self.assertIn('"qualified": true', result.stdout)

    def test_untracked_file_created_during_build_fails_closed(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"FAKE_CREATE_UNTRACKED": "1"},
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("benchmark requires a clean worktree", result.stderr)
        self.assertIn(".cargo/", result.stderr)

    def test_inherited_build_environment_is_rejected(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"RUSTFLAGS": "-C target-cpu=native"},
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("RUSTFLAGS", result.stderr)

    def test_unignored_in_repository_artifact_path_is_rejected(self) -> None:
        result = self.run_runner(self.repo / "bench-output")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Git-ignored path", result.stderr)

    def test_transient_tracked_mutation_cannot_affect_snapshot_build(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"FAKE_TRANSIENT_MUTATION": "1"},
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.repo / "bench-input.txt").read_text(encoding="utf-8"),
            "committed\n",
        )

    def test_home_cargo_configuration_is_isolated_from_builds(self) -> None:
        fake_home = self.root / "home"
        cargo_home = fake_home / ".cargo"
        cargo_home.mkdir(parents=True)
        (cargo_home / "config.toml").write_text(
            "[build]\nrustflags = ['-C', 'target-cpu=native']\n",
            encoding="utf-8",
        )
        result = self.run_runner(
            self.root / "artifacts",
            {
                "HOME": str(fake_home),
                "FAKE_REQUIRE_ISOLATED_CARGO_HOME": "1",
            },
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_snapshot_is_read_only_and_digest_checked_after_build(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"FAKE_MUTATE_SNAPSHOT": "1"},
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("source snapshot", result.stderr)

    def test_config_created_in_snapshot_ancestry_fails_after_build(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"FAKE_CREATE_ANCESTOR_CONFIG": "1"},
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("automatic Cargo config", result.stderr)

    def test_config_created_in_isolated_cargo_home_fails_after_build(self) -> None:
        result = self.run_runner(
            self.root / "artifacts",
            {"FAKE_CREATE_ISOLATED_HOME_CONFIG": "1"},
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("isolated Cargo home config", result.stderr)

    def test_ancestor_cargo_configuration_is_rejected(self) -> None:
        cargo_directory = self.root / ".cargo"
        cargo_directory.mkdir()
        (cargo_directory / "config.toml").write_text(
            "[build]\nrustflags = ['-C', 'target-cpu=native']\n",
            encoding="utf-8",
        )
        result = self.run_runner(self.root / "artifacts")
        self.assertEqual(result.returncode, 2)
        self.assertIn("automatic Cargo config", result.stderr)


if __name__ == "__main__":
    unittest.main()
