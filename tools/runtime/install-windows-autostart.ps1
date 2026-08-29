param(
    [string]$ValueName = "MagicMarketData-GrpcRuntime",
    [ValidateRange(0, 3600)]
    [int]$DelaySeconds = 20
)

$ErrorActionPreference = "Stop"

if ($ValueName.Trim().Length -eq 0) {
    throw "ValueName must not be empty"
}

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
$action = '"' + $powershell + '" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $entryScript + '" -DelaySeconds ' + $DelaySeconds
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
