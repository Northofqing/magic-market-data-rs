param(
    [string]$ValueName = "MagicMarketData-GrpcRuntime"
)

$ErrorActionPreference = "Stop"
$runPath = "Software\Microsoft\Windows\CurrentVersion\Run"

try {
    $runKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($runPath, $true)
    if ($null -ne $runKey) {
        $runKey.DeleteValue($ValueName, $false)
    }
} finally {
    if ($null -ne $runKey) {
        $runKey.Dispose()
    }
}

[Console]::WriteLine("autostart=removed value=$ValueName")
