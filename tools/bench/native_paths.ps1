[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Target
)
$ErrorActionPreference = 'Stop'
try {
    if (Test-Path -LiteralPath $Path) { throw 'cache junction destination already exists' }
    if (-not (Test-Path -LiteralPath $Target -PathType Container)) { throw 'cache target missing' }
    New-Item -ItemType Junction -Path $Path -Target $Target -ErrorAction Stop | Out-Null
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 2
}
exit 0
