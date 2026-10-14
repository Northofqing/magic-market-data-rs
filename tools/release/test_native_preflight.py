"""Windows-native gate parity and fail-closed fixture regressions.

Fixtures do not claim real admission or Rust/coverage execution. The real
preflight runs those unchanged checkers and workloads separately.
"""

from __future__ import annotations

import os
import json
import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COMPLIANCE = ROOT / "tools/compliance/check.ps1"
DOCS = ROOT / "tools/docs/check_links.ps1"
PREFLIGHT = ROOT / "tools/release/preflight.ps1"
PWSH = shutil.which("pwsh.exe") if os.name == "nt" else shutil.which("pwsh")
RG = shutil.which("rg.exe") if os.name == "nt" else shutil.which("rg")


def shell_table(name: str) -> list[str]:
    text = (ROOT / "tools/compliance/check.sh").read_text(encoding="utf-8")
    block = re.search(rf"(?m)^{re.escape(name)}=\(\n(.*?)^\)", text, re.S)
    assert block is not None
    return block.group(1).split()


def powershell_table(name: str) -> list[str]:
    text = COMPLIANCE.read_text(encoding="utf-8")
    block = re.search(rf"(?m)^\${re.escape(name)} = @\(\n(.*?)^\)", text, re.S)
    assert block is not None
    return re.findall(r"'([^']+)'", block.group(1))


class NativeGateParityTests(unittest.TestCase):
    def test_required_files_and_members_are_identical(self) -> None:
        self.assertEqual(shell_table("required"), powershell_table("required"))
        self.assertEqual(
            shell_table("workspace_members"), powershell_table("workspaceMembers")
        )

    def test_all_original_python_checkers_are_retained(self) -> None:
        original = (ROOT / "tools/compliance/check.sh").read_text(encoding="utf-8")
        expected = re.findall(r"\"\$\{python_cmd\[@\]\}\" ([^\n]+\.py)", original)
        self.assertEqual(expected, powershell_table("pythonChecks"))

    def test_preflight_retains_all_rust_workloads_and_strict_coverage(self) -> None:
        text = PREFLIGHT.read_text(encoding="utf-8")
        for workload in (
            "cargo-check", "cargo-test", "cargo-clippy", "cargo-doc",
            "cargo-doctest", "format", "coverage-thresholds",
            "documentation-links", "compliance", "git-diff-check",
        ):
            self.assertIn(f"'{workload}'", text)
        for flag in (
            "'--workspace'", "'--all-targets'", "'--all-features'",
            "'--locked'", "'--offline'", "'--test-threads=1'",
            "'warnings'", "RUSTDOCFLAGS = '-D warnings'",
        ):
            self.assertIn(flag, text)
        self.assertIn("tools/coverage/check_thresholds.py", text)
        self.assertIn("-Filter 'test_*.py' -File -Recurse", text)
        self.assertIn("ProcessStartInfo", text)
        self.assertIn(".ArgumentList.Add", text)
        self.assertIn("BaseStream.CopyToAsync", text)
        self.assertIn("'failed'", text)
        self.assertNotIn("Remove-Item", text)
        self.assertNotIn("taskkill", text.lower())
        self.assertNotIn("MAGIC_REQUIRE_COVERAGE_EVIDENCE", text)


@unittest.skipUnless(os.name == "nt", "Windows-native execution; POSIX retains its shell gate")
class NativeGateExecutionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not PWSH or not RG:
            raise RuntimeError("native Windows regressions require existing pwsh.exe and rg.exe")

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="magic-native-gate-")
        self.fixture = Path(self.temporary.name)
        for relative in shell_table("required"):
            self.write(relative, "fixture\n")
        for member in shell_table("workspace_members"):
            self.write(member + "/Cargo.toml", '[package]\nname = "fixture"\nversion = "0.2.0"\n')
        members = "\n".join(f'  "{member}",' for member in shell_table("workspace_members"))
        self.write("Cargo.toml", f"[workspace]\nmembers = [\n{members}\n]\n")
        self.write("docs/business_rules.md", "\n".join(f"## BR-{number:03d} Fixture" for number in range(1, 45)))
        self.write("docs/ENGINEERING_RULES.md", "## Gate D Release\n")
        self.write(".github/workflows/ci.yml", "fixture\n")
        for checker in powershell_table("pythonChecks"):
            self.write(checker, "print('fixture checker')\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write(self, relative: str, text: str) -> None:
        path = self.fixture / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def run_script(
        self, script: Path, *extra: str, use_explicit_rg: bool = True,
        environment_overrides: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        assert PWSH is not None and RG is not None
        arguments = [PWSH, "-NoLogo", "-NoProfile", "-File", str(script)]
        if script != PREFLIGHT:
            arguments.extend(["-RepositoryRoot", str(self.fixture)])
            if use_explicit_rg:
                arguments.extend(["-RgExecutable", RG])
        if script == COMPLIANCE:
            arguments.extend(["-PythonExecutable", sys.executable])
        arguments.extend(extra)
        environment = os.environ.copy()
        environment["PYTHONUTF8"] = "1"
        environment["PYTHONIOENCODING"] = "utf-8"
        environment.update(environment_overrides or {})
        return subprocess.run(
            arguments, cwd=ROOT, env=environment, capture_output=True,
            encoding="utf-8", errors="strict", timeout=20, check=False,
        )

    def assert_rejected(self, expected: str) -> None:
        result = self.run_script(COMPLIANCE)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(expected, result.stderr)

    def test_scripts_parse_with_native_parser(self) -> None:
        assert PWSH is not None
        code = (
            "$ErrorActionPreference='Stop'; foreach($path in (ConvertFrom-Json $env:MAGIC_NATIVE_PARSE_PATHS)){"
            "$t=$null;$e=$null;[void][System.Management.Automation.Language.Parser]::ParseFile($path,[ref]$t,[ref]$e);"
            "if($e.Count){throw $e[0].Message}}"
        )
        environment = os.environ.copy()
        environment["MAGIC_NATIVE_PARSE_PATHS"] = json.dumps([str(COMPLIANCE), str(DOCS), str(PREFLIGHT)])
        result = subprocess.run(
            [PWSH, "-NoLogo", "-NoProfile", "-Command", code], env=environment,
            capture_output=True, encoding="utf-8", timeout=20, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_compliance_valid_fixture(self) -> None:
        result = self.run_script(COMPLIANCE)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("compliance: passed", result.stdout)

    def test_empty_required_file_rejected(self) -> None:
        self.write("Cargo.lock", "")
        self.assert_rejected("missing required file: Cargo.lock")

    def test_python_native_failure_is_propagated(self) -> None:
        self.write(powershell_table("pythonChecks")[0], "raise SystemExit(17)\n")
        result = self.run_script(COMPLIANCE)
        self.assertEqual(result.returncode, 17, result.stdout + result.stderr)

    def test_fixed_toolchain_selector_rejected(self) -> None:
        self.write("rust-toolchain.toml", '[toolchain]\nchannel = "stable"\n')
        self.assert_rejected("fixed repository toolchain selector")

    def test_rust_version_rejected(self) -> None:
        self.write("crates/magic-market-core/Cargo.toml", '[package]\nversion = "0.2.0"\nrust-version = "1.0"\n')
        self.assert_rejected("must not declare a fixed Rust version")

    def test_numeric_workflow_toolchain_rejected(self) -> None:
        self.write(".github/workflows/ci.yml", "uses: dtolnay/rust-toolchain@1.2.3\n")
        self.assert_rejected("must use the stable Rust toolchain")

    def test_numeric_active_toolchain_rejected(self) -> None:
        self.write("tools/fixture.ps1", "RUSTUP_TOOLCHAIN=" + "1.2.3\n")
        self.assert_rejected("must not select a numeric Rust version")

    def test_missing_workspace_member_rejected(self) -> None:
        path = self.fixture / "Cargo.toml"
        text = path.read_text(encoding="utf-8").replace('"crates/magic-market-core"', '"crates/other"')
        self.write("Cargo.toml", text)
        self.assert_rejected("missing workspace member: crates/magic-market-core")

    def test_workspace_version_mismatch_rejected(self) -> None:
        self.write("crates/magic-market-core/Cargo.toml", '[package]\nversion = "0.3.0"\n')
        self.assert_rejected("workspace crate version mismatch")

    def test_workspace_version_missing_rejected(self) -> None:
        self.write("crates/magic-market-core/Cargo.toml", '[package]\nname = "fixture"\n')
        self.assert_rejected("workspace crate version missing")

    def test_downstream_dependency_rejected(self) -> None:
        self.write("crates/magic-market-core/Cargo.toml", '[package]\nversion = "0.2.0"\n# stock_analysis\n')
        self.assert_rejected("downstream stock_analysis dependency")

    def test_router_provider_dependency_rejected(self) -> None:
        self.write("crates/magic-market-router/Cargo.toml", '[package]\nversion = "0.2.0"\n[dependencies]\nmagic-sina-rs = "0.2.0"\n')
        self.assert_rejected("provider-neutral")

    def test_router_dev_provider_is_not_a_production_dependency(self) -> None:
        self.write("crates/magic-market-router/Cargo.toml", '[package]\nversion = "0.2.0"\n[dependencies]\nserde = "1"\n[dev-dependencies]\nmagic-sina-rs = "0.2.0"\n')
        result = self.run_script(COMPLIANCE)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_router_next_nested_provider_header_is_rejected(self) -> None:
        self.write("crates/magic-market-router/Cargo.toml", '[package]\nversion = "0.2.0"\n[dependencies]\n[dependencies.magic-sina-rs]\nversion = "0.2.0"\n')
        self.assert_rejected("provider-neutral")

    def test_missing_rule_rejected(self) -> None:
        text = (self.fixture / "docs/business_rules.md").read_text(encoding="utf-8")
        self.write("docs/business_rules.md", text.replace("## BR-044 ", "## OTHER "))
        self.assert_rejected("missing registered business rule: BR-044")

    def test_missing_gate_d_rejected(self) -> None:
        self.write("docs/ENGINEERING_RULES.md", "## Gate C Verification\n")
        self.assert_rejected("missing Gate D release rule")

    def test_link_relative_root_fragment_remote_and_unicode(self) -> None:
        self.write("docs/nested/文档.md", "[relative](../present.txt#anchor) [root](LICENSE-MIT)\n[fragment](#title) [web](https://example.invalid/) [mail](mailto:user@example.invalid)\n")
        self.write("docs/present.txt", "present\n")
        result = self.run_script(DOCS)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_link_missing_target_rejected(self) -> None:
        self.write("docs/links.md", "[missing](not-present.txt)\n")
        result = self.run_script(DOCS)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("docs/links.md: missing link target not-present.txt", result.stderr)

    def test_default_ripgrep_resolver_selects_one_from_multiple_path_candidates(self) -> None:
        assert PWSH is not None and RG is not None
        # Copies are generated fixture tools, not installs or global PATH edits.
        directories = [self.fixture / "rg-one", self.fixture / "rg-two"]
        for directory in directories:
            directory.mkdir()
            shutil.copyfile(RG, directory / "rg.exe")
        path = os.pathsep.join([*(str(directory) for directory in directories), os.environ.get("PATH", "")])
        environment = os.environ.copy()
        environment["PATH"] = path
        inventory = subprocess.run(
            [PWSH, "-NoLogo", "-NoProfile", "-Command",
             "@(Get-Command rg.exe -CommandType Application).Count"],
            env=environment, capture_output=True, encoding="utf-8",
            timeout=20, check=False,
        )
        self.assertEqual(inventory.returncode, 0, inventory.stdout + inventory.stderr)
        self.assertGreaterEqual(int(inventory.stdout.strip()), 2)
        self.write("docs/default-resolver.md", "[valid](../LICENSE-MIT)\n")
        result = self.run_script(
            DOCS, use_explicit_rg=False, environment_overrides={"PATH": path},
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("documentation links: passed", result.stdout)

    def test_link_scan_preserves_ripgrep_glob_override_and_directory_ignores(self) -> None:
        # The canonical -g '*.md' whitelist overrides a file-level ignore;
        # an ignored directory is pruned before its children are visited.
        self.write(".ignore", "docs/ignored.md\ndocs/excluded/\n")
        self.write("docs/ignored.md", "[ignored](missing.txt)\n")
        self.write("docs/excluded/ignored.md", "[excluded](missing.txt)\n")
        result = self.run_script(DOCS)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("docs/ignored.md: missing link target", result.stderr)
        self.assertNotIn("docs/excluded", result.stderr)

    def test_ripgrep_errors_are_not_success(self) -> None:
        shutil.rmtree(self.fixture / "crates")
        result = self.run_script(DOCS)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("ripgrep failed", result.stderr)

    def test_preflight_rejects_missing_coverage_before_build(self) -> None:
        result = self.run_script(PREFLIGHT, "-CoverageJson", str(self.fixture / "missing.json"))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("requires existing MAGIC_COVERAGE_JSON", result.stderr)
        self.assertNotIn("preflight: cargo", result.stdout)

    def test_preflight_native_capture_preserves_raw_utf8_and_failure_code(self) -> None:
        assert PWSH is not None
        # Execute only the production capture functions, not the Cargo gate.
        wrapper = r'''param([string] $ToolSource, [string] $Python)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$runDirectory = Join-Path $root 'capture'
[void] [IO.Directory]::CreateDirectory($runDirectory)
$records = [Collections.Generic.List[object]]::new()
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($ToolSource, [ref] $tokens, [ref] $errors)
if ($errors.Count) { throw $errors[0].Message }
$functions = $ast.FindAll({ param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
    $node.Name -in @('Write-NewJson', 'Invoke-Check')
}, $false)
if ($functions.Count -ne 2) { throw 'capture function set changed' }
foreach ($definition in $functions) { . ([ScriptBlock]::Create($definition.Extent.Text)) }
try {
    Invoke-Check 'leaf-fixture' $Python @('-c', "import sys; sys.stdout.buffer.write('中文'.encode('utf-8')); sys.stderr.buffer.write(b'leaf-error'); sys.exit(17)")
    exit 99
} catch {
    if ($_.Exception.Data.Contains('NativeExitCode')) { exit ([int] $_.Exception.Data['NativeExitCode']) }
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 98
}
'''
        self.write("capture.ps1", wrapper)
        result = subprocess.run(
            [PWSH, "-NoLogo", "-NoProfile", "-File", str(self.fixture / "capture.ps1"),
             "-ToolSource", str(PREFLIGHT), "-Python", sys.executable],
            cwd=ROOT, capture_output=True, encoding="utf-8", errors="strict",
            timeout=20, check=False,
        )
        self.assertEqual(result.returncode, 17, result.stdout + result.stderr)
        capture = self.fixture / "capture"
        output = (capture / "01-leaf-fixture.stdout.bin").read_bytes()
        error = (capture / "01-leaf-fixture.stderr.bin").read_bytes()
        self.assertEqual(output, "中文".encode("utf-8"))
        self.assertEqual(error, b"leaf-error")
        receipt = json.loads((capture / "01-leaf-fixture.json").read_text(encoding="utf-8"))
        self.assertEqual(receipt["native_exit_code"], 17)
        self.assertEqual(receipt["stdout_sha256"], hashlib.sha256(output).hexdigest())
        self.assertEqual(receipt["stderr_sha256"], hashlib.sha256(error).hexdigest())


if __name__ == "__main__":
    unittest.main()
