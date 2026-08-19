param(
    [string]$Destination = "target/runtime/client-bundle",
    [string]$BundleVersion = "2026-08-19.2",
    [string]$SourceCommit = ""
)

$ErrorActionPreference = "Stop"
$repositoryRoot = [System.IO.Path]::GetFullPath(
    [System.IO.Path]::Combine($PSScriptRoot, "../..")
)
$destinationPath = if ([System.IO.Path]::IsPathRooted($Destination)) {
    $Destination
} else {
    [System.IO.Path]::Combine($repositoryRoot, $Destination)
}

if ([string]::IsNullOrWhiteSpace($SourceCommit)) {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = "git"
    $startInfo.Arguments = "-C `"$repositoryRoot`" rev-parse HEAD"
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $process = [System.Diagnostics.Process]::Start($startInfo)
    $SourceCommit = $process.StandardOutput.ReadToEnd().Trim()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "git rev-parse HEAD failed"
    }
}
if ($SourceCommit -notmatch '^[0-9a-f]{7,40}$') {
    throw "SourceCommit must be a Git commit SHA"
}

$publicFiles = @(
    @("market.proto", "crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto"),
    @("grpc-external-api.md", "docs/integrations/grpc-external-api.md"),
    @("grpc-derived-products.md", "docs/integrations/grpc-derived-products.md")
)

$null = [System.IO.Directory]::CreateDirectory($destinationPath)
foreach ($entry in $publicFiles) {
    [System.IO.File]::Copy(
        [System.IO.Path]::Combine($repositoryRoot, $entry[1]),
        [System.IO.Path]::Combine($destinationPath, $entry[0]),
        $true
    )
}

$proto = [System.IO.File]::ReadAllText(
    [System.IO.Path]::Combine($destinationPath, "market.proto")
)
$serviceMatch = [regex]::Match(
    $proto,
    '(?s)service\s+MarketDataService\s*\{(?<body>.*?)\r?\n\}'
)
if (-not $serviceMatch.Success) {
    throw "market.proto has no MarketDataService block"
}
$rpcCount = [regex]::Matches($serviceMatch.Groups['body'].Value, '\brpc\s+').Count
if ($rpcCount -ne 60) {
    throw "client bundle must contain exactly 60 MarketDataService RPCs, got $rpcCount"
}

$generatedAt = [DateTimeOffset]::UtcNow.ToString("O")
$metadata = @"
{
  "bundle_version": "$BundleVersion",
  "source_commit": "$SourceCommit",
  "market_data_rpc_count": $rpcCount,
  "global_news_schema_version": 2,
  "instrument_news_schema_version": 2,
  "generated_at_utc": "$generatedAt"
}
"@
$utf8 = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText(
    [System.IO.Path]::Combine($destinationPath, "bundle-metadata.json"),
    $metadata,
    $utf8
)

$readme = @"
# Magic Market client bundle $BundleVersion

Source commit: $SourceCommit
MarketDataService RPCs: $rpcCount

Public contract files are covered by manifest.sha256. GlobalNews and InstrumentNews
use schema version 2; all other request payload versions remain documented per method.
TLS client identities and Bearer tokens are deployment-private and are not covered by,
or copied by, this public contract builder.

Verify the public contract files from this directory:

    sha256sum -c manifest.sha256

On macOS, where sha256sum is not installed by default:

    shasum -a 256 -c manifest.sha256
"@
[System.IO.File]::WriteAllText(
    [System.IO.Path]::Combine($destinationPath, "README.md"),
    $readme,
    $utf8
)

$manifestFiles = @(
    "market.proto",
    "grpc-external-api.md",
    "grpc-derived-products.md",
    "bundle-metadata.json",
    "README.md"
)
$manifest = foreach ($name in $manifestFiles) {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead([System.IO.Path]::Combine($destinationPath, $name))
    try {
        $hash = ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace(
            "-",
            ""
        ).ToLowerInvariant()
    } finally {
        $stream.Dispose()
        $sha256.Dispose()
    }
    "$hash  $name"
}
$manifestPath = [System.IO.Path]::Combine($destinationPath, "manifest.sha256")
$manifestText = [string]::Join("`n", $manifest) + "`n"
[System.IO.File]::WriteAllText($manifestPath, $manifestText, [System.Text.Encoding]::ASCII)
$manifestBytes = [System.IO.File]::ReadAllBytes($manifestPath)
if ([Array]::IndexOf($manifestBytes, [byte]13) -ge 0) {
    throw "manifest.sha256 must contain LF line endings without carriage returns"
}

[System.Console]::WriteLine("bundle=$destinationPath")
[System.Console]::WriteLine("version=$BundleVersion")
[System.Console]::WriteLine("source_commit=$SourceCommit")
[System.Console]::WriteLine("market_data_rpcs=$rpcCount")
