from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SOURCE = Path(__file__).resolve().parent


class NativePackageParityTests(unittest.TestCase):
    def test_windows_binary_and_probe_set_matches_posix_package(self) -> None:
        posix = (SOURCE / "package.sh").read_text(encoding="utf-8").replace("\\\n", " ")
        native = (SOURCE / "package.ps1").read_text(encoding="utf-8")
        probes = re.findall(r"^build_probe\s+(\S+)\s+(\S+)\s+(\S+)", posix, re.MULTILINE)
        binaries = re.findall(r"^\s*build_binary\s+(\S+)\s+(\S+)\s+(\S+)", posix, re.MULTILINE)
        probe_block = re.search(r"\$probes = @\((.*?)\n    \)", native, re.DOTALL)
        binary_block = re.search(r"\$binaries = @\((.*?)\n    \)", native, re.DOTALL)
        self.assertIsNotNone(probe_block)
        self.assertIsNotNone(binary_block)
        native_probes = re.findall(r"@\('([^']+)', '([^']+)', '([^']+)'\)", probe_block.group(1))
        native_binaries = re.findall(r"@\('([^']+)', '([^']+)'\)", binary_block.group(1))
        self.assertEqual(set(native_probes), set(probes))
        self.assertEqual(set(native_binaries), {(package, name) for package, name, installed in binaries if name == installed})
        self.assertEqual(len(native_probes), len(probes))
        self.assertEqual(len(native_binaries), len(binaries))

    def test_native_package_keeps_identity_and_locked_build_guards(self) -> None:
        native = (SOURCE / "package.ps1").read_text(encoding="utf-8")
        for invariant in (
            "'diff', '--quiet'", "'diff', '--cached', '--quiet'",
            "'ls-files', '--others', '--exclude-standard', '-z'",
            "'--release', '--locked', '--offline', '--target'",
            "MAGIC_MARKET_SOURCE_REVISION", "CARGO_TARGET_DIR",
            "RELEASE_REVISION", "TARGET_TRIPLE", "RUSTC_VERSION", "CARGO_VERSION",
            "SHA256SUMS", "[StringComparer]::Ordinal", "ReparsePoint",
            "[IO.File]::Copy($packageInput, $destination, $false)",
            "release output already exists; preserve previous evidence",
        ):
            with self.subTest(invariant=invariant):
                self.assertIn(invariant, native)
        self.assertNotIn("Remove-Item", native)
        self.assertNotIn("bash", native.lower())


@unittest.skipUnless(os.name == "nt", "native execution requires Windows; POSIX packaging remains separate")
class NativePackageExecutionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.pwsh = shutil.which("pwsh.exe")
        self.git = shutil.which("git.exe")
        if not self.pwsh or not self.git:
            self.fail("existing Windows PowerShell7/native Git are required, not silently skipped")
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "native package fixture"
        (self.root / "tools/release").mkdir(parents=True)
        shutil.copy2(SOURCE / "package.ps1", self.root / "tools/release/package.ps1")
        (self.root / "Cargo.toml").write_text('[workspace]\nresolver = "2"\n', encoding="utf-8")
        (self.root / ".gitignore").write_text("/target\n", encoding="utf-8")
        self.git_command("init", "-q")
        self.git_command("config", "user.name", "Native package fixture")
        self.git_command("config", "user.email", "fixture@example.invalid")
        self.git_command("add", "-A")
        self.git_command("-c", "core.hooksPath=", "commit", "-qm", "fixture")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git_command(self, *arguments: str) -> None:
        subprocess.run([self.git, "-C", str(self.root), *arguments], check=True, capture_output=True, timeout=15)

    def run_package(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [self.pwsh, "-NoProfile", "-NonInteractive", "-File", str(self.root / "tools/release/package.ps1")],
            cwd=self.root, check=False, capture_output=True, text=True,
            encoding="utf-8", errors="strict", timeout=30,
        )

    def test_dirty_tracked_source_fails_before_build_creation(self) -> None:
        (self.root / "Cargo.toml").write_text('[workspace]\nresolver = "2"\n# dirty\n', encoding="utf-8")
        result = self.run_package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exit=1", result.stderr)
        self.assertFalse((self.root / "target").exists())

    def test_untracked_build_input_fails_before_build_creation(self) -> None:
        (self.root / "tools/release/untracked.ps1").write_text("# untracked build input\n", encoding="utf-8")
        result = self.run_package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("untracked build inputs", result.stderr)
        self.assertFalse((self.root / "target").exists())

    def test_staged_source_change_fails_before_build_creation(self) -> None:
        (self.root / "Cargo.toml").write_text('[workspace]\nresolver = "2"\n# staged\n', encoding="utf-8")
        self.git_command("add", "Cargo.toml")
        result = self.run_package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exit=1", result.stderr)
        self.assertFalse((self.root / "target").exists())


@unittest.skipUnless(os.name == "nt", "native execution requires Windows; POSIX packaging remains separate")
class NativePackageFunctionExecutionTests(unittest.TestCase):
    """Execute production functions without evaluating the release entry point."""

    def setUp(self) -> None:
        self.pwsh = shutil.which("pwsh.exe")
        if not self.pwsh:
            self.fail("existing Windows PowerShell7 is required, not silently skipped")
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "native package function fixture"
        self.root.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_functions(
        self, names: tuple[str, ...], body: str, child_code: str = ""
    ) -> subprocess.CompletedProcess[bytes]:
        # Parse the current production source and load only the named functions.
        # Dot-sourcing package.ps1 would run its release entry point and is forbidden here.
        function_names = ", ".join(f"'{name}'" for name in names)
        wrapper = self.root / "function-fixture.ps1"
        wrapper.write_text(
            """param([string]$ToolSource, [string]$Python, [string]$ChildCode)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$repositoryRoot = $PSScriptRoot
$script:nativeFailureExit = 1
$tokens = $null
$parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($ToolSource, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw 'production package source did not parse' }
"""
            + f"$functionNames = @({function_names})\n"
            + """foreach ($name in $functionNames) {
    $definitions = @($ast.FindAll({ param($node)
        $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq $name
    }, $true))
    if ($definitions.Count -ne 1) { throw "expected one production function: $name" }
    . ([ScriptBlock]::Create($definitions[0].Extent.Text))
}
try {
"""
            + body
            + """
    exit 0
}
catch {
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'exception.txt'), $_.Exception.Message, [Text.UTF8Encoding]::new($false))
    exit $script:nativeFailureExit
}
""",
            encoding="utf-8",
        )
        return subprocess.run(
            [self.pwsh, "-NoProfile", "-NonInteractive", "-File", str(wrapper),
             "-ToolSource", str(SOURCE / "package.ps1"), "-Python", sys.executable,
             "-ChildCode", child_code],
            cwd=self.root, check=False, capture_output=True, timeout=30,
        )

    def test_manifest_includes_hidden_system_and_hidden_directory_files(self) -> None:
        distribution = self.root / "distribution"
        contents = {
            "visible.bin": b"visible\x00\xff",
            "hidden.bin": b"hidden",
            "system.bin": b"system\xfe",
            "hidden-system.bin": b"both attributes",
            "hidden folder/inside.bin": b"hidden directory child",
            "docs/中文 space.md": "UTF-8 文件\n".encode("utf-8"),
        }
        for relative, data in contents.items():
            path = distribution / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        result = self.run_functions(
            ("Assert-LocalPath", "Write-PackageManifest"),
            """$distribution = Join-Path $PSScriptRoot 'distribution'
$attributes = @{
    'hidden.bin' = [IO.FileAttributes]::Hidden
    'system.bin' = [IO.FileAttributes]::System
    'hidden-system.bin' = [IO.FileAttributes]::Hidden -bor [IO.FileAttributes]::System
    'hidden folder' = [IO.FileAttributes]::Hidden
}
$observed = @{}
foreach ($relative in $attributes.Keys) {
    $path = Join-Path $distribution $relative
    [IO.File]::SetAttributes($path, [IO.File]::GetAttributes($path) -bor $attributes[$relative])
    $observed[$relative] = [int][IO.File]::GetAttributes($path)
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'attributes.json'), ($observed | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
Write-PackageManifest $distribution
""",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        observed = json.loads((self.root / "attributes.json").read_text(encoding="utf-8"))
        for relative, flags in {"hidden.bin": 2, "system.bin": 4, "hidden-system.bin": 6, "hidden folder": 2}.items():
            with self.subTest(attribute_precondition=relative):
                self.assertEqual(observed[relative] & flags, flags)
        expected = "".join(
            f"{hashlib.sha256(contents[relative]).hexdigest()}  {relative}\n"
            for relative in sorted(contents)
        ).encode("utf-8")
        self.assertEqual((distribution / "SHA256SUMS").read_bytes(), expected)
        for relative, data in contents.items():
            with self.subTest(retained_input=relative):
                self.assertEqual((distribution / relative).read_bytes(), data)

    def test_manifest_refuses_existing_output_and_retains_evidence(self) -> None:
        distribution = self.root / "distribution"
        distribution.mkdir()
        payload = b"retained payload\x00\xff"
        prior_manifest = b"prior manifest bytes\n"
        (distribution / "payload.bin").write_bytes(payload)
        (distribution / "SHA256SUMS").write_bytes(prior_manifest)
        result = self.run_functions(
            ("Assert-LocalPath", "Write-PackageManifest"),
            "Write-PackageManifest (Join-Path $PSScriptRoot 'distribution')\n",
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual((distribution / "SHA256SUMS").read_bytes(), prior_manifest)
        self.assertEqual((distribution / "payload.bin").read_bytes(), payload)

    def test_capture_native_failure_retains_non_utf8_bytes_and_exit(self) -> None:
        stdout = b"\xffstdout\x00\n"
        stderr = b"\x80stderr\xfe\n"
        result = self.run_functions(
            ("Invoke-Native",),
            "$captured = Invoke-Native $Python @('-c', $ChildCode) -Capture\n",
            f"import sys; sys.stdout.buffer.write({stdout!r}); sys.stderr.buffer.write({stderr!r}); sys.exit(17)",
        )
        self.assertEqual(result.returncode, 17)
        self.assertEqual(result.stdout, stdout)
        self.assertEqual(result.stderr, stderr)
        self.assertIn("exit=17", (self.root / "exception.txt").read_text(encoding="utf-8"))

    def test_capture_native_success_returns_utf8_and_forwards_raw_stderr(self) -> None:
        stdout = "成功捕获 中文\n\x00".encode("utf-8")
        stderr = b"native diagnostic \x80\n"
        result = self.run_functions(
            ("Invoke-Native",),
            "$captured = Invoke-Native $Python @('-c', $ChildCode) -Capture\n[Console]::Out.Write($captured)\n",
            f"import sys; sys.stdout.buffer.write({stdout!r}); sys.stderr.buffer.write({stderr!r})",
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, stdout)
        self.assertEqual(result.stderr, stderr)
        self.assertFalse((self.root / "exception.txt").exists())

    def test_capture_native_zero_invalid_utf8_fails_without_losing_bytes(self) -> None:
        stdout = b"invalid\xff UTF-8\n"
        stderr = b"diagnostic\xfe\n"
        result = self.run_functions(
            ("Invoke-Native",),
            "$captured = Invoke-Native $Python @('-c', $ChildCode) -Capture\n",
            f"import sys; sys.stdout.buffer.write({stdout!r}); sys.stderr.buffer.write({stderr!r})",
        )
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, stdout)
        self.assertEqual(result.stderr, stderr)
        self.assertIn("not valid UTF-8", (self.root / "exception.txt").read_text(encoding="utf-8"))

    def test_capture_start_failure_is_not_masked_by_stream_cleanup(self) -> None:
        result = self.run_functions(
            ("Invoke-Native",),
            "Invoke-Native (Join-Path $PSScriptRoot 'missing-native.exe') @() -Capture\n",
        )
        self.assertEqual(result.returncode, 1)
        exception = (self.root / "exception.txt").read_text(encoding="utf-8")
        self.assertIn("missing-native.exe", exception)
        self.assertNotIn("stdoutRaw", exception)
        self.assertNotIn("stderrRaw", exception)


if __name__ == "__main__":
    unittest.main()
