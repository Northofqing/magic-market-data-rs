from __future__ import annotations

import os
import json
import hashlib
import shutil
import stat
import subprocess
import tempfile
import textwrap
import sys
import unittest
from pathlib import Path


SOURCE_DIR = Path(__file__).resolve().parent


class ReleaseProfileRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.artifacts = []
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
        if os.name == "nt":
            self._prepare_windows_fixture()
            return
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
    def _prepare_windows_fixture(self) -> None:
        bench = self.repo / "tools/bench"
        for name in ("release_profile.ps1", "release_profile_native.py", "native_paths.ps1"):
            source = SOURCE_DIR / name
            if source.exists():
                shutil.copy2(source, bench / name)
        (bench / "fixture-tools.json").write_text(
            json.dumps({"schema": 1, "cargo": "tools/bench/fake_cargo.py",
                        "rustc": "tools/bench/fake_rustc.py", "benchmark_suffix": ".py"}),
            encoding="utf-8",
        )
        (bench / "fake_rustc.py").write_text(
            "print('rustc 1.95.0 (59807616e 2026-04-14)')\n", encoding="utf-8"
        )
        (bench / "fake_cargo.py").write_text(textwrap.dedent('''\
            import json
            import os
            import subprocess
            import sys
            from pathlib import Path

            if sys.argv[1:] == ["--version"]:
                print("cargo 1.95.0 (f2d3ce0bd 2026-03-21)")
                raise SystemExit(0)
            manifest = Path(sys.argv[sys.argv.index("--manifest-path") + 1])
            source = manifest.parent
            if Path.cwd() != Path(source.anchor):
                raise SystemExit("build did not use the controlled root directory")
            for target in (manifest, source / "bench-input.txt", source / "unexpected-write"):
                try:
                    with target.open("wb"):
                        pass
                except PermissionError:
                    pass
                else:
                    raise SystemExit("source snapshot remains writable: " + str(target))
            try:
                (source / "unexpected-directory").mkdir()
            except PermissionError:
                pass
            else:
                raise SystemExit("source snapshot allows new directories")
            home = Path(os.environ["CARGO_HOME"])
            with (home / ".package-cache").open("r+b") as lock:
                import msvcrt
                lock.write(b"x")
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            try:
                (home / ".package-cache").unlink()
            except PermissionError:
                pass
            else:
                raise SystemExit("isolated Cargo cache lock allows deletion")
            if os.environ.get("FAKE_REQUIRE_ISOLATED_CARGO_HOME") == "1":
                if home == Path(os.environ["HOME"]) / ".cargo" or any(
                    (home / name).exists() for name in ("config", "config.toml")
                ):
                    raise SystemExit("build consumed inherited Cargo configuration")
                with (home / ".package-cache").open("r+b"):
                    pass
                try:
                    (home / "unapproved-root-file").write_text("bad", encoding="utf-8")
                except PermissionError:
                    pass
                else:
                    raise SystemExit("isolated Cargo home remains writable")
            if os.environ.get("FAKE_TRANSIENT_MUTATION") == "1":
                original = Path(os.environ["FAKE_REPO_ROOT"]) / "bench-input.txt"
                original.write_text("tampered\\n", encoding="utf-8")
                consumed = (source / "bench-input.txt").read_text(encoding="utf-8")
                original.write_text("committed\\n", encoding="utf-8")
                if consumed != "committed\\n":
                    raise SystemExit("build consumed transient worktree mutation")
            candidate = os.environ["CARGO_PROFILE_RELEASE_LTO"] == "thin"
            if not candidate and os.environ.get("FAKE_MUTATE_WORKTREE") == "1":
                (Path(os.environ["FAKE_REPO_ROOT"]) / "bench-input.txt").write_text("dirty", encoding="utf-8")
            if not candidate and os.environ.get("FAKE_MUTATE_INDEX") == "1":
                original = Path(os.environ["FAKE_REPO_ROOT"])
                (original / "bench-input.txt").write_text("staged", encoding="utf-8")
                subprocess.run(["git", "-C", str(original), "add", "bench-input.txt"], check=True,
                               capture_output=True, timeout=10)
            if not candidate and os.environ.get("FAKE_CHANGE_REVISION") == "1":
                subprocess.run(["git", "-C", os.environ["FAKE_REPO_ROOT"], "commit", "--allow-empty", "-qm", "changed"], check=True,
                               capture_output=True, timeout=10)
            if os.environ.get("FAKE_CACHE_IO") == "1":
                for name in ("registry", "git"):
                    with (home / name / "cache-data").open("a", encoding="utf-8") as cache:
                        cache.write("cargo\\n")
            if not candidate and os.environ.get("FAKE_MUTATE_SNAPSHOT") == "1":
                try:
                    manifest.write_text("changed", encoding="utf-8")
                except PermissionError:
                    print("benchmark source snapshot mutation was blocked", file=sys.stderr)
                    raise SystemExit(2)
                print("benchmark source snapshot mutation unexpectedly succeeded", file=sys.stderr)
                raise SystemExit(2)
            if not candidate and os.environ.get("FAKE_CREATE_ISOLATED_HOME_CONFIG") == "1":
                try:
                    (home / "config.toml").write_text("[build]", encoding="utf-8")
                except PermissionError:
                    print("benchmark isolated Cargo home config mutation was blocked", file=sys.stderr)
                    raise SystemExit(2)
                print("benchmark isolated Cargo home config mutation unexpectedly succeeded", file=sys.stderr)
                raise SystemExit(2)
            if not candidate and os.environ.get("FAKE_CREATE_ANCESTOR_CONFIG") == "1":
                config = source.parent / ".cargo/config.toml"
                config.parent.mkdir()
                config.write_text("[build]", encoding="utf-8")
            if not candidate and os.environ.get("FAKE_CHANGE_SNAPSHOT_ACL") == "1":
                icacls = str(Path(os.environ["SystemRoot"]) / "System32/icacls.exe")
                subprocess.run([icacls, str(source), "/reset", "/Q"], check=True,
                               capture_output=True, timeout=10)
            elapsed = 90 if candidate else 100
            workloads = [("tdx_bar_parse", 20000, 11), ("json_normalize", 10000, 12),
                         ("zlib_decompress", 5000, 13), ("zlib_roundtrip", 2000, 14)]
            document = {"schema": 1, "workloads": [
                {"workload": name, "iterations": count, "elapsed_ns": elapsed,
                 "throughput_per_second": count * 1000000000 / elapsed, "checksum": checksum}
                for name, count, checksum in workloads]}
            binary = Path(os.environ["CARGO_TARGET_DIR"]) / "release/examples/parse_bench.py"
            binary.parent.mkdir(parents=True)
            profile = "candidate" if candidate else "default"
            binary.write_text(
                "import json, os\\nfrom pathlib import Path\\n" +
                "if os.environ.get('FAKE_INVOCATION_LOG'):\\n" +
                "    with Path(os.environ['FAKE_INVOCATION_LOG']).open('a', encoding='utf-8') as log:\\n" +
                "        log.write(" + repr(profile + "\\n") + ")\\n" +
                "print(" + repr(json.dumps(document)) + ")\\n", encoding="utf-8")
            if candidate and os.environ.get("FAKE_CREATE_UNTRACKED") == "1":
                config = Path(os.environ["FAKE_REPO_ROOT"]) / ".cargo/config.toml"
                config.parent.mkdir()
                config.write_text("[build]", encoding="utf-8")
            ''').strip() + "\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "add", "-A"], check=True,
                       capture_output=True, timeout=10)
        subprocess.run(["git", "-C", str(self.repo), "commit", "-qm", "native fixture"],
                       check=True, capture_output=True, timeout=10)

    def tearDown(self) -> None:
        if os.name == "nt":
            # Only this test's resolved, owned TemporaryDirectory; never a cache target.
            for artifact in self.artifacts:
                source = artifact / "source"
                if source.is_dir() and not source.is_symlink() and not source.is_junction():
                    if not source.resolve().is_relative_to(self.root.resolve()):
                        raise AssertionError("test cleanup escaped its owned temporary root")
                    subprocess.run([str(Path(os.environ["SystemRoot"]) / "System32/icacls.exe"),
                                    str(source), "/reset", "/T", "/Q"], check=True,
                                   capture_output=True, timeout=15)
                home = artifact / "cargo-home"
                if home.is_dir():
                    if not home.resolve().is_relative_to(self.root.resolve()):
                        raise AssertionError("test cleanup escaped its owned temporary root")
                    subprocess.run([str(Path(os.environ["SystemRoot"]) / "System32/icacls.exe"),
                                    str(home), "/reset", "/Q"], check=True,
                                   capture_output=True, timeout=10)
                    for name in (".package-cache", ".package-cache-mutate"):
                        lock = home / name
                        if lock.exists():
                            subprocess.run([str(Path(os.environ["SystemRoot"]) / "System32/icacls.exe"),
                                            str(lock), "/reset", "/Q"], check=True,
                                           capture_output=True, timeout=10)
        self.temporary.cleanup()

    @staticmethod
    def _write_executable(path: Path, content: str) -> None:
        path.write_text(content, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def run_runner(
        self, artifact: Path, extra_environment: dict[str, str] | None = None,
        *, default_python: bool = False
    ) -> subprocess.CompletedProcess[str]:
        self.artifacts.append(artifact)
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
                **({"HOME": str(self.root / "home")} if os.name == "nt" else {}),
            }
        )
        if extra_environment:
            environment.update(extra_environment)
        command = ["bash", "tools/bench/release_profile.sh"]
        if os.name == "nt":
            command = [str(Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"),
                       "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File",
                       "tools/bench/release_profile.ps1", "-PythonExecutable", sys.executable,
                       "-FixtureTools", "tools/bench/fixture-tools.json"]
            if default_python:
                environment["PATH"] = str(Path(sys.executable).parent) + os.pathsep + environment["PATH"]
                index = command.index("-PythonExecutable")
                del command[index:index + 2]
        return subprocess.run(
            command,
            cwd=self.repo,
            env=environment,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="strict",
            timeout=60,
        )

    def test_clean_external_artifact_run_is_qualified(self) -> None:
        result = self.run_runner(self.root / "artifacts")
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
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

    def test_windows_protection_and_profile_order_are_observable(self) -> None:
        if os.name != "nt":
            # The original POSIX runner still exercises this same ordering contract.
            result = self.run_runner(self.root / "artifacts")
            self.assertEqual(result.returncode, 0, result.stderr)
            return
        log = self.root / "invocations.jsonl"
        artifact = self.root / "artifacts"
        result = self.run_runner(artifact, {"FAKE_INVOCATION_LOG": str(log)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(log.read_text(encoding="utf-8").splitlines(), [
            "default", "candidate", "default", "candidate", "candidate", "default",
            "default", "candidate", "candidate", "default", "default", "candidate"])
        for name in ("evidence.json", "report.json"):
            execution = json.loads((artifact / name).read_text(encoding="utf-8"))["execution_provenance"]
            self.assertTrue(execution["fixture_only"])
            self.assertFalse(execution["production_performance_evidence"])
            self.assertEqual(execution["sha256"], hashlib.sha256(
                (artifact / execution["file"]).read_bytes()).hexdigest())

    def test_persistent_snapshot_protection_change_is_rejected(self) -> None:
        if os.name != "nt":
            result = self.run_runner(self.root / "artifacts", {"FAKE_MUTATE_SNAPSHOT": "1"})
        else:
            result = self.run_runner(self.root / "artifacts", {"FAKE_CHANGE_SNAPSHOT_ACL": "1"})
        self.assertEqual(result.returncode, 2)
        self.assertIn("source snapshot", result.stderr)

    def test_native_tracked_index_and_revision_guards(self) -> None:
        cases = (("FAKE_MUTATE_WORKTREE", "tracked worktree files changed"),
                 ("FAKE_MUTATE_INDEX", "index changed"),
                 ("FAKE_CHANGE_REVISION", "revision changed"))
        for flag, expected in cases:
            with self.subTest(invariant=expected):
                if os.name == "nt":
                    result = self.run_runner(self.root / flag, {flag: "1"})
                    self.assertEqual(result.returncode, 2)
                    self.assertIn(expected, result.stderr)
                    subprocess.run(["git", "-C", str(self.repo), "restore", "--staged", "--worktree", "bench-input.txt"],
                                   check=True, capture_output=True, timeout=10)
                else:
                    # New Windows adapters cannot affect the existing POSIX isolation path.
                    result = self.run_runner(self.root / flag)
                    self.assertEqual(result.returncode, 0, result.stderr)

    def test_native_cache_junctions_preserve_writable_cache_targets(self) -> None:
        home = self.root / "home"
        for name in ("registry", "git"):
            cache = home / ".cargo" / name
            cache.mkdir(parents=True)
            (cache / "cache-data").write_text("initial\n", encoding="utf-8")
        result = self.run_runner(self.root / "artifacts", {
            "HOME": str(home), "FAKE_CACHE_IO": "1", "FAKE_REQUIRE_ISOLATED_CARGO_HOME": "1"})
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        if os.name == "nt":
            for name in ("registry", "git"):
                self.assertEqual((home / ".cargo" / name / "cache-data").read_text(encoding="utf-8"),
                                 "initial\ncargo\ncargo\n")

    def test_default_python_resolution_and_utf8_output(self) -> None:
        artifact = self.root / "科技新闻-é"
        result = self.run_runner(artifact, default_python=True)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn(str(artifact), result.stdout)

    def test_actual_native_toolchain_resolution_does_not_build(self) -> None:
        if os.name != "nt":
            result = self.run_runner(self.root / "artifacts")
            self.assertEqual(result.returncode, 0, result.stderr)
            return
        environment = {name: value for name, value in os.environ.items()
                       if not name.startswith(("CARGO_", "RUST", "SCCACHE_"))}
        environment["PATH"] = str(Path(sys.executable).parent) + os.pathsep + environment["PATH"]
        result = subprocess.run([
            str(Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"),
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File",
            str(SOURCE_DIR / "release_profile.ps1"), "-ToolchainInfo"],
            cwd=self.repo, env=environment, capture_output=True, text=True,
            encoding="utf-8", errors="strict", timeout=35)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        document = json.loads(result.stdout)
        self.assertTrue(document["read_only"])
        self.assertFalse(document["benchmark_executed"])
        self.assertEqual(document["native_exit"], 0)
        self.assertFalse((self.repo / "target").exists())
        for name in ("cargo", "rustc"):
            executable = Path(document["executables"][name][0])
            self.assertEqual(executable.name.lower(), name + ".exe")
            self.assertTrue(executable.is_file())
            self.assertTrue(document["versions"][name].startswith(name + " "))


if __name__ == "__main__":
    unittest.main()
