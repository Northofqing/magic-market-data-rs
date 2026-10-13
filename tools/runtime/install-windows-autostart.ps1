param(
    [string]$ValueName = "MagicMarketData-GrpcRuntime",
    [Parameter(Mandatory = $true)]
    [string]$RuntimeRoot,
    [ValidateRange(0, 3600)]
    [int]$DelaySeconds = 20
)

$ErrorActionPreference = "Stop"

if ($ValueName.Trim().Length -eq 0) {
    throw "ValueName must not be empty"
}

# Required rather than defaulted. Left to itself, windows-autostart.ps1 falls back
# to `<repo>\target\runtime`, which the isolated qualified build deletes on sight
# (run_build.py, configs_absent(): "unreviewed Cargo config or existing production
# runtime"). An autostart entry registered against that path is a logon job that
# cannot work and disappears with the directory -- exactly the failure this
# arrangement exists to prevent. Naming the root here makes it a decision instead
# of an accident.
if ([string]::IsNullOrWhiteSpace($RuntimeRoot)) {
    throw "RuntimeRoot must not be empty"
}
$RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)

$entryScript = [IO.Path]::GetFullPath([IO.Path]::Combine($PSScriptRoot, "windows-autostart.ps1"))
if (-not [IO.File]::Exists($entryScript)) {
    throw "autostart entry script is missing: $entryScript"
}

$powershell = [IO.Path]::Combine(
    [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows),
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe"
)
$currentUser = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$action = '"' + $powershell + '" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $entryScript + '" -RuntimeRoot "' + $RuntimeRoot + '" -DelaySeconds ' + $DelaySeconds
$runPath = "Software\Microsoft\Windows\CurrentVersion\Run"

try {
    $runKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($runPath, $true)
    if ($null -eq $runKey) {
        $runKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($runPath, $true)
    }
    if ($null -eq $runKey) {
        throw "unable to open the current-user Run key"
    }
    $runKey.SetValue($ValueName, $action, [Microsoft.Win32.RegistryValueKind]::String)
    $stored = [string]$runKey.GetValue($ValueName, "")
} finally {
    if ($null -ne $runKey) {
        $runKey.Dispose()
    }
}

if ($stored -ne $action) {
    throw "the current-user Run entry did not round-trip exactly"
}

[Console]::WriteLine("autostart=installed user=$currentUser value=$ValueName delay_seconds=$DelaySeconds")
[Console]::WriteLine("entry_script=$entryScript")
[Console]::WriteLine("runtime_root=$RuntimeRoot")
