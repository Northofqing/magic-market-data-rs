#requires -Version 7.3
[CmdletBinding()]
param(
    [string] $RepositoryRoot = (Join-Path $PSScriptRoot '../..'),
    [string] $RgExecutable = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$PSNativeCommandUseErrorActionPreference = $false
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$result = 0
Push-Location -LiteralPath $root
try {
    if ($RgExecutable) { $rg = (Resolve-Path -LiteralPath $RgExecutable).Path }
    else { $rg = (Get-Command rg.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source }
    if ([IO.Path]::GetExtension($rg) -ne '.exe') { throw 'a native ripgrep .exe is required' }
    # Same ripgrep scope, ignore rules and Markdown pattern as check_links.sh.
    $matches = @(& $rg '-n' '--with-filename' '-o' '-g' '*.md' '--' '\[[^]]+\]\([^)]*\)' 'docs' 'crates')
    $code = $LASTEXITCODE
    if ($code -notin @(0, 1)) {
        $exception = [Exception]::new("ripgrep failed (exit=$code)")
        $exception.Data['NativeExitCode'] = $code
        throw $exception
    }
    foreach ($entry in $matches) {
        $match = [regex]::Match($entry, '^(.*?):([0-9]+):\[[^]]+\]\(([^)]*)\)$')
        if (-not $match.Success) { throw 'invalid ripgrep link record' }
        $file = $match.Groups[1].Value.Replace('\', '/')
        $target = $match.Groups[3].Value
        if ($target.StartsWith('http://', [StringComparison]::Ordinal) -or
            $target.StartsWith('https://', [StringComparison]::Ordinal) -or
            $target.StartsWith('mailto:', [StringComparison]::Ordinal)) { continue }
        $target = ($target -split '#', 2)[0]
        if (-not $target) { continue }
        $fileDirectory = [IO.Path]::GetDirectoryName($file)
        if (-not $fileDirectory) { $fileDirectory = '.' }
        $relativeTarget = [IO.Path]::Combine($root, $fileDirectory, $target)
        $rootTarget = [IO.Path]::Combine($root, $target)
        if (-not (Test-Path -LiteralPath $relativeTarget) -and -not (Test-Path -LiteralPath $rootTarget)) {
            [Console]::Error.WriteLine("${file}: missing link target $target")
            $result = 1
        }
    }
    if ($result -eq 0) { Write-Output 'documentation links: passed' }
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    $result = 1
    if ($_.Exception.Data.Contains('NativeExitCode')) { $result = [int] $_.Exception.Data['NativeExitCode'] }
} finally { Pop-Location }
exit $result
