param(
    [Parameter(Mandatory = $true)] [string]$Endpoint,
    [Parameter(Mandatory = $true)] [string]$TokenEnvironment,
    [Parameter(Mandatory = $true)] [ValidatePattern('^[0-9a-f]{40}$')] [string]$SourceRevision,
    [Parameter(Mandatory = $true)] [ValidatePattern('^[0-9a-f]{64}$')] [string]$BinarySha256,
    [Parameter(Mandatory = $true)] [ValidatePattern('^[0-9a-f]{64}$')] [string]$ContractSha256,
    [ValidateSet('Flows', 'Historical', 'Announcements', 'News')] [string]$CaseGroup = 'Historical',
    [string]$TlsBundle = ''
)

# Read-only receipt collection. Never starts, stops or changes a server.
# Exit 0 means all cases passed the stated bounded acceptance checks; it does
# not certify trading-session coverage, PIT, source finality or authority-empty.
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath([IO.Path]::Combine($PSScriptRoot, '../..'))
$probeExe = Join-Path $repo 'target/debug/examples/correlated_query_probe.exe'
$runId = [guid]::NewGuid().ToString('N')
$outputDir = Join-Path $repo "target/business-receipts/$runId"
$null = [IO.Directory]::CreateDirectory($outputDir)
$utf8 = [Text.UTF8Encoding]::new($false)
$cases = switch ($CaseGroup) {
    'Flows' {
        @{ name = 'money'; operation = 'money-flows'; code = 4; provider = 'Eastmoney'; version = 1; schema = 'magic.market.money_flow'; payload = '{"instruments":[{"exchange":"Shenzhen","code":"300005","asset_class":"Equity"}]}' }
        @{ name = 'board'; operation = 'board-flows'; code = 36; provider = 'Eastmoney'; version = 1; schema = 'magic.market.board_flow'; payload = '{"category":"Industry","interval":"Day1","limit":1}' }
    }
    'Historical' {
        foreach ($code in @('688277', '688561')) {
            @{ name = "bars-$code"; operation = 'historical-bars'; code = 1; provider = 'HithinkFinance'; version = 1; schema = 'magic.market.bar'; payload = ('{"instrument":{"exchange":"Shanghai","code":"' + $code + '","asset_class":"Equity"},"interval":"Day","start":"2026-07-16","end":"2026-07-30","limit":15}') }
        }
        @{ name = 'bars-688561-limit1'; operation = 'historical-bars'; code = 1; provider = 'HithinkFinance'; version = 1; schema = 'magic.market.bar'; payload = '{"instrument":{"exchange":"Shanghai","code":"688561","asset_class":"Equity"},"interval":"Day","start":"2026-07-16","end":"2026-07-30","limit":1}' }
    }
    'Announcements' {
        foreach ($version in @(1, 2)) {
            @{ name = "announcements-v$version"; operation = 'market-announcements'; code = 19; provider = 'Cninfo'; version = $version; schema = if ($version -eq 2) { 'magic.market.market_announcements.coverage' } else { 'magic.market.announcement' }; payload = '{"start":"2026-10-01","end":"2026-10-01","limit":300}' }
        }
    }
    'News' {
        foreach ($provider in @('WallstreetCn', 'Jin10', 'Cls', 'Cailianpress', 'Eastmoney', 'ThePaper', 'XinhuaFinance', 'Yicai', 'Yonhap')) {
            @{ name = "news-$provider"; operation = 'global-news'; code = 17; provider = $provider; version = 2; schema = 'magic.market.news_item'; payload = '{"limit":1}' }
        }
    }
}
$results = @()
foreach ($item in $cases) {
    $requestId = "$runId-$($item.name)"
    $arguments = @($Endpoint, $TokenEnvironment, $item.operation, $requestId, $item.payload, '--provider', $item.provider, '--schema-version', [string]$item.version)
    if ($TlsBundle) { $arguments += @('--tls-bundle', $TlsBundle) }
    $arguments += '--receipt-json'
    $stderrFile = Join-Path $outputDir "$($item.name).stderr.log"
    $raw = & $probeExe @arguments 2> $stderrFile
    $captureExit = $LASTEXITCODE
    $text = ($raw | Out-String).Trim()
    [IO.File]::WriteAllText((Join-Path $outputDir "$($item.name).receipt.json"), $text, $utf8)
    $identityValid = $false
    $positiveValid = $false
    $grpcCode = 'CaptureFailure'
    if ($captureExit -eq 0 -and $text) {
        $receipt = $text | ConvertFrom-Json
        $identityValid = $receipt.same_process -eq $true
        foreach ($health in @($receipt.health_before, $receipt.health_after)) {
            $identityValid = $identityValid -and $health.live -and $health.ready -and
                $health.build_identity.source_revision -eq $SourceRevision -and
                $health.build_identity.binary_sha256 -eq $BinarySha256 -and
                $health.build_identity.contract_sha256 -eq $ContractSha256 -and
                [string]::IsNullOrEmpty($health.build_identity.identity_error)
        }
        $grpcCode = $receipt.rpc.grpc_code
        $response = $receipt.rpc.response
        $capabilityValid = @($receipt.capabilities | Where-Object {
            $_.operation -eq $item.code -and $_.provider -eq $item.provider -and
            $_.repository_admission -eq 1 -and $_.runtime_available -eq $true
        }).Count -eq 1
        $positiveValid = $identityValid -and $capabilityValid -and $grpcCode -eq 'OK' -and
            $response.request_id -eq $requestId -and $response.operation -eq $item.code -and
            $response.admission -eq 1 -and $response.selected_provider -eq $item.provider -and
            $response.record_count -gt 0 -and -not [string]::IsNullOrEmpty($response.batch_id)
        foreach ($record in @($response.records)) {
            $positiveValid = $positiveValid -and $record.schema -eq $item.schema -and
                $record.schema_version -eq $item.version -and
                $record.content_type -eq 'application/json; charset=utf-8'
        }
        if ($CaseGroup -eq 'Historical' -and $positiveValid) {
            $request = $receipt.request_json
            $seenDates = [Collections.Generic.HashSet[string]]::new()
            $previousDate = ''
            foreach ($record in $response.records) {
                $data = $record.decoded
                $positiveValid = $positiveValid -and $data.instrument.exchange -eq $request.instrument.exchange -and
                    $data.instrument.code -eq $request.instrument.code -and
                    $data.instrument.asset_class -eq $request.instrument.asset_class -and
                    $data.interval -eq 'Day' -and $data.adjustment -eq 'Unadjusted' -and
                    $data.bar_start -eq $data.bar_end -and $data.source_at -eq $data.bar_start -and
                    $data.bar_start -ge $request.start -and $data.bar_start -le $request.end -and
                    $data.bar_start -gt $previousDate -and $seenDates.Add($data.bar_start) -and
                    $data.provider -eq 'Tonghuashun' -and $data.batch_id -eq $response.batch_id -and
                    $data.observed_at -eq $response.observed_at -and $null -ne $data.volume -and
                    $null -ne $data.amount -and $data.volume -ge 0 -and $data.amount -ge 0
                $previousDate = $data.bar_start
            }
            $positiveValid = $positiveValid -and $response.record_count -le $request.limit
        }
        if ($CaseGroup -eq 'Flows' -and $positiveValid) {
            $data = $response.records[0].decoded
            $evidence = if ($item.name -eq 'money') { $data } else { $data.evidence }
            $positiveValid = $response.complete -eq $true -and $response.record_count -eq 1 -and
                -not [string]::IsNullOrEmpty($response.source_at) -and
                -not [string]::IsNullOrEmpty($response.observed_at) -and
                $evidence.provider -eq 'Eastmoney' -and $evidence.batch_id -eq $response.batch_id -and
                $evidence.source_at -eq $response.source_at -and $evidence.observed_at -eq $response.observed_at
            if ($item.name -eq 'money') {
                $positiveValid = $positiveValid -and $data.instrument.exchange -eq 'Shenzhen' -and
                    $data.instrument.code -eq '300005' -and $data.instrument.asset_class -eq 'Equity' -and
                    $data.status -eq 'Available' -and -not [string]::IsNullOrEmpty($data.source_date)
                foreach ($field in @('main_net', 'super_large_net', 'large_net', 'medium_net', 'small_net')) {
                    $positiveValid = $positiveValid -and $null -ne $data.$field
                }
            } else {
                $positiveValid = $positiveValid -and $data.category -eq 'Industry' -and $data.interval -eq 'Day1' -and
                    $data.rank -eq 1 -and -not [string]::IsNullOrEmpty($data.board_code) -and
                    -not [string]::IsNullOrEmpty($data.board_name)
            }
        }
    }
    $results += [ordered]@{ name = $item.name; request_id = $requestId; capture_exit = $captureExit; grpc_code = $grpcCode; identity_valid = [bool]$identityValid; bounded_positive_valid = [bool]$positiveValid }
    Write-Output "$($item.name): capture=$captureExit grpc=$grpcCode identity=$identityValid bounded_positive=$positiveValid"
}
$accepted = @($results | Where-Object { -not $_.bounded_positive_valid }).Count -eq 0
$summary = [ordered]@{
    run_id = $runId; endpoint = $Endpoint; group = $CaseGroup; observed_at_utc = [DateTimeOffset]::UtcNow.ToString('o')
    expected_identity = @{ source_revision = $SourceRevision; binary_sha256 = $BinarySha256; contract_sha256 = $ContractSha256 }
    criteria = 'same-process identity, admitted capability, positive full CanonicalRecord payload; Historical additionally exact normalized identity/range/unique order/flattened evidence/units shape, Flows additionally complete record/source date/interval evidence'
    certifies_session_coverage = $false; certifies_pit = $false; certifies_authority_empty = $false
    acceptance_exit = if ($accepted) { 0 } else { 2 }; cases = $results
}
[IO.File]::WriteAllText((Join-Path $outputDir 'summary.json'), ($summary | ConvertTo-Json -Depth 20), $utf8)
$manifest = @(Get-ChildItem -LiteralPath $outputDir -File | ForEach-Object {
    @{ path = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(); bytes = $_.Length }
})
[IO.File]::WriteAllText((Join-Path $outputDir 'manifest.json'), ($manifest | ConvertTo-Json -Depth 10), $utf8)
Write-Output "output_dir=$outputDir"
exit $summary.acceptance_exit
