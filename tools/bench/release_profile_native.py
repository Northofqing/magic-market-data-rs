"""Windows benchmark orchestration; production executes native EXEs, never Bash.

NTFS DACLs reject ordinary writes/new children. Retained non-inheritable read
handles additionally prevent source-file writes/deletes even after ACL changes.
Current owner/group/DACL, attributes, paths and bytes are digest-bound. This is
not a hostile-owner sandbox: owners can change ACLs, which checks detect.
"""
from __future__ import annotations

import argparse
import base64
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile


class GuardError(Exception):
    """A provenance/isolation invariant failed; keep artifacts and return 2."""


class CommandError(GuardError):
    def __init__(self, message, native_exit):
        super().__init__(message)
        self.native_exit = native_exit


class WindowsFiles:
    def __init__(self):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.security = ctypes.WinDLL("advapi32", use_last_error=True)
        self.kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD,
            wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
            wintypes.HANDLE]
        self.kernel.CreateFileW.restype = wintypes.HANDLE
        self.kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        self.kernel.CloseHandle.restype = wintypes.BOOL
        self.security.GetFileSecurityW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD,
            ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
        self.security.GetFileSecurityW.restype = wintypes.BOOL
        self.kernel.GetVolumeInformationW.argtypes = [wintypes.LPCWSTR,
            wintypes.LPWSTR, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD),
            ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD),
            wintypes.LPWSTR, wintypes.DWORD]
        self.kernel.GetVolumeInformationW.restype = wintypes.BOOL
        self.handles = []

    def require_ntfs(self, path):
        name = ctypes.create_unicode_buffer(256)
        if not self.kernel.GetVolumeInformationW(path.anchor, None, 0, None,
                                                None, None, name, len(name)):
            raise ctypes.WinError(ctypes.get_last_error())
        if name.value != "NTFS":
            raise GuardError("benchmark immutable snapshot requires NTFS")

    def descriptor(self, path):
        needed = wintypes.DWORD()
        self.security.GetFileSecurityW(str(path), 7, None, 0, ctypes.byref(needed))
        if ctypes.get_last_error() != 122 or not needed.value:
            raise ctypes.WinError(ctypes.get_last_error())
        buffer = ctypes.create_string_buffer(needed.value)
        if not self.security.GetFileSecurityW(str(path), 7, buffer, len(buffer),
                                             ctypes.byref(needed)):
            raise ctypes.WinError(ctypes.get_last_error())
        return buffer.raw[:needed.value]

    def retain_read_only(self, path):
        # GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING; NULL security = no inheritance.
        handle = self.kernel.CreateFileW(str(path), 0x80000000, 1, None, 3, 0, None)
        if handle == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        self.handles.append(handle)

    def close(self):
        for handle in reversed(self.handles):
            if not self.kernel.CloseHandle(handle):
                raise ctypes.WinError(ctypes.get_last_error())
        self.handles.clear()


def native_executable(name):
    found = shutil.which(name)
    if not found or Path(found).suffix.lower() != ".exe":
        raise GuardError(f"benchmark requires existing native Windows executable: {name}")
    # Keep argv[0] spelling: cargo.exe/rustc.exe may be rustup proxy symlinks.
    path = Path(found).absolute()
    with path.open("rb") as executable:
        if executable.read(2) != b"MZ":
            raise GuardError(f"benchmark tool is not a native Windows executable: {path}")
    return str(path)


def rust_tools(root_directory):
    if shutil.which("rustup.exe"):
        rustup = native_executable("rustup.exe")
        resolved = {}
        for tool in ("cargo", "rustc"):
            path = run([rustup, "which", tool], cwd=root_directory, timeout=10).decode("utf-8").strip()
            if not Path(path).is_absolute() or Path(path).name.lower() != f"{tool}.exe":
                raise GuardError(f"rustup which did not identify an existing native {tool}.exe")
            resolved[tool] = [native_executable(path)]
        return resolved
    return {tool: [native_executable(f"{tool}.exe")] for tool in ("cargo", "rustc")}


def run(arguments, *, cwd, environment=None, timeout=60, output=None, log=None):
    try:
        result = subprocess.run(arguments, cwd=cwd, env=environment, check=False,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=timeout)
    except subprocess.TimeoutExpired as error:
        if log is not None:
            log.append({"argv": arguments, "native_exit": None, "timeout_seconds": timeout,
                        "stdout_b64": base64.b64encode(error.stdout or b"").decode("ascii"),
                        "stderr_b64": base64.b64encode(error.stderr or b"").decode("ascii")})
        raise GuardError(f"benchmark owned command exceeded {timeout} seconds") from error
    if log is not None:
        log.append({"argv": arguments, "native_exit": result.returncode,
                    "stdout_b64": base64.b64encode(result.stdout).decode("ascii"),
                    "stderr_b64": base64.b64encode(result.stderr).decode("ascii")})
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="backslashreplace").strip()
        raise CommandError(f"benchmark command failed (native {result.returncode}): {detail}",
                           result.returncode)
    if output is not None:
        output.write_bytes(result.stdout)
    return result.stdout


def reject_configs(start):
    for parent in (start, *start.parents):
        for name in ("config", "config.toml"):
            candidate = parent / ".cargo" / name
            if os.path.lexists(candidate):
                raise GuardError(f"benchmark rejects automatic Cargo config: {candidate}")


def clean_source(git, repo, revision):
    if run([git, "-C", str(repo), "rev-parse", "HEAD"], cwd=repo).decode().strip() != revision:
        raise GuardError("benchmark revision changed during execution")
    status = run([git, "-C", str(repo), "status", "--porcelain=v1",
                  "--untracked-files=all"], cwd=repo).decode("utf-8")
    for record in status.splitlines():
        if record[:2] == "??":
            continue
        if len(record) < 3:
            raise GuardError("benchmark cannot validate Git porcelain status")
        if record[1] != " ":
            raise GuardError("tracked worktree files changed during benchmark execution")
        if record[0] != " ":
            raise GuardError("index changed during benchmark execution")
    if status:
        raise GuardError(f"benchmark requires a clean worktree and index\n{status}")


def tree_entries(root):
    if root.lstat().st_file_attributes & 0x400:
        raise GuardError(f"benchmark source snapshot rejects reparse root: {root}")
    result = [root]
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in sorted(directories + files):
            path = Path(parent) / name
            if path.lstat().st_file_attributes & 0x400:
                raise GuardError(f"benchmark source snapshot rejects reparse entries: {path}")
            result.append(path)
    return sorted(result, key=lambda path: path.relative_to(root).as_posix())


def digest_snapshot(root, native):
    digest = hashlib.sha256()
    for path in tree_entries(root):
        metadata = path.lstat()
        fields = ("directory" if path.is_dir() else "file",
                  path.relative_to(root).as_posix(), str(metadata.st_file_attributes))
        for value in (*fields, native.descriptor(path)):
            data = value if isinstance(value, bytes) else value.encode("utf-8")
            digest.update(len(data).to_bytes(8, "big")); digest.update(data)
        if path.is_file():
            with path.open("rb") as content:
                for chunk in iter(lambda: content.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif not path.is_dir():
            raise GuardError(f"benchmark source snapshot has unsupported entry: {path}")
    return digest.hexdigest()


def verify_effective_protection(source, home):
    """Probe actual native denies, not NTFS directory read-only attributes."""
    for root, label in ((source, "source snapshot"), (home, "isolated Cargo home")):
        probe = root / ".benchmark-permission-probe"
        if os.path.lexists(probe):
            raise GuardError(f"benchmark {label} permission-probe pathname already exists")
        try:
            with probe.open("xb"):
                pass
        except PermissionError:
            pass
        else:
            raise GuardError(f"benchmark {label} allows new files; failed probe retained")
        try:
            probe.mkdir()
        except PermissionError:
            pass
        else:
            raise GuardError(f"benchmark {label} allows new directories; failed probe retained")
    try:
        with (source / "Cargo.toml").open("r+b"):
            pass
    except PermissionError:
        pass
    else:
        raise GuardError("benchmark source snapshot allows source-file writes")
    for name in (".package-cache", ".package-cache-mutate"):
        with (home / name).open("r+b"):
            pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-tools", type=Path,
                        help="explicit committed test adapter; never production performance evidence")
    parser.add_argument("--toolchain-info", action="store_true",
                        help="read-only native resolution/version check; no benchmark/build/artifacts")
    arguments = parser.parse_args()
    if os.name != "nt":
        raise GuardError("use release_profile.sh on non-Windows hosts")
    repo = Path(__file__).resolve().parents[2]
    for name in sorted(os.environ):
        if name.startswith(("CARGO_", "RUST", "SCCACHE_")):
            raise GuardError(f"benchmark rejects inherited build environment variable: {name}")
    if arguments.toolchain_info:
        if arguments.fixture_tools:
            raise GuardError("toolchain-info cannot use fixture tools")
        root_directory = Path(repo.anchor)
        tools = rust_tools(root_directory)
        versions = {name: run([*command, "--version"], cwd=root_directory, timeout=10).decode("utf-8").strip()
                    for name, command in tools.items()}
        print(json.dumps({"read_only": True, "benchmark_executed": False,
                          "native_exit": 0, "executables": tools, "versions": versions}, indent=2), flush=True)
        return 0
    git = native_executable("git.exe")
    revision = run([git, "-C", str(repo), "rev-parse", "HEAD"], cwd=repo).decode().strip()
    if not re.fullmatch("[0-9a-f]{40}", revision):
        raise GuardError("benchmark revision must be an exact lowercase commit SHA")
    clean_source(git, repo, revision)
    fixture = None
    if arguments.fixture_tools:
        fixture_path = arguments.fixture_tools.resolve(strict=True)
        relative = fixture_path.relative_to(repo).as_posix()
        run([git, "-C", str(repo), "ls-files", "--error-unmatch", "--", relative], cwd=repo)
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        if set(fixture) != {"schema", "cargo", "rustc", "benchmark_suffix"} or type(fixture["schema"]) is not int or fixture["schema"] != 1 or fixture["benchmark_suffix"] != ".py":
            raise GuardError("invalid explicit benchmark fixture adapter")
        for tool in ("cargo", "rustc"):
            path = Path(fixture[tool])
            if path.is_absolute() or ".." in path.parts or path.suffix != ".py":
                raise GuardError("fixture tools must be committed relative Python paths")
            run([git, "-C", str(repo), "ls-files", "--error-unmatch", "--", path.as_posix()], cwd=repo)
    else:
        tools = rust_tools(Path(repo.anchor))
        cargo = tools["cargo"]
        rustc = tools["rustc"]
    requested = os.environ.get("MAGIC_RELEASE_BENCH_DIR")
    if requested:
        artifact = Path(requested).absolute()
        if os.path.lexists(artifact):
            raise GuardError("benchmark artifact directory already exists")
        artifact.mkdir(parents=True, exist_ok=False)
    else:
        (repo / "target").mkdir(exist_ok=True)
        artifact = Path(tempfile.mkdtemp(prefix="release-profile.", dir=repo / "target"))
    artifact = artifact.resolve(strict=True)
    if artifact.is_relative_to(repo):
        if not artifact.is_relative_to(repo / "target") or artifact == repo / "target":
            raise GuardError("in-repository benchmark artifacts must use a Git-ignored path")
        try:
            run([git, "-C", str(repo), "check-ignore", "-q", str(artifact)], cwd=repo)
        except CommandError as failure:
            raise GuardError("in-repository benchmark artifacts must use a Git-ignored path") from failure
    clean_source(git, repo, revision)
    source = artifact / "source"
    home = artifact / "cargo-home"
    source.mkdir(); home.mkdir()
    commands = []
    native = WindowsFiles()
    try:
        native.require_ntfs(source)
        archive = artifact / "source.tar"
        run([git, "-C", str(repo), "archive", "--format=tar", f"--output={archive}", revision], cwd=repo, log=commands)
        with tarfile.open(archive) as contents:
            for member in contents.getmembers():
                if not (member.isdir() or member.isfile()) or Path(member.name).is_absolute() or ".." in Path(member.name).parts:
                    raise GuardError("benchmark committed snapshot contains unsupported archive entry")
            contents.extractall(source, filter="data")
        reject_configs(source)
        root_directory = Path(source.anchor)
        reject_configs(root_directory)
        system32 = Path(os.environ["SystemRoot"]) / "System32"
        sid_output = run([str(system32 / "whoami.exe"), "/user", "/fo", "csv", "/nh"], cwd=root_directory)
        matches = re.findall(rb"S-1-\d+(?:-\d+)+", sid_output)
        if not matches:
            raise GuardError("benchmark cannot establish current Windows user SID")
        sid = matches[-1].decode("ascii")
        icacls = str(system32 / "icacls.exe")
        junctions = {}
        user_home = os.environ.get("HOME") or os.environ.get("USERPROFILE")
        if not user_home:
            raise GuardError("benchmark requires HOME or USERPROFILE for Cargo cache isolation")
        for name in ("registry", "git"):
            cache = Path(user_home) / ".cargo" / name
            if cache.exists():
                target = cache.resolve(strict=True)
                if not target.is_dir():
                    raise GuardError("benchmark Cargo cache target is not a directory")
                link = home / name
                run([str(system32 / "WindowsPowerShell/v1.0/powershell.exe"), "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", str(source / "tools/bench/native_paths.ps1"), "-Path", str(link), "-Target", str(target)], cwd=root_directory, log=commands)
                junctions[name] = str(target)
        for name in (".package-cache", ".package-cache-mutate"):
            lock = home / name
            lock.touch()
            run([icacls, str(lock), "/grant:r", f"*{sid}:(R,W)", "/deny", f"*{sid}:(DE)", "/Q"], cwd=root_directory, log=commands)
        if home.lstat().st_file_attributes & 0x400:
            raise GuardError("benchmark isolated Cargo home root is a reparse point")
        run([icacls, str(home), "/deny", f"*{sid}:(WD,AD,WEA,WA,DE,DC)", "/Q"], cwd=root_directory, log=commands)
        home_descriptor = native.descriptor(home)
        tree_entries(source)  # Reject reparse entries BEFORE recursive ACL changes.
        run([icacls, str(source), "/deny", f"*{sid}:(OI)(CI)(WD,AD,WEA,WA,DE,DC)", "/T", "/Q"], cwd=root_directory, log=commands)
        for path in tree_entries(source):
            if path.is_file():
                native.retain_read_only(path)
        verify_effective_protection(source, home)
        expected = digest_snapshot(source, native)
        (artifact / "source-digest.sha256").write_text(expected + "\n", encoding="ascii")
        if fixture:
            cargo = [sys.executable, "-I", "-B", "-X", "utf8", str(source / fixture["cargo"])]
            rustc = [sys.executable, "-I", "-B", "-X", "utf8", str(source / fixture["rustc"])]

        def verify():
            clean_source(git, repo, revision)
            reject_configs(source); reject_configs(root_directory)
            for name in ("config", "config.toml"):
                if os.path.lexists(home / name):
                    raise GuardError(f"benchmark isolated Cargo home config exists: {home / name}")
            if native.descriptor(home) != home_descriptor:
                raise GuardError("benchmark isolated Cargo home protection changed")
            expected_home_entries = set(junctions) | {".package-cache", ".package-cache-mutate"}
            if set(os.listdir(home)) != expected_home_entries:
                raise GuardError("benchmark isolated Cargo home unexpected entry")
            for name, target in junctions.items():
                if not (home / name).is_junction() or str((home / name).resolve(strict=True)) != target:
                    raise GuardError("benchmark isolated Cargo cache junction changed")
            if digest_snapshot(source, native) != expected:
                raise GuardError("benchmark source snapshot changed during execution")
            verify_effective_protection(source, home)

        def verified_run(command, **options):
            verify()
            try:
                return run(command, cwd=root_directory, environment=options.pop("environment", environment),
                           log=commands, **options)
            finally:
                verify()

        print(f"artifact directory: {artifact}\nsource snapshot SHA-256: {expected}", flush=True)
        if fixture:
            print("FIXTURE ONLY: comparison qualification is NOT production performance evidence.", flush=True)
        environment = dict(os.environ, CARGO_HOME=str(home), PYTHONUTF8="1")
        binaries = {}
        directories = {}
        for profile_name, lto, units in (("default", "false", "16"), ("candidate", "thin", "1")):
            target = artifact / f"{profile_name}-target"
            directories[profile_name] = artifact / f"{profile_name}-runs"
            directories[profile_name].mkdir()
            build_environment = dict(environment, CARGO_TARGET_DIR=str(target),
                CARGO_PROFILE_RELEASE_LTO=lto, CARGO_PROFILE_RELEASE_CODEGEN_UNITS=units)
            verified_run([*cargo, "build", "--manifest-path", str(source / "Cargo.toml"), "-p", "magic-tdx-rs", "--example", "parse_bench", "--release", "--locked", "--offline"], environment=build_environment, timeout=10 if fixture else 1800)
            binaries[profile_name] = target / ("release/examples/parse_bench.py" if fixture else "release/examples/parse_bench.exe")
            if not binaries[profile_name].is_file():
                raise GuardError(f"benchmark binary is missing: {binaries[profile_name]}")

        def measure(profile_name, index=None):
            binary = binaries[profile_name]
            command = ([sys.executable, "-I", "-B", "-X", "utf8", str(binary)] if fixture else [str(binary)])
            output = None if index is None else directories[profile_name] / f"run-{index}.json"
            verified_run(command, timeout=10 if fixture else 60, output=output)

        measure("default"); measure("candidate")
        for index in range(1, 6):
            for profile_name in (("default", "candidate") if index % 2 else ("candidate", "default")):
                print(f"measuring {profile_name} run {index}", flush=True)
                measure(profile_name, index)
        verify()
        versions = {"rustc": verified_run([*rustc, "--version"]).decode("utf-8").strip(),
                    "cargo": verified_run([*cargo, "--version"]).decode("utf-8").strip()}
        verify()
        try:
            comparison = subprocess.run([
                sys.executable, "-I", "-B", "-X", "utf8",
                str(source / "tools/bench/compare_release_profiles.py"), "collect",
                "--default-dir", str(directories["default"]),
                "--candidate-dir", str(directories["candidate"]),
                "--default-binary", str(binaries["default"]),
                "--candidate-binary", str(binaries["candidate"]),
                "--evidence", str(artifact / "evidence.json"),
                "--report", str(artifact / "report.json"), "--revision", revision,
                "--rustc", versions["rustc"], "--cargo", versions["cargo"],
                "--platform", platform.platform()], cwd=root_directory,
                env=environment, capture_output=True, timeout=30)
        finally:
            verify()
        provenance = {"schema": 1, "fixture_only": fixture is not None,
            "production_performance_evidence": fixture is None, "revision": revision,
            "source_digest": expected, "snapshot_protection": "NTFS DACL + retained read-only sharing handles; protection-inclusive digest",
            "commands": commands, "comparison_native_exit": comparison.returncode,
            "comparison_stdout_b64": base64.b64encode(comparison.stdout).decode("ascii"),
            "comparison_stderr_b64": base64.b64encode(comparison.stderr).decode("ascii")}
        provenance_path = artifact / "runner-provenance.json"
        provenance_path.write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
        binding = {"fixture_only": fixture is not None, "production_performance_evidence": fixture is None,
                   "file": provenance_path.name, "sha256": hashlib.sha256(provenance_path.read_bytes()).hexdigest()}
        for name in ("evidence.json", "report.json"):
            path = artifact / name
            if path.exists():
                document = json.loads(path.read_text(encoding="utf-8"))
                document["execution_provenance"] = binding
                path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(comparison.stdout.decode("utf-8", errors="backslashreplace"), end="", flush=True)
        if comparison.stderr:
            print(comparison.stderr.decode("utf-8", errors="backslashreplace"), file=sys.stderr, end="")
        print(f"evidence: {artifact / 'evidence.json'}\nreport: {artifact / 'report.json'}", flush=True)
        return comparison.returncode
    finally:
        (artifact / "native-command-log.json").write_text(json.dumps(commands, indent=2) + "\n", encoding="utf-8")
        native.close()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CommandError as failure:
        print(str(failure), file=sys.stderr, flush=True)
        raise SystemExit(failure.native_exit)
    except (GuardError, OSError, ValueError, subprocess.SubprocessError) as failure:
        print(str(failure), file=sys.stderr, flush=True)
        raise SystemExit(2)
