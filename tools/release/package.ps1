#requires -Version 7.3
[CmdletBinding()]
param(
    [string]$DistributionDirectory,
    [string]$CargoExecutable = 'cargo.exe',
    [string]$RustcExecutable = 'rustc.exe',
    [string]$GitExecutable = 'git.exe'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$nativeFailureExit = 1

function Assert-LocalPath([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if ($absolute.StartsWith('\\') -or -not [IO.Path]::IsPathFullyQualified($absolute)) {
        throw 'release paths must be fully qualified local Windows paths'
    }
    $cursor = $absolute
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "release path contains a reparse point: $cursor"
            }
        }
        $parent = [IO.Directory]::GetParent($cursor)
        $cursor = if ($null -eq $parent) { $null } else { $parent.FullName }
    }
    return $absolute
}

function Resolve-NativeExecutable([string]$Name) {
    $command = Get-Command -Name $Name -CommandType Application -ErrorAction Stop | Select-Object -First 1
    $toolItem = Get-Item -LiteralPath $command.Source -Force
    $path = $command.Source
    if ($toolItem.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        # Installed rustup proxies can be symlinks. Resolve their current active
        # tool without selecting/installing a toolchain or executing rustup as Cargo.
        $target = $toolItem.ResolveLinkTarget($true)
        if ($null -eq $target -or $target.Name -ine 'rustup.exe' -or $toolItem.Name -notin @('cargo.exe', 'rustc.exe')) {
            throw "unsupported native executable link: $Name"
        }
        $rustup = Assert-LocalPath $target.FullName
        $toolName = [IO.Path]::GetFileNameWithoutExtension($toolItem.Name)
        $path = (Invoke-Native $rustup @('which', $toolName) -Capture).Trim()
        if ([IO.Path]::GetFileName($path) -ine $toolItem.Name) { throw 'rustup returned a different native tool' }
    }
    $path = Assert-LocalPath $path
    if ([IO.Path]::GetExtension($path) -ine '.exe') {
        throw "release tooling requires native executables: $Name"
    }
    return $path
}

function Invoke-Native([string]$Executable, [string[]]$Arguments, [switch]$Capture) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.WorkingDirectory = $repositoryRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    if ($Capture) {
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $stdoutRaw = $null
    $stderrRaw = $null
    try {
        if (-not $process.Start()) { throw 'unable to start native release command' }
        if ($Capture) {
            $stdoutRaw = [IO.MemoryStream]::new()
            $stderrRaw = [IO.MemoryStream]::new()
            $stdoutTask = $process.StandardOutput.BaseStream.CopyToAsync($stdoutRaw)
            $stderrTask = $process.StandardError.BaseStream.CopyToAsync($stderrRaw)
        }
        # The calling recovery controller owns the bounded, non-breakaway Job.
        # This script never looks up or terminates an unrelated process by PID.
        $process.WaitForExit()
        $exitCode = $process.ExitCode
        if ($exitCode -ne 0) { $script:nativeFailureExit = $exitCode }
        if ($Capture) {
            [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]] @($stdoutTask, $stderrTask))
            $stdoutBytes = $stdoutRaw.ToArray()
            $stderrBytes = $stderrRaw.ToArray()
            # Raw stderr is forwarded without a locale/UTF-8 decoder. The
            # enclosing recovery controller preserves these original bytes.
            if ($stderrBytes.Length) {
                [Console]::OpenStandardError().Write($stderrBytes, 0, $stderrBytes.Length)
            }
        }
        if ($exitCode -ne 0) {
            if ($Capture -and $stdoutBytes.Length) {
                [Console]::OpenStandardOutput().Write($stdoutBytes, 0, $stdoutBytes.Length)
            }
            throw "native release command failed: $Executable exit=$exitCode"
        }
        if ($Capture) {
            try { return [Text.UTF8Encoding]::new($false, $true).GetString($stdoutBytes) }
            catch {
                if ($stdoutBytes.Length) { [Console]::OpenStandardOutput().Write($stdoutBytes, 0, $stdoutBytes.Length) }
                throw 'native stdout is not valid UTF-8; original bytes forwarded'
            }
        }
    }
    finally {
        if ($null -ne $stdoutRaw) { $stdoutRaw.Dispose() }
        if ($null -ne $stderrRaw) { $stderrRaw.Dispose() }
        $process.Dispose()
    }
}

function Write-PackageManifest([string] $DistributionRoot) {
    $packagedPaths = [Collections.Generic.List[string]]::new()
    foreach ($file in Get-ChildItem -LiteralPath $DistributionRoot -Recurse -File -Force) {
        $packagedPaths.Add($file.FullName.Substring($DistributionRoot.Length + 1).Replace('\', '/'))
    }
    $packagedPaths.Sort([StringComparer]::Ordinal)
    $manifestLines = @(foreach ($relative in $packagedPaths) {
        if ($relative.Contains("`n") -or $relative.Contains("`r") -or $relative.Contains('\')) { throw 'unsupported SHA256 manifest filename' }
        $packagedPath = Assert-LocalPath (Join-Path $DistributionRoot $relative)
        $sha = (Get-FileHash -LiteralPath $packagedPath -Algorithm SHA256).Hash.ToLowerInvariant()
        "$sha  $relative"
    })
    $manifestBytes = [Text.UTF8Encoding]::new($false).GetBytes(($manifestLines -join "`n") + "`n")
    $manifest = [IO.File]::Open((Join-Path $DistributionRoot 'SHA256SUMS'), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try { $manifest.Write($manifestBytes, 0, $manifestBytes.Length) }
    finally { $manifest.Dispose() }
}

function Assert-CleanBuildInputs {
    Invoke-Native $git @('diff', '--quiet')
    Invoke-Native $git @('diff', '--cached', '--quiet')
    $untracked = Invoke-Native $git @('ls-files', '--others', '--exclude-standard', '-z', '--',
        '.cargo', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml', 'crates', 'tools/release') -Capture
    if ($untracked.Length -ne 0) { throw 'release packaging rejects untracked build inputs' }
    foreach ($name in @('.cargo/config', '.cargo/config.toml')) {
        if (Test-Path -LiteralPath (Join-Path $repositoryRoot $name)) {
            Invoke-Native $git @('ls-files', '--error-unmatch', '--', $name)
        }
    }
    if ($script:revision) {
        $current = (Invoke-Native $git @('rev-parse', 'HEAD') -Capture).Trim()
        if ($current -cne $script:revision) { throw 'release revision changed during packaging' }
    }
}

function Copy-PackagedFile([string]$Source, [string]$RelativeDestination) {
    $packageInput = Assert-LocalPath $Source
    $destination = Assert-LocalPath (Join-Path $distributionRoot $RelativeDestination)
    if (-not $destination.StartsWith($distributionRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'packaged destination escapes distribution root'
    }
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination)) | Out-Null
    [IO.File]::Copy($packageInput, $destination, $false)
}

function Build-Product([string]$Package, [string]$Kind, [string]$Name, [string]$InstalledName) {
    Assert-CleanBuildInputs
    Invoke-Native $cargo @('build', '-p', $Package, "--$Kind", $Name,
        '--release', '--locked', '--offline', '--target', $hostTriple)
    Assert-CleanBuildInputs
    $builtRoot = Join-Path $packageTarget "$hostTriple/release"
    if ($Kind -eq 'example') { $builtRoot = Join-Path $builtRoot 'examples' }
    Copy-PackagedFile (Join-Path $builtRoot "$Name.exe") "bin/$InstalledName.exe"
}

$script:revision = $null
$savedRevision = [Environment]::GetEnvironmentVariable('MAGIC_MARKET_SOURCE_REVISION', 'Process')
$savedTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')
try {
    if (-not $IsWindows) { throw 'package.ps1 is the Windows-native packaging entry point' }
    $git = Resolve-NativeExecutable $GitExecutable
    $cargo = Resolve-NativeExecutable $CargoExecutable
    $rustc = Resolve-NativeExecutable $RustcExecutable
    Assert-CleanBuildInputs
    $script:revision = (Invoke-Native $git @('rev-parse', 'HEAD') -Capture).Trim()
    if ($script:revision -cnotmatch '^[0-9a-f]{40}$') { throw 'release requires a full lowercase Git SHA-1' }
    $rustVersion = Invoke-Native $rustc @('-vV') -Capture
    $hostMatch = [regex]::Match($rustVersion, '(?m)^host: ([A-Za-z0-9_.-]+)\r?$')
    if (-not $hostMatch.Success) { throw 'unable to determine a safe Rust host triple' }
    $hostTriple = $hostMatch.Groups[1].Value
    if ($hostTriple -notmatch '-windows-msvc$') { throw 'Windows release requires the existing Windows MSVC toolchain' }
    $distParent = Assert-LocalPath (Join-Path $repositoryRoot 'target/dist')
    if (-not $DistributionDirectory) { $DistributionDirectory = Join-Path $distParent $script:revision }
    $distributionRoot = Assert-LocalPath $DistributionDirectory
    if (-not $distributionRoot.StartsWith($distParent + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Windows release output must stay inside this repository target/dist'
    }
    if (Test-Path -LiteralPath $distributionRoot) { throw 'release output already exists; preserve previous evidence' }
    $packageTarget = Assert-LocalPath (Join-Path $repositoryRoot "target/package-build/$($script:revision).$([guid]::NewGuid().ToString('N'))")
    if (Test-Path -LiteralPath $packageTarget) { throw 'new package build path already exists' }
    [IO.Directory]::CreateDirectory($packageTarget) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $distributionRoot 'bin')) | Out-Null
    [Environment]::SetEnvironmentVariable('MAGIC_MARKET_SOURCE_REVISION', $script:revision, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $packageTarget, 'Process')

    $binaries = @(
        @('magic-market-grpc-server', 'magic-market-grpc-server'),
        @('magic-market-monitor-server', 'magic-market-monitor-server'),
        @('magic-tdx-native-bridge', 'magic-tdx-native-bridge'),
        @('magic-market-tdx-agent', 'magic-market-tdx-agent')
    )
    foreach ($binary in $binaries) { Build-Product $binary[0] 'bin' $binary[1] $binary[1] }
    $probes = @(
        @('magic-tdx-rs', 'live_probe', 'magic-tdx-live-probe'),
        @('magic-tdx-rs', 'security_profile_probe', 'magic-tdx-security-profile-probe'),
        @('magic-emquant-rs', 'live_probe', 'magic-emquant-live-probe'),
        @('magic-tencent-rs', 'live_probe', 'magic-tencent-live-probe'),
        @('magic-tencent-rs', 'load_probe', 'magic-tencent-load-probe'),
        @('magic-sina-rs', 'live_probe', 'magic-sina-live-probe'),
        @('magic-sina-rs', 'load_probe', 'magic-sina-load-probe'),
        @('magic-market-router', 'live_probe', 'magic-router-live-probe'),
        @('magic-market-composition', 'provider_top_n_live_probe', 'magic-provider-topn-live-probe'),
        @('magic-eastmoney-rs', 'live_probe', 'magic-eastmoney-live-probe'),
        @('magic-eastmoney-rs', 'load_probe', 'magic-eastmoney-load-probe'),
        @('magic-cninfo-rs', 'live_probe', 'magic-cninfo-live-probe'),
        @('magic-cninfo-rs', 'load_probe', 'magic-cninfo-load-probe'),
        @('magic-ths-rs', 'live_probe', 'magic-ths-live-probe'),
        @('magic-ths-rs', 'load_probe', 'magic-ths-load-probe'),
        @('magic-cls-rs', 'live_probe', 'magic-cls-live-probe'),
        @('magic-cls-rs', 'load_probe', 'magic-cls-load-probe'),
        @('magic-jin10-rs', 'live_probe', 'magic-jin10-live-probe'),
        @('magic-jin10-rs', 'load_probe', 'magic-jin10-load-probe'),
        @('magic-thepaper-rs', 'live_probe', 'magic-thepaper-live-probe'),
        @('magic-thepaper-rs', 'load_probe', 'magic-thepaper-load-probe'),
        @('magic-nbs-rs', 'live_probe', 'magic-nbs-live-probe'),
        @('magic-pbc-rs', 'live_probe', 'magic-pbc-live-probe'),
        @('magic-pbc-rs', 'load_probe', 'magic-pbc-load-probe'),
        @('magic-cfets-rs', 'live_probe', 'magic-cfets-live-probe'),
        @('magic-cfets-rs', 'load_probe', 'magic-cfets-load-probe'),
        @('magic-fred-rs', 'live_probe', 'magic-fred-live-probe'),
        @('magic-fred-rs', 'load_probe', 'magic-fred-load-probe'),
        @('magic-imf-rs', 'live_probe', 'magic-imf-live-probe'),
        @('magic-imf-rs', 'load_probe', 'magic-imf-load-probe'),
        @('magic-worldbank-rs', 'live_probe', 'magic-worldbank-live-probe'),
        @('magic-sec-rs', 'live_probe', 'magic-sec-live-probe'),
        @('magic-sec-rs', 'load_probe', 'magic-sec-load-probe'),
        @('magic-xinhua-rs', 'live_probe', 'magic-xinhua-live-probe'),
        @('magic-xinhua-rs', 'load_probe', 'magic-xinhua-load-probe'),
        @('magic-yicai-rs', 'live_probe', 'magic-yicai-live-probe'),
        @('magic-yicai-rs', 'load_probe', 'magic-yicai-load-probe'),
        @('magic-stcn-rs', 'live_probe', 'magic-stcn-live-probe'),
        @('magic-stcn-rs', 'load_probe', 'magic-stcn-load-probe'),
        @('magic-yonhap-rs', 'live_probe', 'magic-yonhap-live-probe'),
        @('magic-yonhap-rs', 'load_probe', 'magic-yonhap-load-probe'),
        @('magic-wallstreetcn-rs', 'live_probe', 'magic-wallstreetcn-live-probe'),
        @('magic-wallstreetcn-rs', 'load_probe', 'magic-wallstreetcn-load-probe'),
        @('magic-baidu-rs', 'live_probe', 'magic-baidu-live-probe'),
        @('magic-baidu-rs', 'load_probe', 'magic-baidu-load-probe'),
        @('magic-iwencai-rs', 'live_probe', 'magic-iwencai-live-probe'),
        @('magic-iwencai-rs', 'load_probe', 'magic-iwencai-load-probe'),
        @('magic-exchange-rs', 'live_probe', 'magic-exchange-live-probe'),
        @('magic-exchange-rs', 'load_probe', 'magic-exchange-load-probe'),
        @('magic-gov-rs', 'live_probe', 'magic-gov-live-probe')
    )
    foreach ($probe in $probes) { Build-Product $probe[0] 'example' $probe[1] $probe[2] }
    $trackedDocs = (Invoke-Native $git @('ls-files', '-z', '--', 'docs') -Capture).Split([char]0, [StringSplitOptions]::RemoveEmptyEntries)
    foreach ($doc in $trackedDocs) { Copy-PackagedFile (Join-Path $repositoryRoot $doc) $doc }
    foreach ($name in @('LICENSE-MIT', 'LICENSE-APACHE')) { Copy-PackagedFile (Join-Path $repositoryRoot $name) "licenses/$name" }
    Copy-PackagedFile (Join-Path $repositoryRoot 'LICENSES/tdxrs-MIT.txt') 'licenses/tdxrs-MIT.txt'
    foreach ($name in @('README.md', 'Cargo.lock')) { Copy-PackagedFile (Join-Path $repositoryRoot $name) $name }
    Copy-PackagedFile (Join-Path $repositoryRoot 'crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto') 'proto/magic/market/v1/market.proto'
    [IO.File]::WriteAllText((Join-Path $distributionRoot 'RELEASE_REVISION'), $script:revision + "`n", [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $distributionRoot 'TARGET_TRIPLE'), $hostTriple + "`n", [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $distributionRoot 'RUSTC_VERSION'), $rustVersion, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $distributionRoot 'CARGO_VERSION'), (Invoke-Native $cargo @('-V') -Capture), [Text.UTF8Encoding]::new($false))
    Assert-CleanBuildInputs
    Write-PackageManifest $distributionRoot
    Write-Output "release package: $distributionRoot"
    Write-Output "retained isolated package target: $packageTarget"
    exit 0
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit $nativeFailureExit
}
finally {
    [Environment]::SetEnvironmentVariable('MAGIC_MARKET_SOURCE_REVISION', $savedRevision, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $savedTarget, 'Process')
}
