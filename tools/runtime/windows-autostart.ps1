param(
    [string]$RuntimeRoot = "",
    [ValidateRange(0, 3600)]
    [int]$DelaySeconds = 0
)

$ErrorActionPreference = "Stop"

# Some managed Windows environments replace PSModulePath and prevent built-in
# cmdlets used by the runtime scripts from autoloading. Load only the trusted
# in-box modules from this PowerShell installation before invoking them.
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

if ($DelaySeconds -gt 0) {
    [Threading.Thread]::Sleep($DelaySeconds * 1000)
}

$repoRoot = [IO.Path]::GetFullPath([IO.Path]::Combine($PSScriptRoot, "..", ".."))
if ([string]::IsNullOrWhiteSpace($RuntimeRoot)) {
    $RuntimeRoot = [IO.Path]::Combine($repoRoot, "target", "runtime")
} else {
    $RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)
}

$logRoot = [IO.Path]::Combine($RuntimeRoot, "logs")
[IO.Directory]::CreateDirectory($logRoot) | Out-Null
$logPath = [IO.Path]::Combine($logRoot, "windows-autostart.log")

function Write-AutostartLog([string]$Level, [string]$Message) {
    $timestamp = [DateTimeOffset]::UtcNow.ToString("O")
    [IO.File]::AppendAllText(
        $logPath,
        "ts=$timestamp level=$Level target=windows_autostart message=$Message$([Environment]::NewLine)",
        [Text.UTF8Encoding]::new($false)
    )
}

function Test-RecordedProcess(
    [string]$PidPath,
    [string]$ExpectedName,
    [string]$ExpectedExecutable
) {
    if (-not [IO.File]::Exists($PidPath)) {
        return $false
    }
    $parsedPid = 0
    if (-not [int]::TryParse([IO.File]::ReadAllText($PidPath).Trim(), [ref]$parsedPid)) {
        return $false
    }
    try {
        $process = [Diagnostics.Process]::GetProcessById($parsedPid)
        $actualExecutable = [IO.Path]::GetFullPath($process.MainModule.FileName)
        $expectedPath = [IO.Path]::GetFullPath($ExpectedExecutable)
        return $process.ProcessName -eq $ExpectedName -and
            $actualExecutable.Equals($expectedPath, [StringComparison]::OrdinalIgnoreCase)
    } catch {
        return $false
    }
}

function Start-TdxTerminalWatchdog {
    $watchdogScript = [IO.Path]::Combine($PSScriptRoot, "tdx-terminal-watchdog.ps1")
    $watchdogConfig = [IO.Path]::Combine($RuntimeRoot, "tdx-terminal-watchdog.json")
    if (-not [IO.File]::Exists($watchdogScript) -or -not [IO.File]::Exists($watchdogConfig)) {
        Write-AutostartLog "INFO" "TDX terminal watchdog is not configured"
        return
    }
    $watchdogPidPath = [IO.Path]::Combine($RuntimeRoot, "tdx-terminal-watchdog.pid")
    $powershell = [IO.Path]::Combine(
        [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows),
        "System32",
        "WindowsPowerShell",
        "v1.0",
        "powershell.exe"
    )
    if (Test-RecordedProcess $watchdogPidPath "powershell" $powershell) {
        Write-AutostartLog "INFO" "TDX terminal watchdog already running"
        return
    }
    if ([IO.File]::Exists($watchdogPidPath)) {
        [IO.File]::Delete($watchdogPidPath)
    }
    $watchdogArguments = @(
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy", "Bypass",
        "-WindowStyle", "Hidden",
        "-File", ('"' + $watchdogScript + '"'),
        "-RuntimeRoot", ('"' + $RuntimeRoot + '"')
    )
    Start-Process `
        -FilePath $powershell `
        -ArgumentList $watchdogArguments `
        -WorkingDirectory $PSScriptRoot `
        -WindowStyle Hidden | Out-Null
    [Threading.Thread]::Sleep(500)
    if (-not [IO.File]::Exists($watchdogPidPath)) {
        throw "TDX terminal watchdog did not remain running after startup"
    }
    Write-AutostartLog "INFO" "TDX terminal watchdog started"
}

$mutex = [Threading.Mutex]::new($false, "Local\MagicMarketDataGrpcRuntimeAutostart")
$acquired = $false
try {
    $acquired = $mutex.WaitOne(0)
    if (-not $acquired) {
        Write-AutostartLog "INFO" "another autostart invocation is active"
        exit 0
    }

    Start-TdxTerminalWatchdog

    $serverPidPath = [IO.Path]::Combine($RuntimeRoot, "grpc-server.pid")
    $agentPidPath = [IO.Path]::Combine($RuntimeRoot, "tdx-agent.pid")
    $serverExecutable = [IO.Path]::Combine($RuntimeRoot, "bin", "magic-market-grpc-server.exe")
    $agentExecutable = [IO.Path]::Combine($RuntimeRoot, "bin", "magic-market-tdx-agent.exe")
    $serverRunning = Test-RecordedProcess $serverPidPath "magic-market-grpc-server" $serverExecutable
    $agentRunning = Test-RecordedProcess $agentPidPath "magic-market-tdx-agent" $agentExecutable

    if ($serverRunning -and $agentRunning) {
        Write-AutostartLog "INFO" "runtime already running"
        exit 0
    }

    $startScript = [IO.Path]::Combine($RuntimeRoot, "start.ps1")
    $stopScript = [IO.Path]::Combine($RuntimeRoot, "stop.ps1")
    if (-not [IO.File]::Exists($startScript) -or -not [IO.File]::Exists($stopScript)) {
        throw "runtime start/stop scripts are missing"
    }

    if ($serverRunning -or $agentRunning -or [IO.File]::Exists($serverPidPath) -or [IO.File]::Exists($agentPidPath)) {
        Write-AutostartLog "WARN" "partial or stale runtime state detected; resetting"
        $stopOutput = @(& $stopScript 2>&1)
        foreach ($line in $stopOutput) {
            Write-AutostartLog "INFO" ([string]$line)
        }
    }

    $startOutput = @(& $startScript 2>&1)
    foreach ($line in $startOutput) {
        Write-AutostartLog "INFO" ([string]$line)
    }

    if (-not (Test-RecordedProcess $serverPidPath "magic-market-grpc-server" $serverExecutable)) {
        throw "gRPC server did not remain running after startup"
    }
    if (-not (Test-RecordedProcess $agentPidPath "magic-market-tdx-agent" $agentExecutable)) {
        throw "TDX agent did not remain running after startup"
    }
    Write-AutostartLog "INFO" "runtime started successfully"
} catch {
    Write-AutostartLog "ERROR" $_.Exception.Message
    exit 1
} finally {
    if ($acquired) {
        $mutex.ReleaseMutex()
    }
    $mutex.Dispose()
}
