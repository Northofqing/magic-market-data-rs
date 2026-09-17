param(
    [string]$Destination = "target/runtime/client-bundle",
    [string]$BundleVersion = "2026-09-17.1",
    [string]$SourceCommit = "",
    [string]$ServiceVersion = "",
    [string]$ContractSha256 = "",
    [string]$BinarySha256 = ""
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
$identityValues = @($ServiceVersion, $ContractSha256, $BinarySha256)
$identityValueCount = @(
    $identityValues | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
).Count
if ($identityValueCount -ne 0 -and $identityValueCount -ne $identityValues.Count) {
    throw "ServiceVersion, ContractSha256 and BinarySha256 must be supplied together"
}
$hasDeploymentIdentity = $identityValueCount -eq $identityValues.Count
if ($hasDeploymentIdentity) {
    if ($SourceCommit.Length -ne 40) {
        throw "deployment identity requires a full 40-character SourceCommit"
    }
    if ($ServiceVersion -notmatch '^[0-9A-Za-z][0-9A-Za-z.+-]{0,63}$') {
        throw "ServiceVersion is invalid"
    }
    if ($ContractSha256 -notmatch '^[0-9a-f]{64}$') {
        throw "ContractSha256 must be a lowercase SHA-256 digest"
    }
    if ($BinarySha256 -notmatch '^[0-9a-f]{64}$') {
        throw "BinarySha256 must be a lowercase SHA-256 digest"
    }
}

$publicFiles = @(
    @("market.proto", "crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto"),
    @("grpc-external-api.md", "docs/integrations/grpc-external-api.md"),
    @("grpc-derived-products.md", "docs/integrations/grpc-derived-products.md"),
    @("tdx-public-security-profile.md", "docs/integrations/tdx-public-security-profile.md"),
    @("unadmitted-provider-routes.md", "docs/integrations/unadmitted-provider-routes.md")
)

$null = [System.IO.Directory]::CreateDirectory($destinationPath)
foreach ($entry in $publicFiles) {
    [System.IO.File]::Copy(
        [System.IO.Path]::Combine($repositoryRoot, $entry[1]),
        [System.IO.Path]::Combine($destinationPath, $entry[0]),
        $true
    )
}

$destinationRoot = [System.IO.Path]::GetFullPath($destinationPath)
$destinationPrefix = $destinationRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
foreach ($entry in $publicFiles) {
    if (-not $entry[0].EndsWith(".md", [System.StringComparison]::OrdinalIgnoreCase)) {
        continue
    }
    $documentPath = [System.IO.Path]::Combine($destinationRoot, $entry[0])
    $document = [System.IO.File]::ReadAllText($documentPath)
    foreach ($match in [regex]::Matches(
        $document,
        '\]\((?<target>[^):#?]+\.md)(?:#[^)]*)?\)'
    )) {
        $target = $match.Groups['target'].Value.Replace(
            '/',
            [System.IO.Path]::DirectorySeparatorChar
        )
        $resolved = [System.IO.Path]::GetFullPath(
            [System.IO.Path]::Combine($destinationRoot, $target)
        )
        if (-not $resolved.StartsWith(
            $destinationPrefix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or -not [System.IO.File]::Exists($resolved)) {
            throw "$($entry[0]) links to missing bundle document $target"
        }
    }
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
if ($rpcCount -ne 63) {
    throw "client bundle must contain exactly 63 MarketDataService RPCs, got $rpcCount"
}

$generatedAt = [DateTimeOffset]::UtcNow.ToString("O")
$deploymentIdentity = "null"
if ($hasDeploymentIdentity) {
    $deploymentIdentity = @"
{
    "service_version": "$ServiceVersion",
    "source_revision": "$SourceCommit",
    "contract_sha256": "$ContractSha256",
    "binary_sha256": "$BinarySha256",
    "contract_sha256_scope": "SHA-256 of the raw compiled FileDescriptorSet bytes returned by magic_market_grpc_contracts::v1::FILE_DESCRIPTOR_SET",
    "binary_sha256_scope": "SHA-256 of the exact deployed magic-market-grpc-server executable bytes"
  }
"@
}
$metadata = @"
{
  "bundle_version": "$BundleVersion",
  "source_commit": "$SourceCommit",
  "market_data_rpc_count": $rpcCount,
  "global_news_schema_version": 2,
  "instrument_news_schema_version": 2,
  "t0_evidence_schema_version": 2,
  "financial_statements_schema_version": 2,
  "realtime_quotes_schema_version": 1,
  "current_auction_observations_schema_version": 1,
  "economic_release_observations_schema_version": 1,
  "economic_release_schedule_schema_version": 1,
  "deployment_build_identity": $deploymentIdentity,
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

Public contract files are covered by manifest.sha256. GlobalNews, InstrumentNews,
T0Evidence, and FinancialStatements support the documented schema version 2;
FinancialStatements also retains its frozen version 1 projection.
GetHealth and GetListenerStatus expose append-only aggregate runtime observability fields.
When deployment_build_identity is non-null, compare every field with GetHealth.build_identity
before admitting the endpoint; the documented hash scopes are exact and case-sensitive.
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
    "tdx-public-security-profile.md",
    "unadmitted-provider-routes.md",
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
