param(
    [Parameter(Mandatory = $true)][string]$Bundle,
    [string]$Endpoint = "10.211.55.3:50051"
)

$ErrorActionPreference = "Stop"
$probe = Join-Path $PSScriptRoot "futures-delivery-grpc.ps1"
$cases = @(
    @{ Name = "legacy-v1"; Args = @{ SchemaVersion = 1; Year = 2026; Month = 9 }; Code = "InvalidArgument"; Message = "version 2" },
    @{ Name = "missing-year"; Args = @{ BusinessJson = '{"month":9}' }; Code = "InvalidArgument"; Message = "missing field" },
    @{ Name = "invalid-month"; Args = @{ Year = 2026; Month = 13 }; Code = "InvalidArgument"; Message = "month must be in 1..=12" },
    @{ Name = "unsupported-2027"; Args = @{ Year = 2027; Month = 9 }; Code = "Unimplemented"; Message = "admitted only for 2026" }
)

foreach ($case in $cases) {
    $requestId = "r08-$($case.Name)-conformance"
    $arguments = @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $probe,
        "-Bundle", $Bundle, "-Endpoint", $Endpoint, "-RequestId", $requestId
    )
    foreach ($key in $case.Args.Keys) {
        $arguments += "-$key"
        $arguments += [string]$case.Args[$key]
    }
    $output = & powershell.exe @arguments 2>&1
    $status = $LASTEXITCODE
    $message = $output -join "`n"
    if ($status -eq 0 -or
        -not $message.Contains("Code: $($case.Code)") -or
        -not $message.Contains($case.Message)) {
        throw "negative case $($case.Name) did not fail as expected: $message"
    }
    Write-Output "case=$($case.Name) grpc_code=$($case.Code)"
}

Write-Output "FuturesDelivery negative conformance: 4 cases passed"
