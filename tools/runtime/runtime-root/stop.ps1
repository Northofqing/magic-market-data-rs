$ErrorActionPreference = "Stop"
$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

function Stop-RecordedProcess(
    [string] $pidFile,
    [string] $expectedProcessName
) {
    if (-not (Test-Path -LiteralPath $pidFile)) {
        return
    }
    $recordedPid = [int][IO.File]::ReadAllText($pidFile)
    $process = Get-Process -Id $recordedPid -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return
    }
    if ($process.ProcessName -ne $expectedProcessName) {
        throw "PID $recordedPid no longer belongs to $expectedProcessName"
    }
    Stop-Process -Id $recordedPid
    Wait-Process -Id $recordedPid -Timeout 10 -ErrorAction SilentlyContinue
}

$agentPidFile = Join-Path $runtimeRoot "tdx-agent.pid"
$serverPidFile = Join-Path $runtimeRoot "grpc-server.pid"
$agentPid = if (Test-Path -LiteralPath $agentPidFile) {
    [int][IO.File]::ReadAllText($agentPidFile)
} else {
    0
}
$monitorPids = if ($agentPid -ne 0) {
    @(Get-CimInstance Win32_Process |
        Where-Object {
            $_.ParentProcessId -eq $agentPid -and
            $_.Name -eq "magic-market-monitor-server.exe"
        } |
        ForEach-Object { [int]$_.ProcessId })
} else {
    @()
}

Stop-RecordedProcess $agentPidFile "magic-market-tdx-agent"
foreach ($monitorPid in $monitorPids) {
    if (Get-Process -Id $monitorPid -ErrorAction SilentlyContinue) {
        Stop-Process -Id $monitorPid
    }
}
Stop-RecordedProcess $serverPidFile "magic-market-grpc-server"

Write-Output "magic-market runtime stopped"
