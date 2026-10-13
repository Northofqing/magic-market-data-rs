<#
    Restart the runtime only when it is wholly absent, and only from the exact
    admitted binaries.

    Adapted from the recovered 2026-10-03 keepalive (see RECOVERY.md). The differences
    are limited to identity: the runtime root is derived from this script's own
    location instead of a hardcoded `<repo>\target\runtime`, and the three pinned
    hashes below are the currently admitted candidate builds rather than the
    superseded 2026-09 binary pair. Every guard is otherwise unchanged: a partial or
    foreign runtime is reported rather than repaired, the listener must be owned by
    the recorded server pid, and an existing credential is never silently replaced.
#>
param([switch]$CheckOnly)
$ErrorActionPreference = 'Stop'

$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$serverImage = Join-Path $runtimeRoot 'bin\magic-market-grpc-server.exe'
$agentImage = Join-Path $runtimeRoot 'bin\magic-market-tdx-agent.exe'
# The admitted candidate build: source revision 841e4ae7a9df62be0c4536fa9009c66089d282bf.
$expectedServerHash = '9658a0e19e84ccd2fca657c3dcb5410b9a92a319c4deeabdd1ea4930d2c3acb5'
$expectedAgentHash = '3556e7c672b541bc58093ab2705301fe36c3865dda32f5f16125daa1fafc1004'
# The launcher itself is pinned so a rewritten start.ps1 cannot be smuggled in behind
# an unchanged binary pair.
$expectedStartHash = '323107691287245b1a856e761e6ab36738bc54ea082b285df693ae174342a20f'
$logPath = Join-Path $runtimeRoot 'logs\grpc-keepalive.log'

function Write-Receipt([string]$Message) {
    $line = [DateTimeOffset]::UtcNow.ToString('O') + ' ' + $Message
    if (!$CheckOnly) {
        $logRoot = Split-Path -Parent $logPath
        if (!(Test-Path -LiteralPath $logRoot)) { [IO.Directory]::CreateDirectory($logRoot) | Out-Null }
        [IO.File]::AppendAllText($logPath, $line + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    }
    Write-Output $line
}
function Assert-RecordedOwner($Process, [string]$ExpectedPath, [string]$PidFile) {
    if (!$Process -or !$Process.Path -or ![string]::Equals($Process.Path, $ExpectedPath, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected executable owner; no stop or restart performed' }
    $recorded = 0
    if (!(Test-Path -LiteralPath $PidFile -PathType Leaf) -or ![int]::TryParse([IO.File]::ReadAllText($PidFile).Trim(), [ref]$recorded) -or $recorded -ne $Process.Id) { throw 'PID record mismatch; no stop or restart performed' }
    $ownerRows = @(& "$env:SystemRoot\System32\tasklist.exe" /FI "PID eq $($Process.Id)" /FO CSV /NH /V)
    if ($LASTEXITCODE -ne 0) { throw 'Native process owner inventory failed' }
    $owner = @($ownerRows | ConvertFrom-Csv -Header ImageName,PID,SessionName,SessionNumber,MemUsage,Status,UserName,CPUTime,WindowTitle)
    if ($owner.Count -ne 1 -or $owner[0].PID -ne $Process.Id -or $owner[0].UserName -ine [Security.Principal.WindowsIdentity]::GetCurrent().Name) { throw 'Process principal mismatch; no stop or restart performed' }
}
function Get-RuntimeListeners {
    $rows = @(& "$env:SystemRoot\System32\netstat.exe" -ano -p tcp)
    if ($LASTEXITCODE -ne 0) { throw 'Native listener inventory failed' }
    foreach ($row in $rows) {
        $parts = @($row.Trim() -split '\s+')
        if ($parts.Count -eq 5 -and $parts[0] -eq 'TCP' -and $parts[3] -eq 'LISTENING' -and $parts[1] -match '^(.*):50051$') {
            [pscustomobject]@{LocalAddress=$Matches[1]; OwningProcess=[int]$parts[4]}
        }
    }
}

$mutex = [Threading.Mutex]::new($false, 'Local\MagicMarketDataGrpcRuntimeAutostart')
$acquired = $false
try {
    if ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne 'S-1-5-21-1559849321-108443701-1031012297-1001') { throw 'Interactive runtime principal differs' }
    $acquired = $mutex.WaitOne(0)
    if (!$acquired) { Write-Receipt 'another runtime invocation is active'; exit 0 }
    if (!(Test-Path -LiteralPath $serverImage -PathType Leaf)) { throw 'Server binary is absent; coordinated deployment required' }
    if (!(Test-Path -LiteralPath $agentImage -PathType Leaf)) { throw 'Agent binary is absent; coordinated deployment required' }
    if ((Get-FileHash -LiteralPath $serverImage -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedServerHash) { throw 'Server binary differs from the admitted candidate identity; coordinated deployment required' }
    if ((Get-FileHash -LiteralPath $agentImage -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedAgentHash) { throw 'Agent binary differs from the admitted candidate identity' }
    if ((Get-FileHash -LiteralPath (Join-Path $runtimeRoot 'start.ps1') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedStartHash) { throw 'Launcher differs from the pinned identity' }
    if (!(Test-Path -LiteralPath (Join-Path $runtimeRoot '.grpc-token') -PathType Leaf)) { throw 'Existing credential missing; do not generate a replacement' }

    $servers = @(Get-Process -Name 'magic-market-grpc-server' -ErrorAction SilentlyContinue)
    $agents = @(Get-Process -Name 'magic-market-tdx-agent' -ErrorAction SilentlyContinue)
    $processes = @($servers) + @($agents)
    $listeners = @(Get-RuntimeListeners)
    if ($servers.Count -eq 1 -and $agents.Count -eq 1 -and $listeners.Count -eq 1) {
        Assert-RecordedOwner $servers[0] $serverImage (Join-Path $runtimeRoot 'grpc-server.pid')
        Assert-RecordedOwner $agents[0] $agentImage (Join-Path $runtimeRoot 'tdx-agent.pid')
        if ($listeners[0].OwningProcess -ne $servers[0].Id -or $listeners[0].LocalAddress -ne '10.211.55.3') { throw 'Listener owner/address differs' }
        Write-Receipt ('running server_pid=' + $servers[0].Id + ' agent_pid=' + $agents[0].Id)
        exit 0
    }
    if ($processes.Count -ne 0 -or $listeners.Count -ne 0) { throw 'Partial or foreign runtime present; no process stopped and no new instance started' }
    if ($CheckOnly) { Write-Receipt 'runtime stopped'; exit 3 }
    & (Join-Path $runtimeRoot 'start.ps1')
    $serverId = [int][IO.File]::ReadAllText((Join-Path $runtimeRoot 'grpc-server.pid')).Trim()
    $agentId = [int][IO.File]::ReadAllText((Join-Path $runtimeRoot 'tdx-agent.pid')).Trim()
    $server = Get-Process -Id $serverId
    $agent = Get-Process -Id $agentId
    Assert-RecordedOwner $server $serverImage (Join-Path $runtimeRoot 'grpc-server.pid')
    Assert-RecordedOwner $agent $agentImage (Join-Path $runtimeRoot 'tdx-agent.pid')
    $listener = @(Get-RuntimeListeners)
    if ($listener.Count -ne 1 -or $listener[0].OwningProcess -ne $serverId -or $listener[0].LocalAddress -ne '10.211.55.3') { throw 'Started runtime did not acquire the expected listener' }
    Write-Receipt ('restored server_pid=' + $serverId + ' agent_pid=' + $agentId)
} catch {
    Write-Receipt ('ERROR ' + $_.Exception.Message)
    exit 1
} finally {
    if ($acquired) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
