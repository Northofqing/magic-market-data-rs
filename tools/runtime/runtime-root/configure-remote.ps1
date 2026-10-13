<#
    Validate the TLS material this runtime root needs, in place.

    Historical behaviour: this script minted a fresh CA, server certificate and agent
    certificate. That is deliberately NOT reproduced here. The Mac client trusts one
    specific CA, so regenerating it would silently break the mTLS trust chain while
    looking like a successful configuration. The original material was recovered from
    the private runtime baseline (see RECOVERY.md in this directory), and the only
    safe operation left is to prove that material is present and consistent.

    To rotate the material you must coordinate a new CA with the Mac side first; that
    is a separate, explicitly authorized change and is out of scope here.
#>
param(
    [string] $ServerIp = "10.211.55.3",
    [string] $ServerDns = "magic-market.local"
)

$ErrorActionPreference = "Stop"

$runtimeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$tlsRoot = Join-Path $runtimeRoot "tls"

$required = @("ca.pem", "server.pem", "server-key.pem", "tdx-agent.pem", "tdx-agent-key.pem")
$missing = @()
foreach ($name in $required) {
    $path = Join-Path $tlsRoot $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        $missing += $path
        continue
    }
    if ((Get-Item -LiteralPath $path).Length -eq 0) {
        $missing += "$path (empty)"
    }
}
if ($missing.Count -gt 0) {
    throw "TLS material is incomplete; do not start the runtime. Missing or empty: $($missing -join '; ')"
}

function Read-PemCertificate([string] $Path) {
    $text = [IO.File]::ReadAllText($Path)
    if ($text -notmatch "-----BEGIN CERTIFICATE-----") {
        throw "$Path is not a PEM certificate"
    }
    return New-Object Security.Cryptography.X509Certificates.X509Certificate2($Path)
}

function Get-SubjectAlternativeNames($Certificate) {
    $extension = $Certificate.Extensions | Where-Object { $_.Oid.Value -eq "2.5.29.17" }
    if ($null -eq $extension) {
        return @()
    }
    # Decode the raw SAN sequence textually: entries appear as DNS/IP value pairs and the
    # probe only needs to confirm the expected identity is present.
    $bytes = $extension.RawData
    $names = @()
    $text = [Text.Encoding]::ASCII.GetString($bytes)
    foreach ($match in [regex]::Matches($text, "[0-9A-Za-z_.:\-]{4,}")) {
        $names += $match.Value
    }
    return $names
}

# X509Certificate2.NotBefore/NotAfter are DateTime in local time on .NET Framework, so
# the comparison boundary is a plain UTC DateTime rather than a DateTimeOffset.
$now = [DateTime]::UtcNow
$ca = Read-PemCertificate (Join-Path $tlsRoot "ca.pem")
$server = Read-PemCertificate (Join-Path $tlsRoot "server.pem")
$agent = Read-PemCertificate (Join-Path $tlsRoot "tdx-agent.pem")

$problems = @()

foreach ($pair in @(@{ Name = "ca.pem"; Cert = $ca }, @{ Name = "server.pem"; Cert = $server }, @{ Name = "tdx-agent.pem"; Cert = $agent })) {
    if ($pair.Cert.NotAfter.ToUniversalTime() -le $now) {
        $problems += "$($pair.Name) expired at $($pair.Cert.NotAfter.ToUniversalTime().ToString('u'))"
    }
    if ($pair.Cert.NotBefore.ToUniversalTime() -gt $now) {
        $problems += "$($pair.Name) is not valid until $($pair.Cert.NotBefore.ToUniversalTime().ToString('u'))"
    }
}

if ($server.Issuer -ne $ca.Subject) {
    $problems += "server.pem is not issued by ca.pem (issuer '$($server.Issuer)' vs ca subject '$($ca.Subject)')"
}
if ($agent.Issuer -ne $ca.Subject) {
    $problems += "tdx-agent.pem is not issued by ca.pem (issuer '$($agent.Issuer)' vs ca subject '$($ca.Subject)')"
}

# The peer name is the TLS authority the agent validates (`--tls-domain`), not the
# address it dials: $ServerIp is only where the socket goes, and requiring it as a SAN
# would reject a certificate that is in fact correct for every current caller.
$serverNames = Get-SubjectAlternativeNames $server
if ($serverNames -notcontains $ServerDns) {
    $problems += "server.pem has no SAN for $ServerDns (found: $($serverNames -join ', '))"
}

foreach ($name in @("server-key.pem", "tdx-agent-key.pem")) {
    $text = [IO.File]::ReadAllText((Join-Path $tlsRoot $name))
    if ($text -notmatch "-----BEGIN (RSA |EC |)PRIVATE KEY-----") {
        $problems += "$name is not a PEM private key"
    }
}

if ($problems.Count -gt 0) {
    throw "TLS material failed validation:`n  - " + ($problems -join "`n  - ")
}

[PSCustomObject]@{
    RuntimeRoot = $runtimeRoot
    TlsRoot = $tlsRoot
    CaSubject = $ca.Subject
    CaExpires = $ca.NotAfter.ToString("u")
    ServerSubject = $server.Subject
    ServerExpires = $server.NotAfter.ToString("u")
    ServerSubjectAlternativeNames = ($serverNames -join ", ")
    AgentSubject = $agent.Subject
    AgentExpires = $agent.NotAfter.ToString("u")
    Configured = $true
    Regenerated = $false
}
