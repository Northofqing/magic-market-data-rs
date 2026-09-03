param(
    [string]$RuntimeRoot = "",
    [ValidateRange(1, 300)]
    [int]$PollSeconds = 10,
    [ValidateRange(50, 5000)]
    [int]$LoopbackTimeoutMillis = 500,
    [switch]$Once
)

$ErrorActionPreference = "Stop"

$managementModule = [IO.Path]::Combine(
    $PSHOME,
    "Modules",
    "Microsoft.PowerShell.Management",
    "Microsoft.PowerShell.Management.psd1"
)
$utilityModule = [IO.Path]::Combine(
    $PSHOME,
    "Modules",
    "Microsoft.PowerShell.Utility",
    "Microsoft.PowerShell.Utility.psd1"
)
Import-Module $managementModule -Force
Import-Module $utilityModule -Force

$repoRoot = [IO.Path]::GetFullPath([IO.Path]::Combine($PSScriptRoot, "..", ".."))
if ([string]::IsNullOrWhiteSpace($RuntimeRoot)) {
    $RuntimeRoot = [IO.Path]::Combine($repoRoot, "target", "runtime")
} else {
    $RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)
}

$approvedSha256 = "58bd2117ec86e8c063639f7adae4218011bb93998e3d93dcd286672d1978736b"
$configPath = [IO.Path]::Combine($RuntimeRoot, "tdx-terminal-watchdog.json")
$pidPath = [IO.Path]::Combine($RuntimeRoot, "tdx-terminal-watchdog.pid")
$logRoot = [IO.Path]::Combine($RuntimeRoot, "logs")
$logPath = [IO.Path]::Combine($logRoot, "tdx-terminal-watchdog.log")
[IO.Directory]::CreateDirectory($logRoot) | Out-Null

function Write-WatchdogLog([string]$Level, [string]$Event, [string]$Fields) {
    $timestamp = [DateTimeOffset]::UtcNow.ToString("O")
    $suffix = if ([string]::IsNullOrWhiteSpace($Fields)) { "" } else { " $Fields" }
    [IO.File]::AppendAllText(
        $logPath,
        "ts=$timestamp level=$Level target=tdx_terminal_watchdog event=$Event$suffix$([Environment]::NewLine)",
        [Text.UTF8Encoding]::new($false)
    )
}

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    } finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Test-LoopbackReady([int]$TimeoutMillis) {
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $connect = $client.ConnectAsync("127.0.0.1", 17709)
        if (-not $connect.Wait($TimeoutMillis)) {
            return $false
        }
        return $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

if (-not [IO.File]::Exists($configPath)) {
    throw "TDX watchdog configuration is missing: $configPath"
}
$config = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($configPath)) | ConvertFrom-Json
$configuredExecutable = [string]$config.executable
if ([string]::IsNullOrWhiteSpace($configuredExecutable)) {
    throw "TDX watchdog executable is required"
}
$tdxExecutable = [IO.Path]::GetFullPath($configuredExecutable)
if (-not [IO.Path]::GetFileName($tdxExecutable).Equals("TdxW.exe", [StringComparison]::OrdinalIgnoreCase)) {
    throw "TDX watchdog accepts only an exact TdxW.exe executable"
}
if (-not [IO.File]::Exists($tdxExecutable)) {
    throw "configured TdxW.exe does not exist"
}
if ((Get-Sha256 $tdxExecutable) -ne $approvedSha256) {
    throw "configured TdxW.exe does not match the admitted compatibility hash"
}

$mutex = [Threading.Mutex]::new($false, "Local\MagicMarketDataTdxTerminalWatchdog")
$acquired = $false
$lastState = ""
$validatedPid = 0
try {
    $acquired = $mutex.WaitOne(0)
    if (-not $acquired) {
        Write-WatchdogLog "INFO" "already_running" ""
        exit 0
    }
    [IO.File]::WriteAllText(
        $pidPath,
        ([Diagnostics.Process]::GetCurrentProcess().Id.ToString()),
        [Text.UTF8Encoding]::new($false)
    )

    do {
        $sessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
        $candidates = @(
            [Diagnostics.Process]::GetProcessesByName("TdxW") |
                Where-Object { $_.SessionId -eq $sessionId }
        )
        $terminalState = "missing"
        $terminalPid = 0
        if ($candidates.Count -eq 0) {
            if ((Get-Sha256 $tdxExecutable) -ne $approvedSha256) {
                throw "TdxW.exe changed after watchdog startup"
            }
            $started = Start-Process `
                -FilePath $tdxExecutable `
                -WorkingDirectory ([IO.Path]::GetDirectoryName($tdxExecutable)) `
                -WindowStyle Normal `
                -PassThru
            $terminalPid = $started.Id
            $validatedPid = $terminalPid
            $terminalState = "started"
            Write-WatchdogLog "INFO" "terminal_started" "pid=$terminalPid"
        } elseif ($candidates.Count -eq 1) {
            $candidate = $candidates[0]
            $terminalPid = $candidate.Id
            try {
                $actualExecutable = [IO.Path]::GetFullPath($candidate.MainModule.FileName)
                if (-not $actualExecutable.Equals($tdxExecutable, [StringComparison]::OrdinalIgnoreCase)) {
                    $terminalState = "identity_mismatch"
                } elseif ($validatedPid -ne $terminalPid) {
                    if ((Get-Sha256 $actualExecutable) -ne $approvedSha256) {
                        $terminalState = "hash_mismatch"
                    } else {
                        $validatedPid = $terminalPid
                        $terminalState = "running"
                    }
                } else {
                    $terminalState = "running"
                }
            } catch {
                $terminalState = "identity_unavailable"
            }
        } else {
            $terminalState = "ambiguous"
        }

        $loopbackReady = $false
        if ($terminalState -eq "running" -or $terminalState -eq "started") {
            $loopbackReady = Test-LoopbackReady $LoopbackTimeoutMillis
        }
        $state = "$terminalState`:loopback_$($loopbackReady.ToString().ToLowerInvariant())"
        if ($state -ne $lastState) {
            $level = if ($terminalState -eq "running" -and $loopbackReady) { "INFO" } else { "WARN" }
            Write-WatchdogLog $level "readiness_changed" "terminal=$terminalState loopback_ready=$($loopbackReady.ToString().ToLowerInvariant()) pid=$terminalPid"
            $lastState = $state
        }
        if (-not $Once) {
            [Threading.Thread]::Sleep($PollSeconds * 1000)
        }
    } while (-not $Once)
} catch {
    Write-WatchdogLog "ERROR" "watchdog_failed" "category=configuration_or_runtime"
    throw
} finally {
    if ([IO.File]::Exists($pidPath)) {
        [IO.File]::Delete($pidPath)
    }
    if ($acquired) {
        $mutex.ReleaseMutex()
    }
    $mutex.Dispose()
}
