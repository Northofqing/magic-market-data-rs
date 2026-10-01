param(
    [Parameter(Mandatory = $true)][string]$Bundle,
    [string]$Endpoint = "10.211.55.3:50051",
    [string]$TlsServerName = "magic-market.local",
    [string]$RequestId = "r08-futures-delivery-probe",
    [int]$Year = 2026,
    [int]$Month = 9,
    [int]$SchemaVersion = 2,
    [string]$BusinessJson = ""
)

$ErrorActionPreference = "Stop"
$grpcurl = (Get-Command grpcurl.exe -ErrorAction Stop).Source
$token = [IO.File]::ReadAllText((Join-Path $Bundle "bearer-token.txt")).Trim()
if ($token.Length -lt 32) {
    throw "client bundle bearer token is unexpectedly short"
}

$business = if ($BusinessJson) {
    $BusinessJson
} else {
    @{ year = $Year; month = $Month } | ConvertTo-Json -Compress
}
$request = @{
    context = @{ protocol_version = 1; request_id = $RequestId }
    preferred_provider = "Cffex"
    payload = @{
        schema = "magic.market.futures_delivery.request"
        schema_version = $SchemaVersion
        content_type = "application/json; charset=utf-8"
        data = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($business))
    }
    allow_unadmitted = $false
} | ConvertTo-Json -Compress -Depth 8

$request | & $grpcurl `
    -import-path $Bundle `
    -proto market.proto `
    -cacert (Join-Path $Bundle "ca.pem") `
    -cert (Join-Path $Bundle "client.pem") `
    -key (Join-Path $Bundle "client-key.pem") `
    -authority $TlsServerName `
    -max-time 20 `
    -H "authorization: Bearer $token" `
    -d '@' `
    $Endpoint `
    magic.market.v1.MarketDataService/FuturesDelivery
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
