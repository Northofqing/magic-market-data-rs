#requires -Version 7.3
[CmdletBinding()]
param(
    [string] $RepositoryRoot = (Join-Path $PSScriptRoot '../..'),
    [string] $PythonExecutable = '',
    [string] $RgExecutable = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
$PSNativeCommandUseErrorActionPreference = $false
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path

function Resolve-NativeTool([string] $Configured, [string[]] $Names) {
    if ($Configured) {
        $resolved = (Resolve-Path -LiteralPath $Configured).Path
        if ([IO.Path]::GetExtension($resolved) -ne '.exe') {
            throw 'a native .exe tool is required'
        }
        return $resolved
    }
    foreach ($name in $Names) {
        $command = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($command) { return $command.Source }
    }
    throw ('missing native tool: ' + ($Names -join ', '))
}

function Invoke-Required([string] $Executable, [string[]] $Arguments) {
    & $Executable @Arguments
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        $exception = [Exception]::new("native check failed (exit=$code): $Executable")
        $exception.Data['NativeExitCode'] = $code
        throw $exception
    }
}

function Assert-NoRgMatch([string] $Pattern, [string[]] $Paths, [string] $Message) {
    & $rg '-n' '--' $Pattern @Paths
    $code = $LASTEXITCODE
    if ($code -eq 0) { throw $Message }
    if ($code -ne 1) {
        $exception = [Exception]::new("ripgrep failed (exit=$code)")
        $exception.Data['NativeExitCode'] = $code
        throw $exception
    }
}

$required = @(
    'AGENTS.md'
    'Cargo.toml'
    'Cargo.lock'
    'LICENSE-APACHE'
    'LICENSE-MIT'
    'LICENSES/tdxrs-MIT.txt'
    'docs/ENGINEERING_RULES.md'
    'docs/business_rules.md'
    'docs/DEPLOYMENT.md'
    'docs/MULTI_PROVIDER_ROUTING.md'
    'docs/integrations/tencent-web.md'
    'docs/integrations/README.md'
    'docs/integrations/admissions.tsv'
    'docs/integrations/async-blocking.md'
    'docs/integrations/http-transports.tsv'
    'docs/integrations/tdx-local-terminal.md'
    'docs/integrations/tdx-local-terminal-compatibility.tsv'
    'docs/integrations/sina-web.md'
    'docs/integrations/eastmoney-web.md'
    'docs/integrations/cninfo-web.md'
    'docs/integrations/tonghuashun-web.md'
    'docs/integrations/cls-web.md'
    'docs/integrations/jin10-web.md'
    'docs/integrations/thepaper-web.md'
    'docs/integrations/yonhap-rss.md'
    'docs/integrations/wallstreetcn-rss.md'
    'docs/integrations/baidu-web.md'
    'docs/integrations/iwencai-api.md'
    'docs/integrations/exchange-official.md'
    'docs/integrations/level2-auction.md'
    'docs/integrations/broker-account-boundary.md'
    'docs/integrations/gov-policy.md'
    'docs/integrations/nbs-official.md'
    'docs/integrations/pbc-official.md'
    'docs/integrations/cfets-official.md'
    'docs/integrations/fred-api.md'
    'docs/integrations/imf-datamapper.md'
    'docs/integrations/worldbank-indicators.md'
    'docs/integrations/sec-edgar.md'
    'docs/integrations/xinhua-finance.md'
    'docs/integrations/yicai-news.md'
    'docs/integrations/securities-times.md'
    'crates/magic-market-transport/Cargo.toml'
    'crates/magic-nbs-rs/Cargo.toml'
    'crates/magic-pbc-rs/Cargo.toml'
    'crates/magic-cfets-rs/Cargo.toml'
    'crates/magic-fred-rs/Cargo.toml'
    'crates/magic-imf-rs/Cargo.toml'
    'crates/magic-worldbank-rs/Cargo.toml'
    'crates/magic-sec-rs/Cargo.toml'
    'crates/magic-xinhua-rs/Cargo.toml'
    'crates/magic-yicai-rs/Cargo.toml'
    'crates/magic-stcn-rs/Cargo.toml'
    'crates/magic-market-router/Cargo.toml'
    'crates/magic-market-composition/Cargo.toml'
    'crates/magic-market-analysis/Cargo.toml'
    'crates/magic-tencent-rs/Cargo.toml'
    'crates/magic-sina-rs/Cargo.toml'
    'crates/magic-eastmoney-rs/Cargo.toml'
    'crates/magic-cninfo-rs/Cargo.toml'
    'crates/magic-ths-rs/Cargo.toml'
    'crates/magic-cls-rs/Cargo.toml'
    'crates/magic-jin10-rs/Cargo.toml'
    'crates/magic-thepaper-rs/Cargo.toml'
    'crates/magic-yonhap-rs/Cargo.toml'
    'crates/magic-wallstreetcn-rs/Cargo.toml'
    'crates/magic-tdx-local-rs/Cargo.toml'
    'crates/magic-tdx-native-bridge/Cargo.toml'
    'crates/magic-market-monitor/Cargo.toml'
    'crates/magic-market-monitor-server/Cargo.toml'
    'crates/magic-market-grpc-contracts/Cargo.toml'
    'crates/magic-market-service/Cargo.toml'
    'crates/magic-market-grpc-server/Cargo.toml'
    'crates/magic-market-tdx-agent/Cargo.toml'
    'crates/magic-baidu-rs/Cargo.toml'
    'crates/magic-iwencai-rs/Cargo.toml'
    'crates/magic-exchange-rs/Cargo.toml'
    'crates/magic-gov-rs/Cargo.toml'
)
$workspaceMembers = @(
    'crates/magic-market-core'
    'crates/magic-market-router'
    'crates/magic-market-composition'
    'crates/magic-tdx-rs'
    'crates/magic-emquant-rs'
    'crates/magic-tencent-rs'
    'crates/magic-sina-rs'
    'crates/magic-market-analysis'
    'crates/magic-eastmoney-rs'
    'crates/magic-cninfo-rs'
    'crates/magic-ths-rs'
    'crates/magic-cls-rs'
    'crates/magic-jin10-rs'
    'crates/magic-thepaper-rs'
    'crates/magic-baidu-rs'
    'crates/magic-iwencai-rs'
    'crates/magic-exchange-rs'
    'crates/magic-gov-rs'
    'crates/magic-yonhap-rs'
    'crates/magic-wallstreetcn-rs'
    'crates/magic-tdx-local-rs'
    'crates/magic-tdx-native-bridge'
    'crates/magic-market-monitor'
    'crates/magic-market-monitor-server'
    'crates/magic-market-grpc-contracts'
    'crates/magic-market-service'
    'crates/magic-market-grpc-server'
    'crates/magic-market-tdx-agent'
    'crates/magic-market-transport'
    'crates/magic-nbs-rs'
    'crates/magic-pbc-rs'
    'crates/magic-cfets-rs'
    'crates/magic-fred-rs'
    'crates/magic-imf-rs'
    'crates/magic-worldbank-rs'
    'crates/magic-sec-rs'
    'crates/magic-xinhua-rs'
    'crates/magic-yicai-rs'
    'crates/magic-stcn-rs'
)
$pythonChecks = @(
    'tools/compliance/check_admissions.py'
    'tools/compliance/check_http_transports.py'
    'tools/compliance/check_tdx_local_compatibility.py'
    'tools/compliance/check_tdx_native_boundary.py'
    'tools/compliance/check_grpc_services.py'
)

$savedPythonUtf8 = [Environment]::GetEnvironmentVariable('PYTHONUTF8')
$savedPythonEncoding = [Environment]::GetEnvironmentVariable('PYTHONIOENCODING')
$result = 0
Push-Location -LiteralPath $root
try {
    foreach ($relative in $required) {
        $path = Join-Path $root $relative
        if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-Item -LiteralPath $path).Length -eq 0) {
            throw "missing required file: $relative"
        }
    }
    $python = Resolve-NativeTool $PythonExecutable @('py.exe', 'python3.exe', 'python.exe')
    $rg = Resolve-NativeTool $RgExecutable @('rg.exe')
    $pythonPrefix = @()
    if ([IO.Path]::GetFileName($python) -ieq 'py.exe') { $pythonPrefix = @('-3') }
    $env:PYTHONUTF8 = '1'
    $env:PYTHONIOENCODING = 'utf-8'
    foreach ($check in $pythonChecks) { Invoke-Required $python ($pythonPrefix + @($check)) }

    foreach ($selector in @('rust-toolchain', 'rust-toolchain.toml')) {
        if (Test-Path -LiteralPath (Join-Path $root $selector)) {
            throw "fixed repository toolchain selector is not allowed: $selector"
        }
    }
    $crateManifests = @(Get-ChildItem -LiteralPath (Join-Path $root 'crates') -Directory | ForEach-Object {
        $path = Join-Path $_.FullName 'Cargo.toml'
        if (Test-Path -LiteralPath $path -PathType Leaf) { $path }
    } | Sort-Object -CaseSensitive)
    Assert-NoRgMatch '^[[:space:]]*rust-version[[:space:]]*=' (@('Cargo.toml') + $crateManifests) 'Cargo manifests must not declare a fixed Rust version'
    Assert-NoRgMatch 'dtolnay/rust-toolchain@[0-9]+([.][0-9]+)*' @('.github/workflows') 'active workflows must use the stable Rust toolchain'
    Assert-NoRgMatch 'RUSTUP_TOOLCHAIN=[0-9]+([.][0-9]+)*|cargo[[:space:]]+[+][0-9]+([.][0-9]+)*' @('.github/workflows', 'tools') 'active build and release tooling must not select a numeric Rust version'

    $manifest = [IO.File]::ReadAllText((Join-Path $root 'Cargo.toml'))
    $membersBlock = [regex]::Match($manifest, '(?ms)^members = \[.*?^\]')
    if (-not $membersBlock.Success) { throw 'missing workspace members block' }
    foreach ($member in $workspaceMembers) {
        if (-not $membersBlock.Value.Contains('"' + $member + '"', [StringComparison]::Ordinal)) {
            throw "missing workspace member: $member"
        }
    }
    foreach ($path in $crateManifests) {
        $package = [regex]::Match([IO.File]::ReadAllText($path), '(?ms)^\[package\]\r?\n.*?(?=^\[|\z)')
        $version = [regex]::Match($package.Value, '(?m)^version = "([^"\r\n]*)"')
        if (-not $version.Success) { throw "workspace crate version missing: $path" }
        if ($version.Groups[1].Value -cne '0.2.0') {
            throw "workspace crate version mismatch: $path expected=0.2.0 actual=$($version.Groups[1].Value)"
        }
    }
    Assert-NoRgMatch 'stock_analysis' $crateManifests 'downstream stock_analysis dependency is not allowed'
    $router = [IO.File]::ReadAllText((Join-Path $root 'crates/magic-market-router/Cargo.toml'))
    # sed's canonical section range includes its next section-header line.
    $dependencies = [regex]::Match($router, '(?ms)^\[dependencies\]\r?\n.*?(?:^\[[^\r\n]*(?:\r?\n|\z)|\z)').Value
    if ($dependencies -cmatch 'magic-(tdx|tencent|sina|emquant|eastmoney|cninfo|ths|cls|jin10|thepaper|yonhap|wallstreetcn|baidu|iwencai|exchange|nbs|pbc|cfets|fred|imf|worldbank|sec|xinhua|yicai|stcn)-rs') {
        throw 'router production dependencies must remain provider-neutral'
    }
    $rules = [IO.File]::ReadAllText((Join-Path $root 'docs/business_rules.md'))
    foreach ($number in 1..44) {
        $ruleId = 'BR-{0:000}' -f $number
        if (-not [regex]::IsMatch($rules, '(?m)^## ' + $ruleId + ' ')) { throw "missing registered business rule: $ruleId" }
    }
    $engineering = [IO.File]::ReadAllText((Join-Path $root 'docs/ENGINEERING_RULES.md'))
    if (-not [regex]::IsMatch($engineering, '(?m)^## Gate D ')) { throw 'missing Gate D release rule' }
    Write-Output 'compliance: passed'
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    $result = 1
    if ($_.Exception.Data.Contains('NativeExitCode')) { $result = [int] $_.Exception.Data['NativeExitCode'] }
} finally {
    Pop-Location
    [Environment]::SetEnvironmentVariable('PYTHONUTF8', $savedPythonUtf8)
    [Environment]::SetEnvironmentVariable('PYTHONIOENCODING', $savedPythonEncoding)
}
exit $result
