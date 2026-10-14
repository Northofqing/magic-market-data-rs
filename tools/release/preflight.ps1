#requires -Version 7.3
[CmdletBinding()]
param(
    [string] $CoverageJson = $env:MAGIC_COVERAGE_JSON,
    [string] $EvidenceDirectory = '',
    [string] $PythonExecutable = '',
    [string] $CargoExecutable = '',
    [string] $RustcExecutable = '',
    [string] $GitExecutable = '',
    [string] $RgExecutable = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$records = [Collections.Generic.List[object]]::new()
$utf8 = [Text.UTF8Encoding]::new($false)
$runDirectory = $null
$targetDirectory = $null
$result = 1

function Resolve-NativeTool([string] $Configured, [string[]] $Names) {
    if ($Configured) {
        $resolved = (Resolve-Path -LiteralPath $Configured).Path
        if ([IO.Path]::GetExtension($resolved) -ne '.exe') { throw 'a native .exe tool is required' }
        return $resolved
    }
    foreach ($name in $Names) {
        $command = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($command) { return $command.Source }
    }
    throw ('missing native tool: ' + ($Names -join ', '))
}

function Assert-ContainedPath([string] $Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    $prefix = $root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $absolute.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'preflight artifact path must be inside the repository'
    }
    $cursor = $absolute
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'reparse-point artifact path is not allowed' }
        }
        $cursor = [IO.Path]::GetDirectoryName($cursor)
    }
    return $absolute
}

function Write-NewJson([string] $Path, [object] $Value) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $bytes = $utf8.GetBytes(($Value | ConvertTo-Json -Depth 12) + "`n")
        $stream.Write($bytes, 0, $bytes.Length)
    } finally { $stream.Dispose() }
}

function Invoke-Check([string] $Name, [string] $Executable, [string[]] $Arguments, [hashtable] $Environment = @{}) {
    $number = $records.Count + 1
    $stem = '{0:00}-{1}' -f $number, $Name
    $stdoutPath = Join-Path $runDirectory ($stem + '.stdout.bin')
    $stderrPath = Join-Path $runDirectory ($stem + '.stderr.bin')
    $stdout = [IO.File]::Open($stdoutPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $stderr = [IO.File]::Open($stderrPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $process = [Diagnostics.Process]::new()
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.WorkingDirectory = $root
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $start.Environment['PYTHONUTF8'] = '1'
    $start.Environment['PYTHONIOENCODING'] = 'utf-8'
    foreach ($key in $Environment.Keys) { $start.Environment[$key] = [string] $Environment[$key] }
    $process.StartInfo = $start
    $started = [DateTimeOffset]::UtcNow
    $exitCode = $null
    $startedProcess = $false
    try {
        Write-Output "preflight: $Name"
        $startedProcess = $process.Start()
        if (-not $startedProcess) { throw 'native process did not start' }
        $outputTask = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
        $errorTask = $process.StandardError.BaseStream.CopyToAsync($stderr)
        $process.WaitForExit()
        [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]] @($outputTask, $errorTask))
        $exitCode = $process.ExitCode
    } finally {
        $stdout.Dispose()
        $stderr.Dispose()
        $process.Dispose()
        $record = [ordered] @{
            name = $Name; executable = $Executable; arguments = $Arguments
            started_utc = $started.ToString('o'); ended_utc = [DateTimeOffset]::UtcNow.ToString('o')
            started = $startedProcess; native_exit_code = $exitCode
            stdout = $stdoutPath; stderr = $stderrPath
            stdout_sha256 = (Get-FileHash -LiteralPath $stdoutPath -Algorithm SHA256).Hash.ToLowerInvariant()
            stderr_sha256 = (Get-FileHash -LiteralPath $stderrPath -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        $records.Add($record)
        Write-NewJson (Join-Path $runDirectory ($stem + '.json')) $record
    }
    # Raw bytes above are authoritative; only short UTF-8 tails are presented.
    foreach ($path in @($stdoutPath, $stderrPath)) {
        $length = (Get-Item -LiteralPath $path).Length
        $offset = [Math]::Max(0, $length - 8192)
        $file = [IO.File]::OpenRead($path)
        try {
            [void] $file.Seek($offset, [IO.SeekOrigin]::Begin)
            $buffer = [byte[]]::new([int] ($length - $offset))
            [void] $file.Read($buffer, 0, $buffer.Length)
            if ($buffer.Length) { Write-Output ($utf8.GetString($buffer)) }
        } finally { $file.Dispose() }
    }
    if ($exitCode -ne 0) {
        $exception = [Exception]::new("preflight check failed (exit=$exitCode): $Name")
        $exception.Data['NativeExitCode'] = $exitCode
        throw $exception
    }
}

try {
    if (-not $IsWindows) { throw 'preflight.ps1 is the Windows-native release gate; use preflight.sh on POSIX' }
    # Release coverage is mandatory on this path, regardless of the optional
    # developer-mode switch supported by the POSIX wrapper.
    if (-not $CoverageJson -or -not (Test-Path -LiteralPath $CoverageJson -PathType Leaf)) {
        throw 'release preflight requires existing MAGIC_COVERAGE_JSON evidence or -CoverageJson'
    }
    $coveragePath = (Resolve-Path -LiteralPath $CoverageJson).Path
    $python = Resolve-NativeTool $PythonExecutable @('py.exe', 'python3.exe', 'python.exe')
    $cargo = Resolve-NativeTool $CargoExecutable @('cargo.exe')
    $rustc = Resolve-NativeTool $RustcExecutable @('rustc.exe')
    $git = Resolve-NativeTool $GitExecutable @('git.exe')
    $rg = Resolve-NativeTool $RgExecutable @('rg.exe')
    $pwsh = Join-Path $PSHOME 'pwsh.exe'
    $pythonPrefix = @()
    if ([IO.Path]::GetFileName($python) -ieq 'py.exe') { $pythonPrefix = @('-3') }

    $runId = 'run.' + [Guid]::NewGuid().ToString('N')
    $targetDirectory = Assert-ContainedPath (Join-Path $root ("target/preflight/$runId"))
    $base = if ($EvidenceDirectory) { $EvidenceDirectory } else { Join-Path $root 'target/preflight' }
    $runDirectory = Assert-ContainedPath (Join-Path ([IO.Path]::GetFullPath($base)) $runId)
    if ((Test-Path -LiteralPath $targetDirectory) -or (Test-Path -LiteralPath $runDirectory)) { throw 'fresh preflight run path already exists' }
    [void] [IO.Directory]::CreateDirectory($targetDirectory)
    if ($runDirectory -ne $targetDirectory) { [void] [IO.Directory]::CreateDirectory($runDirectory) }
    Write-NewJson (Join-Path $runDirectory 'inputs.json') ([ordered] @{
        platform = 'windows-native'; target = 'x86_64-pc-windows-msvc'
        source_root = $root; target_directory = $targetDirectory
        coverage = $coveragePath; coverage_sha256 = (Get-FileHash -LiteralPath $coveragePath -Algorithm SHA256).Hash.ToLowerInvariant()
        posix_scripts = 'retained; their Bash execution/syntax is not claimed by this native gate'
        scripts = @(Get-ChildItem -LiteralPath (Join-Path $root 'tools') -Filter '*.ps1' -File -Recurse | Sort-Object FullName | ForEach-Object {
            @{ path = $_.FullName; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
        })
    })

    Invoke-Check 'rustc-version' $rustc @('-vV')
    $versionOutput = [IO.File]::ReadAllText((Join-Path $runDirectory '01-rustc-version.stdout.bin'), $utf8)
    if (-not [regex]::IsMatch($versionOutput, '(?m)^host: x86_64-pc-windows-msvc\r?$')) { throw 'Rust host must be x86_64-pc-windows-msvc' }
    Invoke-Check 'cargo-version' $cargo @('-V')
    Invoke-Check 'format' $cargo @('fmt', '--all', '--', '--check')
    # Native parser for every PowerShell tool, including packaging/benchmark.
    foreach ($script in Get-ChildItem -LiteralPath (Join-Path $root 'tools') -Filter '*.ps1' -File -Recurse) {
        $tokens = $null
        $errors = $null
        [void] [Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref] $tokens, [ref] $errors)
        if ($errors.Count) { throw "PowerShell syntax failed: $($script.FullName): $($errors[0].Message)" }
    }
    Write-NewJson (Join-Path $runDirectory 'native-syntax.json') @{ native_parser_passed = $true; posix_bash_syntax = 'not executed on Windows; retained for POSIX CI' }
    # Discover every tools directory containing regression tests. This retains
    # all three original suites and also executes the new native/tooling suites.
    $testDirectories = @(Get-ChildItem -LiteralPath (Join-Path $root 'tools') -Filter 'test_*.py' -File -Recurse | ForEach-Object { $_.DirectoryName } | Sort-Object -Unique)
    foreach ($directory in $testDirectories) {
        $relative = [IO.Path]::GetRelativePath($root, $directory).Replace('\', '/')
        $name = 'python-' + $relative.Replace('/', '-')
        Invoke-Check $name $python ($pythonPrefix + @('-m', 'unittest', 'discover', '-s', $relative, '-p', 'test_*.py'))
    }
    Invoke-Check 'coverage-thresholds' $python ($pythonPrefix + @('tools/coverage/check_thresholds.py', $coveragePath))
    $buildEnvironment = @{ CARGO_TARGET_DIR = $targetDirectory }
    $buildArguments = @('--workspace', '--all-targets', '--all-features', '--locked', '--offline', '--target', 'x86_64-pc-windows-msvc')
    Invoke-Check 'cargo-check' $cargo (@('check') + $buildArguments) $buildEnvironment
    Invoke-Check 'cargo-test' $cargo (@('test') + $buildArguments + @('--', '--test-threads=1')) $buildEnvironment
    Invoke-Check 'cargo-clippy' $cargo (@('clippy') + $buildArguments + @('--', '-D', 'warnings')) $buildEnvironment
    Invoke-Check 'cargo-doc' $cargo @('doc', '--workspace', '--all-features', '--no-deps', '--locked', '--offline', '--target', 'x86_64-pc-windows-msvc') @{ CARGO_TARGET_DIR = $targetDirectory; RUSTDOCFLAGS = '-D warnings' }
    Invoke-Check 'cargo-doctest' $cargo @('test', '--workspace', '--all-features', '--doc', '--locked', '--offline', '--target', 'x86_64-pc-windows-msvc', '--', '--test-threads=1') $buildEnvironment
    Invoke-Check 'documentation-links' $pwsh @('-NoLogo', '-NoProfile', '-File', (Join-Path $root 'tools/docs/check_links.ps1'), '-RgExecutable', $rg)
    Invoke-Check 'compliance' $pwsh @('-NoLogo', '-NoProfile', '-File', (Join-Path $root 'tools/compliance/check.ps1'), '-PythonExecutable', $python, '-RgExecutable', $rg)
    Invoke-Check 'git-diff-check' $git @('diff', '--check')
    if ((Get-FileHash -LiteralPath $coveragePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne
        (Get-Content -LiteralPath (Join-Path $runDirectory 'inputs.json') -Raw | ConvertFrom-Json).coverage_sha256) {
        throw 'coverage input changed during preflight'
    }
    $result = 0
    Write-Output 'Windows-native release preflight: passed'
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    if ($_.Exception.Data.Contains('NativeExitCode') -and $null -ne $_.Exception.Data['NativeExitCode']) {
        $result = [int] $_.Exception.Data['NativeExitCode']
    }
} finally {
    if ($runDirectory -and (Test-Path -LiteralPath $runDirectory -PathType Container)) {
        Write-NewJson (Join-Path $runDirectory 'receipt.json') ([ordered] @{
            status = $(if ($result -eq 0) { 'passed' } else { 'failed' })
            native_exit_code = $result; target_directory = $targetDirectory
            checks = $records.ToArray(); artifacts_retained = $true
        })
        Write-Output "preflight evidence retained: $runDirectory"
    }
}
exit $result
