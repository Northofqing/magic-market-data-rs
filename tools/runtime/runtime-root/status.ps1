<#
    Read-only runtime health check.

    The contract it reports against is the one the checked-in
    `tools/runtime/windows-autostart.ps1` already enforces: `grpc-server.pid` and
    `tdx-agent.pid` each record a process that must still be alive under the exact
    name and the exact executable path below this runtime root. This script never
    starts, stops or repairs anything.
#>
param(
    [string] $ServerIp = "10.211.55.3",
    [int] $Port = 50051
)

$ErrorActionPreference = "Stop"

$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$binRoot = Join-Path $runtimeRoot "bin"
$serverImage = Join-Path $binRoot "magic-market-grpc-server.exe"
$agentImage = Join-Path $binRoot "magic-market-tdx-agent.exe"

function Test-RecordedProcess([string] $PidPath, [string] $ExpectedName, [string] $ExpectedExecutable) {
    $result = [ordered]@{
        Role = $ExpectedName
        PidFile = $PidPath
        RecordedPid = $null
        Running = $false
        Detail = ""
    }
    if (-not (Test-Path -LiteralPath $PidPath -PathType Leaf)) {
        $result.Detail = "pid file absent"
        return [PSCustomObject]$result
    }
    $parsed = 0
    if (-not [int]::TryParse(([IO.File]::ReadAllText($PidPath)).Trim(), [ref]$parsed)) {
        $result.Detail = "pid file is not an integer"
        return [PSCustomObject]$result
    }
    $result.RecordedPid = $parsed
    try {
        $process = [Diagnostics.Process]::GetProcessById($parsed)
        $actual = [IO.Path]::GetFullPath($process.MainModule.FileName)
        $expected = [IO.Path]::GetFullPath($ExpectedExecutable)
        if ($process.ProcessName -ne $ExpectedName) {
            $result.Detail = "pid $parsed is '$($process.ProcessName)', expected '$ExpectedName'"
        } elseif (-not $actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
            $result.Detail = "pid $parsed runs '$actual', expected '$expected'"
        } else {
            $result.Running = $true
            $result.Detail = "ok"
        }
    } catch {
        $result.Detail = "pid $parsed is not running"
    }
    return [PSCustomObject]$result
}

$server = Test-RecordedProcess (Join-Path $runtimeRoot "grpc-server.pid") "magic-market-grpc-server" $serverImage
$agent = Test-RecordedProcess (Join-Path $runtimeRoot "tdx-agent.pid") "magic-market-tdx-agent" $agentImage

# Read the listener natively rather than through a cmdlet: the owning pid is what proves
# the port belongs to the recorded server, not merely that something is bound.
$listener = $null
$netstat = @(& "$env:SystemRoot\System32\netstat.exe" -ano -p tcp)
foreach ($row in $netstat) {
    $parts = @($row.Trim() -split "\s+")
    if ($parts.Count -eq 5 -and $parts[0] -eq "TCP" -and $parts[3] -eq "LISTENING" -and $parts[1] -match "^(.*):$Port$") {
        $listener = [PSCustomObject]@{ LocalAddress = $Matches[1]; OwningProcess = [int]$parts[4] }
        break
    }
}

$listenerOk = $null -ne $listener -and $null -ne $server.RecordedPid -and
    $listener.OwningProcess -eq $server.RecordedPid -and $listener.LocalAddress -eq $ServerIp

$tokenPath = Join-Path $runtimeRoot ".grpc-token"
$tlsRoot = Join-Path $runtimeRoot "tls"
$missingTls = @()
foreach ($name in @("ca.pem", "server.pem", "server-key.pem", "tdx-agent.pem", "tdx-agent-key.pem")) {
    if (-not (Test-Path -LiteralPath (Join-Path $tlsRoot $name) -PathType Leaf)) {
        $missingTls += $name
    }
}

$healthy = $server.Running -and $agent.Running -and $listenerOk -and $missingTls.Count -eq 0

[PSCustomObject]@{
    RuntimeRoot = $runtimeRoot
    Healthy = $healthy
    GrpcServerPid = $server.RecordedPid
    GrpcServerRunning = $server.Running
    GrpcServerDetail = $server.Detail
    TdxAgentPid = $agent.RecordedPid
    TdxAgentRunning = $agent.Running
    TdxAgentDetail = $agent.Detail
    ListenerAddress = if ($null -ne $listener) { $listener.LocalAddress } else { $null }
    ListenerPid = if ($null -ne $listener) { $listener.OwningProcess } else { $null }
    ListenerOwnedByServer = $listenerOk
    Endpoint = "https://$ServerIp`:$Port"
    TokenPresent = (Test-Path -LiteralPath $tokenPath -PathType Leaf)
    MissingTlsFiles = ($missingTls -join ", ")
} | Format-List

if (-not $healthy) {
    exit 3
}
