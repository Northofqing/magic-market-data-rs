param(
    # `.env.local` carries the provider API keys and lives at the repository root. When
    # this runtime root was `<repo>\target\runtime`, the repository was found by walking
    # up two levels from the script. The runtime root now lives outside the repository
    # (see .agent-state\target-runtime-recovery-20261013\RECOVERY.md: the isolated
    # qualified build refuses to run while `<repo>\target\runtime` exists, so the two
    # must not share a location), which makes that walk wrong. The root is therefore
    # explicit, with the checked-in repository as its default.
    [string] $RepoRoot = "C:\DevelopFile\magic-market-data-rs"
)

$ErrorActionPreference = "Stop"

$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$localEnvPath = Join-Path $RepoRoot ".env.local"
if (Test-Path -LiteralPath $localEnvPath -PathType Leaf) {
    foreach ($line in [IO.File]::ReadAllLines($localEnvPath)) {
        $trimmed = $line.Trim()
        if ($trimmed.Length -eq 0 -or $trimmed.StartsWith("#")) {
            continue
        }
        $separator = $trimmed.IndexOf("=")
        if ($separator -le 0) {
            throw "invalid .env.local entry"
        }
        $name = $trimmed.Substring(0, $separator).Trim()
        $value = $trimmed.Substring($separator + 1).Trim()
        if ($name -notmatch '^[A-Z][A-Z0-9_]*$' -or $value.Length -eq 0) {
            throw "invalid .env.local key/value"
        }
        [Environment]::SetEnvironmentVariable($name, $value, "Process")
    }
}
$binRoot = Join-Path $runtimeRoot "bin"
$logRoot = Join-Path $runtimeRoot "logs"
$tokenPath = Join-Path $runtimeRoot ".grpc-token"
$tlsRoot = Join-Path $runtimeRoot "tls"
$clientRoot = Join-Path $runtimeRoot "client-bundle"
$serverIp = "10.211.55.3"
$serverDns = "magic-market.local"
$serverPidPath = Join-Path $runtimeRoot "grpc-server.pid"
$agentPidPath = Join-Path $runtimeRoot "tdx-agent.pid"

$recordedProcesses = @(
    @{ Path = $serverPidPath; Name = "magic-market-grpc-server" },
    @{ Path = $agentPidPath; Name = "magic-market-tdx-agent" }
)
foreach ($recorded in $recordedProcesses) {
    $pidPath = $recorded.Path
    if (Test-Path -LiteralPath $pidPath) {
        $existingPid = [int](Get-Content -LiteralPath $pidPath -Raw)
        $existingProcess = Get-Process -Id $existingPid -ErrorAction SilentlyContinue
        if ($null -ne $existingProcess -and $existingProcess.ProcessName -eq $recorded.Name) {
            throw "runtime process $existingPid is already running"
        }
        Remove-Item -LiteralPath $pidPath
    }
}

$requiredTlsFiles = @(
    "ca.pem",
    "server.pem",
    "server-key.pem",
    "tdx-agent.pem",
    "tdx-agent-key.pem"
)
foreach ($name in $requiredTlsFiles) {
    $path = Join-Path $tlsRoot $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing TLS file $path; run configure-remote.ps1 first"
    }
}

if (Test-Path -LiteralPath $tokenPath -PathType Leaf) {
    $token = [IO.File]::ReadAllText($tokenPath).Trim()
} else {
    $token = [Guid]::NewGuid().ToString("N") + [Guid]::NewGuid().ToString("N")
    [IO.File]::WriteAllText($tokenPath, $token, [Text.UTF8Encoding]::new($false))
    [IO.File]::SetAttributes($tokenPath, [IO.FileAttributes]::Hidden)
}
if ($token.Length -lt 32) {
    throw "Bearer token is unexpectedly short"
}
[IO.File]::WriteAllText((Join-Path $clientRoot "bearer-token.txt"), $token, [Text.UTF8Encoding]::new($false))
[Environment]::SetEnvironmentVariable("MAGIC_MARKET_GRPC_TOKEN", $token, "Process")

$serverArgs = @(
    "--bind", "$serverIp`:50051",
    "--auth-token-env", "MAGIC_MARKET_GRPC_TOKEN",
    "--max-decoding-bytes", "1048576",
    "--max-encoding-bytes", "1048576",
    "--max-payload-bytes", "524288",
    "--unary-concurrency", "16",
    "--blocking-concurrency", "8",
    "--provider-timeout-ms", "60000",
    "--blocking-deadline-ms", "180000",
    "--max-subscribers", "16",
    "--subscriber-queue-capacity", "128",
    "--replay-max-events", "1000",
    "--replay-max-bytes", "16777216",
    "--agent-command-capacity", "32",
    "--agent-heartbeat-timeout-ms", "15000",
    "--shutdown-timeout-ms", "5000",
    "--reflection", "true",
    "--tls-cert", (Join-Path $tlsRoot "server.pem"),
    "--tls-key", (Join-Path $tlsRoot "server-key.pem"),
    "--tls-client-ca", (Join-Path $tlsRoot "ca.pem")
)

# Start-Process truncates the redirect targets at every start: no rotation, no backup. On
# 2026-09-23 that destroyed the previous run's log, and with it the only record of that
# morning's failure window. Archive first so a restart never costs provenance. Never fatal
# on its own -- a failed archive must not stop the server from starting.
#
# This must keep running only once the previous processes are dead: while a writer still
# holds a redirected log, a directory listing can report Length 0 for a file that already
# has content (measured 2026-09-23: tdx-agent.stderr.log listed as 0 bytes, 341 on archive),
# and the empty-file skip below would then drop it.
$archiveRoot = Join-Path $logRoot "archive"
try {
    $archived = @()
    $logNames = @(
        "grpc-server.stdout.log",
        "grpc-server.stderr.log",
        "tdx-agent.stdout.log",
        "tdx-agent.stderr.log"
    )
    foreach ($logName in $logNames) {
        $logPath = Join-Path $logRoot $logName
        if (-not (Test-Path -LiteralPath $logPath -PathType Leaf)) {
            continue
        }
        $logItem = Get-Item -LiteralPath $logPath
        if ($logItem.Length -eq 0) {
            continue
        }
        if (-not (Test-Path -LiteralPath $archiveRoot)) {
            New-Item -ItemType Directory -Path $archiveRoot | Out-Null
        }
        $archivePath = Join-Path $archiveRoot "$logName.$($logItem.LastWriteTime.ToString('yyyyMMdd-HHmmss'))"
        $suffix = 0
        while (Test-Path -LiteralPath $archivePath) {
            $suffix++
            $archivePath = Join-Path $archiveRoot "$logName.$($logItem.LastWriteTime.ToString('yyyyMMdd-HHmmss'))-$suffix"
        }
        Move-Item -LiteralPath $logPath -Destination $archivePath
        $archived += "$logName -> $(Split-Path -Leaf $archivePath)"
    }
    if ($archived.Count -gt 0) {
        Write-Output "archived previous logs:"
        foreach ($entry in $archived) {
            Write-Output "  $entry"
        }
    }
} catch {
    Write-Warning "could not archive the previous logs: $($_.Exception.Message)"
}

$serverStart = @{
    FilePath = Join-Path $binRoot "magic-market-grpc-server.exe"
    ArgumentList = $serverArgs
    WorkingDirectory = $binRoot
    WindowStyle = "Hidden"
    RedirectStandardOutput = Join-Path $logRoot "grpc-server.stdout.log"
    RedirectStandardError = Join-Path $logRoot "grpc-server.stderr.log"
    PassThru = $true
}
$server = Start-Process @serverStart

Start-Sleep -Seconds 2
if ($server.HasExited) {
    throw "gRPC server exited during startup with code $($server.ExitCode)"
}

$agentArgs = @(
    "--server-uri", "https://$serverIp`:50051",
    "--auth-token-env", "MAGIC_MARKET_GRPC_TOKEN",
    "--max-frame-bytes", "8192",
    "--queue-capacity", "128",
    "--connect-timeout-ms", "5000",
    "--command-timeout-ms", "10000",
    "--heartbeat-interval-ms", "5000",
    "--reconnect-delay-ms", "1000",
    "--shutdown-timeout-ms", "5000",
    "--tls-domain", $serverDns,
    "--tls-ca", (Join-Path $tlsRoot "ca.pem"),
    "--tls-cert", (Join-Path $tlsRoot "tdx-agent.pem"),
    "--tls-key", (Join-Path $tlsRoot "tdx-agent-key.pem")
)

$agentStart = @{
    FilePath = Join-Path $binRoot "magic-market-tdx-agent.exe"
    ArgumentList = $agentArgs
    WorkingDirectory = $binRoot
    WindowStyle = "Hidden"
    RedirectStandardOutput = Join-Path $logRoot "tdx-agent.stdout.log"
    RedirectStandardError = Join-Path $logRoot "tdx-agent.stderr.log"
    PassThru = $true
}
$agent = Start-Process @agentStart

Start-Sleep -Seconds 3
if ($agent.HasExited) {
    throw "TDX agent exited during startup with code $($agent.ExitCode)"
}

[IO.File]::WriteAllText($serverPidPath, $server.Id.ToString(), [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($agentPidPath, $agent.Id.ToString(), [Text.UTF8Encoding]::new($false))

[PSCustomObject]@{
    GrpcServerPid = $server.Id
    TdxAgentPid = $agent.Id
    Endpoint = "https://$serverIp`:50051"
    TlsServerName = $serverDns
    ClientBundle = $clientRoot
    TokenFile = $tokenPath
}
