param(
    [Parameter(Mandatory = $true)][string]$Bundle,
    [string]$Endpoint = "10.211.55.3:50051"
)

$ErrorActionPreference = "Stop"
$dates = @(
    "2026-01-16", "2026-02-24", "2026-03-20", "2026-04-17",
    "2026-05-15", "2026-06-22", "2026-07-17", "2026-08-21",
    "2026-09-18", "2026-10-16", "2026-11-20", "2026-12-18"
)
$products = @("If", "Ih", "Ic", "Im")
$prefixes = @("IF", "IH", "IC", "IM")
$ruleUrls = @(
    "https://www.cffex.com.cn/cn/hs300.html",
    "https://www.cffex.com.cn/cn/sz50gzqh.html",
    "https://www.cffex.com.cn/cn/zz500.html",
    "https://www.cffex.com.cn/zz1000/"
)
$holidayCalendar = "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf"
$probe = Join-Path $PSScriptRoot "futures-delivery-grpc.ps1"

for ($month = 1; $month -le 12; $month++) {
    $responseText = & $probe -Bundle $Bundle -Endpoint $Endpoint -Year 2026 `
        -Month $month -RequestId ("r08-2026-{0:00}-conformance" -f $month)
    $response = ($responseText -join "`n") | ConvertFrom-Json
    $batch = "cffex-equity-index-planned-delivery-2026-v2:{0:00}" -f $month
    if ($response.requestId -ne ("r08-2026-{0:00}-conformance" -f $month) -or
        $response.operation -ne "OPERATION_FUTURES_DELIVERY" -or
        $response.admission -ne "ADMISSION_STATE_ADMITTED" -or
        $response.selectedProvider -ne "Cffex" -or
        $response.batchId -ne $batch -or
        $response.complete -ne $true -or
        $response.records.Count -ne 4 -or
        -not [string]::IsNullOrEmpty($response.sourceAt) -or
        [string]::IsNullOrEmpty($response.observedAt)) {
        throw "invalid outer FuturesDelivery response for month $month"
    }
    for ($index = 0; $index -lt 4; $index++) {
        $wire = $response.records[$index]
        if ($wire.schema -ne "magic.market.futures_delivery_event" -or
            $wire.schemaVersion -ne 2 -or
            $wire.contentType -ne "application/json; charset=utf-8") {
            throw "invalid record envelope for month $month row $index"
        }
        $record = [Text.Encoding]::UTF8.GetString(
            [Convert]::FromBase64String($wire.data)
        ) | ConvertFrom-Json
        $contractCode = "{0}{1:00}{2:00}" -f $prefixes[$index], 26, $month
        if ($record.product -ne $products[$index] -or
            $record.contract_code -ne $contractCode -or
            $record.last_trading_date -ne $dates[$month - 1] -or
            $record.delivery_date -ne $dates[$month - 1] -or
            $record.method -ne "Cash" -or
            $record.schedule_status -ne "Planned" -or
            $record.date_basis -ne "CffexRuleAndPublishedHolidays" -or
            $record.rule_url -ne $ruleUrls[$index] -or
            $record.holiday_calendar_url -ne $holidayCalendar -or
            $null -ne $record.notice_url -or
            $record.evidence.provider -ne "Cffex" -or
            $null -ne $record.evidence.source_at -or
            $record.evidence.observed_at -ne $response.observedAt -or
            $record.evidence.batch_id -ne $batch) {
            throw "invalid FuturesDelivery record for month $month row $index"
        }
    }
    Write-Output ("month={0:00} delivery={1} records=4 batch={2}" -f `
        $month, $dates[$month - 1], $batch)
}

Write-Output "2026 FuturesDelivery conformance: 12 months, 48 records passed"
