[CmdletBinding()]
param(
    [string]$PythonExecutable = 'python.exe',
    [string]$FixtureTools = '',
    [switch]$ToolchainInfo
)

$ErrorActionPreference = 'Stop'
$previousEncoding = [Console]::OutputEncoding
$nativeExit = 2
try {
    [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
    $python = (Get-Command $PythonExecutable -CommandType Application -ErrorAction Stop |
        Select-Object -First 1).Source
    if ([IO.Path]::GetExtension($python) -ne '.exe') {
        throw 'benchmark Python must be an existing native Windows executable'
    }
    $arguments = @('-I', '-B', '-X', 'utf8', (Join-Path $PSScriptRoot 'release_profile_native.py'))
    if ($FixtureTools) { $arguments += @('--fixture-tools', $FixtureTools) }
    if ($ToolchainInfo) { $arguments += '--toolchain-info' }
    & $python @arguments
    $nativeExit = $LASTEXITCODE
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
} finally {
    [Console]::OutputEncoding = $previousEncoding
}
exit $nativeExit
