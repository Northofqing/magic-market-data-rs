# Runtime-root scripts

These five scripts control the gRPC runtime **at the runtime root**, and they are
the canonical source for the copies that live there. They are not executed from
this directory: each derives its own root from `$MyInvocation.MyCommand.Path`, so
they only work next to the `bin\`, `tls\`, `.grpc-token` and `logs\` they
operate on.

| Script | Role |
| --- | --- |
| `start.ps1` | Load provider keys, validate TLS presence, launch the gRPC server then the TDX agent, record both pids |
| `stop.ps1` | Stop the agent, its `magic-market-monitor-server.exe` children, then the server |
| `keepalive.ps1` | Restart only when the runtime is **wholly absent**; verify the admitted binary identities first |
| `status.ps1` | Read-only health check; never starts, stops or repairs |
| `configure-remote.ps1` | Validate the TLS material **in place**; refuses to regenerate it |

## Why they are split from the scripts one level up

`tools/runtime/` holds the half that is invoked **from the repository** and is
therefore path-independent: `windows-autostart.ps1` takes an explicit
`-RuntimeRoot`, and `install-windows-autostart.ps1` registers the current-user Run
entry that calls it. This directory holds the half that is invoked **from the
runtime root** and is rooted by its own location.

## Deployment

The runtime root is deliberately outside the repository:

```
C:\Users\13687\.magic-market-data\runtime\
    bin\        magic-market-grpc-server.exe, magic-market-tdx-agent.exe,
                magic-market-monitor-server.exe, magic-tdx-native-bridge.exe
    tls\        ca.pem, server.pem, server-key.pem, tdx-agent.pem, tdx-agent-key.pem
    logs\       grpc-server.*.log, tdx-agent.*.log, grpc-keepalive.log, archive\
    client-bundle\
    .grpc-token
    start.ps1  stop.ps1  keepalive.ps1  status.ps1  configure-remote.ps1
```

It must not be `<repo>\target\runtime`. The isolated qualified build refuses to
run while that path exists (`run_build.py`, `configs_absent()`:
`"unreviewed Cargo config or existing production runtime"`), so a production
runtime kept there is deleted by the next qualified build. That is how these
scripts were lost once already; see `RECOVERY.md`.

Deploying is a **byte-identical copy**, never a re-typing:

```powershell
$src = Join-Path $PSScriptRoot "runtime-root"     # tools\runtime\runtime-root
$dst = "C:\Users\13687\.magic-market-data\runtime"
foreach ($f in "start.ps1","stop.ps1","keepalive.ps1","status.ps1","configure-remote.ps1") {
    Copy-Item -LiteralPath (Join-Path $src $f) -Destination (Join-Path $dst $f) -Force
}
```

`.gitattributes` pins these paths to `text eol=lf` so a checkout reproduces the
committed bytes exactly. Without it, `core.autocrlf=true` rewrites them to CRLF
on checkout and breaks the `start.ps1` hash pin below -- silently, and only on
machines that have never run the script.

## Pinned identities

`keepalive.ps1` refuses to start anything unless all three match:

| File | SHA-256 |
| --- | --- |
| `bin\magic-market-grpc-server.exe` | `9658a0e19e84ccd2fca657c3dcb5410b9a92a319c4deeabdd1ea4930d2c3acb5` |
| `bin\magic-market-tdx-agent.exe` | `3556e7c672b541bc58093ab2705301fe36c3865dda32f5f16125daa1fafc1004` |
| `start.ps1` | `323107691287245b1a856e761e6ab36738bc54ea082b285df693ae174342a20f` |

The launcher is pinned so a rewritten `start.ps1` cannot be smuggled in behind an
unchanged binary pair. That is also why `.gitattributes` exists: the pin is only
worth anything if the file it names is reproducible from version control.

The binary pair is the admitted candidate built from source revision
`841e4ae7a9df62be0c4536fa9009c66089d282bf`. Rotating either binary means
re-running the qualified build, re-verifying, and updating the pins **in the same
change** -- a pin that does not match its binary is a runtime that will not start.

## Verifying a runtime root

```powershell
& "$RuntimeRoot\configure-remote.ps1"   # TLS present, unexpired, issued by the trusted CA
& "$RuntimeRoot\keepalive.ps1" -CheckOnly   # principal, hashes, token, no foreign runtime; exit 3 = stopped
& "$RuntimeRoot\status.ps1"             # pid files, listener ownership, health; exit 3 = unhealthy
```

`keepalive.ps1 -CheckOnly` exits 0 when healthy and 3 when the runtime is
stopped, having passed every guard on the way. It never starts anything.

## TLS: validation only, by design

`configure-remote.ps1` does not mint certificates. The Mac client trusts exactly
one CA (`CN=MagicMarketData Recovery CA 20261010 v1`), so regenerating the chain
would produce a runtime that reports a successful configuration while every
client connection fails. Rotation requires coordinating the new CA with the Mac
side first, and is a separate authorized change.

The peer name is the TLS authority the agent validates (`--tls-domain
magic-market.local`), not the address it dials. The bind address `10.211.55.3` is
where the socket goes; it is deliberately not required as a SAN.

## Secrets

`.grpc-token` holds the bearer token the Mac client already uses, and `start.ps1`
only generates one when the file is absent -- generating a replacement
invalidates the client's existing credential. Provider API keys are read from
`<repo>\.env.local`, which is why `start.ps1` takes an explicit `-RepoRoot`: the
runtime root is no longer two levels below the repository, so the original
`..\..` walk would resolve to the wrong place. No key or token value is written
into any script, log or error message here.
