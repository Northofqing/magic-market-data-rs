#!/usr/bin/env python3
"""Repeatable LocalBridgeV1 capability probe (40 declared ops).

Provenance: docs/audits/2026-10-13-bridge-capability-probe.md (stock_analysis
repo, Mac host). That audit probed the local bridge
`com.northofqing.grpc-market-server` (127.0.0.1:18082, plaintext HTTP/2 prior
knowledge) op by op and found 13/40 actually returning data versus a static
40/40 ADMITTED + runtime_available=true capability table. Follow-up item 1 of
that audit asks for this measurement to be repeatable; follow-up item 2 asks
for repeated sampling of the two timing-out ops (46 DragonTiger, 47
MarketDragonTiger).

Read-only: every call is a MarketDataService query. The probe never writes,
never changes server state, and never restarts anything.

Transport: curl with --http2-prior-knowledge (same mechanism the audit used).
curl must be built with HTTP/2; check with `curl --version`.

Wire format (audit section 2):
  gRPC frame      = 0x00 + 4-byte big-endian length + QueryRequest
  QueryRequest    = field 1 (RequestContext, len-delimited)
                  + field 3 (CanonicalPayload, len-delimited)
  RequestContext  = field 1 varint(protocol_version=1) + field 2 string(request_id)
  CanonicalPayload= field 1 string(schema) + field 2 varint(schema_version)
                  + field 3 string("application/json; charset=utf-8")
                  + field 4 bytes(params JSON)

Pitfall fixed (audit method step 4): each op uses its own freshly created
request/response files inside a per-op temporary directory that is deleted
after the call, so a previous op's response can never be misread as this op's
result.

Exit codes: 0 = probe ran to completion (verdicts are data, not pass/fail);
3 = usage/selftest error. Exit stays 0 even when individual ops fail: the
failures are the measurement.

Usage:
  python3 local_bridge_capability_probe.py --selftest
  python3 local_bridge_capability_probe.py --addr http://127.0.0.1:18082 \
      --json /tmp/bridge_probe.json
  python3 local_bridge_capability_probe.py --ops 46,47 --repeat 10
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional
from urllib.parse import unquote

CANONICAL_JSON_CONTENT_TYPE = "application/json; charset=utf-8"
PROTOCOL_VERSION = 1
SYSTEM_SERVICE = "magic.market.v1.SystemService"
MARKET_DATA_SERVICE = "magic.market.v1.MarketDataService"

# The 40 ops declared by the bridge GetCapabilities, numbered by the
# LocalBridgeV1 Operation enum (audit section 3). Schema names/versions mirror
# the frozen Mac-side table src/grpc_contract/schema.rs. Params mirror
# src/grpc_contract/params.rs default/required table; per-code ops pass
# explicit codes (audit section 8) instead of relying on the bridge process
# STOCK_LIST. Placeholders are rendered at run time; override with
# --codes/--date.
CODES_PLACEHOLDER = "__CODES__"
DATE_PLACEHOLDER = "__DATE__"
NOW_PLACEHOLDER = "__NOW__"
INSTRUMENTS_PLACEHOLDER = "__INSTRUMENTS__"

OP_TABLE = [
    {"op": 1, "name": "HistoricalBars", "schema": "market.historical_bars", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "days": 120}},
    {"op": 2, "name": "MinuteData", "schema": "market.minute_data", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 3, "name": "RealtimeQuotes", "schema": "market.realtime_quotes", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 4, "name": "MoneyFlows", "schema": "market.money_flows", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 5, "name": "OrderBooks", "schema": "market.order_books", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 8, "name": "SecurityMetadata", "schema": "market.security_metadata", "version": 1,
     "params": {"instruments": INSTRUMENTS_PLACEHOLDER}},
    {"op": 9, "name": "GlobalIndices", "schema": "market.global_indices", "version": 1,
     "params": {}},
    {"op": 10, "name": "ForeignExchange", "schema": "market.foreign_exchange", "version": 1,
     "params": {}},
    {"op": 11, "name": "EconomicCalendar", "schema": "market.economic_calendar", "version": 1,
     "params": {}},
    {"op": 12, "name": "FuturesDelivery", "schema": "market.futures_delivery", "version": 1,
     "params": {}},
    {"op": 17, "name": "GlobalNews", "schema": "news.global_news", "version": 1,
     "params": {"limit": 5}},
    {"op": 18, "name": "Announcements", "schema": "news.announcements", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "start": "2026-09-01", "end": DATE_PLACEHOLDER}},
    {"op": 23, "name": "FinancialStatements", "schema": "market.financial_statements", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "kind": "income"}},
    {"op": 24, "name": "MarketStatistics", "schema": "market.market_statistics", "version": 1,
     "params": {}},
    {"op": 25, "name": "TechnicalBars", "schema": "market.technical_bars", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "count": 48}},
    {"op": 26, "name": "CorporateActions", "schema": "market.corporate_actions", "version": 1,
     "params": {"code": "600396", "window_start": "2026-06-01", "window_end": DATE_PLACEHOLDER}},
    {"op": 27, "name": "BoardDirectory", "schema": "board.directory", "version": 1,
     "params": {}},
    {"op": 28, "name": "BoardConstituents", "schema": "board.constituents", "version": 1,
     "params": {}},
    {"op": 30, "name": "ResearchReports", "schema": "research.reports", "version": 1,
     "params": {}},
    {"op": 32, "name": "Consensus", "schema": "market.consensus", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 34, "name": "SemanticSearch", "schema": "market.semantic_search", "version": 1,
     "params": {"query": "白酒"}},
    {"op": 35, "name": "FundFlowSeries", "schema": "market.fund_flow_series", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "interval": "day1", "limit": 20}},
    {"op": 36, "name": "BoardFlows", "schema": "board.flows", "version": 1,
     "params": {"category": "Industry", "interval": "day1", "limit": 20}},
    {"op": 38, "name": "BlockTrades", "schema": "market.block_trades", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    {"op": 43, "name": "NorthboundDaily", "schema": "market.northbound_daily", "version": 1,
     "params": {}},
    {"op": 44, "name": "LimitPools", "schema": "market.limit_pools", "version": 1,
     "params": {"date": DATE_PLACEHOLDER}},
    {"op": 45, "name": "StrongStockReasons", "schema": "market.strong_stock_reasons", "version": 1,
     "params": {}},
    {"op": 46, "name": "DragonTiger", "schema": "market.dragon_tiger", "version": 1,
     "params": {"date": DATE_PLACEHOLDER}},
    {"op": 47, "name": "MarketDragonTiger", "schema": "market.market_dragon_tiger", "version": 1,
     "params": {"date": DATE_PLACEHOLDER}},
    {"op": 49, "name": "MarketRankings", "schema": "market.market_rankings", "version": 1,
     "params": {}},
    {"op": 52, "name": "ConceptHits", "schema": "market.concept_hits", "version": 1,
     "params": {}},
    {"op": 54, "name": "ProviderTopNRankings", "schema": "market.provider_top_n_rankings", "version": 1,
     "params": {"date": DATE_PLACEHOLDER}},
    {"op": 55, "name": "InstrumentNews", "schema": "news.instrument_news", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER, "from_days": 30}},
    {"op": 56, "name": "IndexQuotes", "schema": "market.index_quotes", "version": 1,
     "params": {"codes": ["sh000001", "sz399001", "sz399006", "sh000688", "sh000016", "sh000300"]}},
    {"op": 57, "name": "IntradayShape", "schema": "market.intraday_shape", "version": 1,
     "params": {"codes": CODES_PLACEHOLDER}},
    # Frozen v2 (upstream contract `2026-08-27.1`); the Mac-side table already
    # carries 2 with a frozen test (audit follow-up 3, already fixed there).
    {"op": 58, "name": "T0Evidence", "schema": "market.t0_evidence", "version": 2,
     "params": {"codes": CODES_PLACEHOLDER, "observed_at": NOW_PLACEHOLDER}},
    # Wire not converged as of 2026-10-13 (audit section 5.2): the server
    # demanded market -> code -> expected_bar_count -> maximum_latest_n ->
    # window_start -> instrument, layer by layer. Seed stays `{}` so the next
    # run reproduces the layering from the same starting point.
    {"op": 59, "name": "OutcomeDailyBars", "schema": "market.outcome_daily_bars", "version": 1,
     "params": {}},
    {"op": 60, "name": "UpperLimitPoolReview", "schema": "market.upper_limit_pool_review", "version": 1,
     "params": {"date": DATE_PLACEHOLDER}},
    {"op": 61, "name": "ChainBatch", "schema": "market.chain_batch", "version": 1,
     "params": {}},
    # Wire not converged as of 2026-10-13 (audit section 5.2): granularity must
    # be a specific serde variant (`Day` passes, `day1`/`D1` rejected) and
    # `from` must be an internally tagged object, not a string.
    {"op": 62, "name": "BenchmarkBars", "schema": "market.benchmark_bars", "version": 1,
     "params": {}},
]

# Op 19 is implemented on neither side of the frozen table; probing it is
# optional and off by default (audit section 5.4: grpc 12, not in the 40).
UNDECLARED_OPS = [
    {"op": 19, "name": "MarketAnnouncements", "schema": "news.market_announcements", "version": 1,
     "params": {"start": "2026-09-01", "end": DATE_PLACEHOLDER, "limit": 300}},
]

MAIN_INDICES = ["sh000001", "sz399001", "sz399006", "sh000688", "sh000016", "sh000300"]


def exchange_of(code):
    first = code[:1]
    if first == "6":
        return "Shanghai"
    if first in ("0", "3"):
        return "Shenzhen"
    if first in ("4", "8", "9"):
        return "Beijing"
    return "Unknown"


def instruments_for(codes):
    return [
        {"exchange": exchange_of(code), "code": code, "asset_class": "Equity"}
        for code in codes
    ]


# --------------------------------------------------------------------------
# Minimal protobuf wire codec (enough for QueryRequest / QueryResponse).
# --------------------------------------------------------------------------

def _varint(value):
    if value < 0:
        raise ValueError("varint cannot encode negative values")
    out = bytearray()
    while True:
        bits = value & 0x7F
        value >>= 7
        if value:
            out.append(bits | 0x80)
        else:
            out.append(bits)
            return bytes(out)


def _tag(field_number, wire_type):
    return _varint((field_number << 3) | wire_type)


def pb_varint(field_number, value):
    return _tag(field_number, 0) + _varint(value)


def pb_bytes(field_number, data):
    return _tag(field_number, 2) + _varint(len(data)) + data


def pb_string(field_number, text):
    return pb_bytes(field_number, text.encode("utf-8"))


def build_query_request(request_id, schema, schema_version, params, provider=None):
    context = pb_varint(1, PROTOCOL_VERSION) + pb_string(2, request_id)
    payload = (
        pb_string(1, schema)
        + pb_varint(2, schema_version)
        + pb_string(3, CANONICAL_JSON_CONTENT_TYPE)
        + pb_bytes(4, params)
    )
    message = pb_bytes(1, context)
    if provider:
        message += pb_string(2, provider)
    return message + pb_bytes(3, payload)


def grpc_frame(message):
    return b"\x00" + len(message).to_bytes(4, "big") + message


class ProtoReader:
    """Sequential protobuf field reader over a single message payload."""

    def __init__(self, data):
        self.data = data
        self.pos = 0

    def _read_varint(self):
        result = 0
        shift = 0
        while True:
            if self.pos >= len(self.data):
                raise ValueError("truncated varint")
            byte = self.data[self.pos]
            self.pos += 1
            result |= (byte & 0x7F) << shift
            if not byte & 0x80:
                return result
            shift += 7
            if shift > 70:
                raise ValueError("varint too long")

    def fields(self):
        """Yield (field_number, wire_type, value) until the buffer ends."""
        while self.pos < len(self.data):
            key = self._read_varint()
            number, wire_type = key >> 3, key & 0x07
            if wire_type == 0:
                yield number, wire_type, self._read_varint()
            elif wire_type == 2:
                length = self._read_varint()
                end = self.pos + length
                if end > len(self.data):
                    raise ValueError("truncated length-delimited field")
                value = self.data[self.pos:end]
                self.pos = end
                yield number, wire_type, value
            elif wire_type == 5:
                end = self.pos + 4
                if end > len(self.data):
                    raise ValueError("truncated 32-bit field")
                value = self.data[self.pos:end]
                self.pos = end
                yield number, wire_type, value
            elif wire_type == 1:
                end = self.pos + 8
                if end > len(self.data):
                    raise ValueError("truncated 64-bit field")
                value = self.data[self.pos:end]
                self.pos = end
                yield number, wire_type, value
            else:
                raise ValueError(f"unsupported wire type {wire_type}")


@dataclass
class QueryResponseSummary:
    operation: Optional[int] = None
    provider: Optional[str] = None
    batch_id: Optional[str] = None
    complete: Optional[bool] = None
    records: int = 0
    diagnostic_blocker: Optional[str] = None


def parse_query_response(payload):
    summary = QueryResponseSummary()
    for number, _wire, value in ProtoReader(payload).fields():
        if number == 2:
            summary.operation = value
        elif number == 4:
            summary.provider = value.decode("utf-8", "replace")
        elif number == 5:
            summary.batch_id = value.decode("utf-8", "replace")
        elif number == 6:
            summary.complete = bool(value)
        elif number == 9:
            summary.records += 1
        elif number == 10:
            summary.diagnostic_blocker = value.decode("utf-8", "replace")
    return summary


@dataclass
class CapabilitySummary:
    operation: int
    runtime_available: bool
    provider: str = ""
    blocker: str = ""


def parse_capabilities_response(payload):
    capabilities = []
    for number, _wire, value in ProtoReader(payload).fields():
        if number != 2:
            continue
        cap = CapabilitySummary(operation=-1, runtime_available=False)
        for cnum, _cwire, cval in ProtoReader(value).fields():
            if cnum == 1:
                cap.operation = cval
            elif cnum == 3:
                cap.runtime_available = bool(cval)
            elif cnum == 4:
                cap.provider = cval.decode("utf-8", "replace")
            elif cnum == 6:
                cap.blocker = cval.decode("utf-8", "replace")
        capabilities.append(cap)
    return capabilities


def split_grpc_frames(data):
    """Split a gRPC body into (data bytes, trailer bytes)."""
    data_out = bytearray()
    trailer_out = bytearray()
    pos = 0
    while pos + 5 <= len(data):
        flags = data[pos]
        length = int.from_bytes(data[pos + 1:pos + 5], "big")
        pos += 5
        end = pos + length
        if end > len(data):
            break
        if flags & 0x80:
            trailer_out += data[pos:end]
        else:
            data_out += data[pos:end]
        pos = end
    return bytes(data_out), bytes(trailer_out)


def parse_trailers(headers_text):
    """Extract (grpc-status, grpc-message) from curl -D output (headers +
    trailers are both dumped there; trailers come last)."""
    status = None
    message = ""
    for line in headers_text.splitlines():
        if ":" not in line:
            continue
        name, _, value = line.partition(":")
        name = name.strip().lower()
        value = value.strip()
        if name == "grpc-status":
            try:
                status = int(value)
            except ValueError:
                status = None
        elif name == "grpc-message":
            message = unquote(value)
    return status, message


# --------------------------------------------------------------------------
# Transport: curl --http2-prior-knowledge, per-op isolated temp files.
# --------------------------------------------------------------------------

@dataclass
class CallResult:
    verdict: str  # OK | UPSTREAM_FAIL | INVALID_ARG | UNIMPLEMENTED | TIMEOUT | TRANSPORT
    grpc_status: Optional[int]
    message: str
    bytes_received: int
    provider: Optional[str] = None
    batch_prefix: Optional[str] = None
    records: Optional[int] = None
    curl_code: Optional[int] = None
    raw_body: bytes = b""


def check_transport():
    """Pick curl (preferred) or the python-h2 fallback; exit 3 if neither."""
    curl = shutil.which("curl")
    if curl is not None:
        version = subprocess.run(
            [curl, "--version"], capture_output=True, text=True
        ).stdout
        if "HTTP2" in version.upper():
            return "curl"
    try:
        import h2.connection  # noqa: F401
        return "h2"
    except ImportError:
        print(
            "no usable HTTP/2 transport: curl lacks nghttp2 and the python "
            "'h2' package is not installed (pip install h2)",
            file=sys.stderr,
        )
        raise SystemExit(3)


def _h2_call(host, port, service, method, extra_headers, body, max_time,
             tls=None):
    """Minimal HTTP/2 unary call via python-h2. Plaintext uses prior
    knowledge; with tls={ca,cert,key} the connection is mTLS and the
    ':scheme' pseudo-header is https."""
    import socket
    from h2.config import H2Configuration
    from h2.connection import H2Connection
    from h2.events import (
        ConnectionTerminated,
        DataReceived,
        ResponseReceived,
        StreamEnded,
        StreamReset,
        TrailersReceived,
    )

    raw_sock = socket.create_connection((host, port), timeout=max_time)
    if tls:
        import ssl
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(tls["ca"])
        context.load_cert_chain(tls["cert"], tls["key"])
        context.set_alpn_protocols(["h2"])
        sock = context.wrap_socket(raw_sock, server_hostname=host)
        if sock.selected_alpn_protocol() != "h2":
            sock.close()
            return 2, b"", None, "TLS ALPN did not negotiate h2"
    else:
        sock = raw_sock
    sock.settimeout(max_time)
    try:
        conn = H2Connection(config=H2Configuration(client_side=True))
        conn.initiate_connection()
        sock.sendall(conn.data_to_send())
        stream_id = conn.get_next_available_stream_id()
        headers = [
            (":method", "POST"),
            (":scheme", "https" if tls else "http"),
            (":authority", f"{host}:{port}"),
            (":path", f"/{service}/{method}"),
            ("content-type", "application/grpc"),
            ("te", "trailers"),
        ] + extra_headers
        conn.send_headers(stream_id, headers)
        conn.send_data(stream_id, body, end_stream=True)
        sock.sendall(conn.data_to_send())

        body_bytes = bytearray()
        grpc_status = None
        grpc_message = ""
        ended = False
        while not ended:
            try:
                data = sock.recv(65535)
            except socket.timeout:
                return 28, b"", None, "socket timeout"
            if not data:
                break
            for event in conn.receive_data(data):
                if isinstance(event, DataReceived) and event.stream_id == stream_id:
                    body_bytes += event.data
                    conn.acknowledge_received_data(
                        event.flow_controlled_length, stream_id
                    )
                elif isinstance(event, (ResponseReceived, TrailersReceived)) and (
                    event.stream_id == stream_id
                ):
                    for name, value in event.headers:
                        if name == b"grpc-status":
                            try:
                                grpc_status = int(value)
                            except ValueError:
                                grpc_status = None
                        elif name == b"grpc-message":
                            grpc_message = unquote(value.decode("utf-8", "replace"))
                elif isinstance(event, (StreamEnded, StreamReset)) and (
                    event.stream_id == stream_id
                ):
                    ended = True
                elif isinstance(event, ConnectionTerminated):
                    ended = True
            outgoing = conn.data_to_send()
            if outgoing:
                sock.sendall(outgoing)
        return 0, bytes(body_bytes), grpc_status, grpc_message
    finally:
        sock.close()


def _curl_call(addr, service, method, body, max_time, workdir, auth_header):
    """curl --http2-prior-knowledge transport; returns (raw, grpc_status,
    grpc_message, curl_returncode, stderr)."""
    req_file = workdir / "request.bin"
    resp_file = workdir / "response.bin"
    headers_file = workdir / "headers.txt"
    # Method-step-4 pitfall guard: files are created fresh inside a per-call
    # temp dir and deleted here before use, so a stale response can never be
    # misread as this call's result.
    for path in (req_file, resp_file, headers_file):
        if path.exists():
            path.unlink()
    req_file.write_bytes(body)

    curl = shutil.which("curl")
    assert curl is not None
    headers = [
        "-H", "content-type: application/grpc",
        "-H", "te: trailers",
    ]
    if auth_header:
        headers += ["-H", auth_header]
    proc = subprocess.run(
        [
            curl, "-sS", "--http2-prior-knowledge", "-X", "POST",
            *headers,
            "--data-binary", f"@{req_file}",
            "-D", str(headers_file),
            "-o", str(resp_file),
            "--max-time", str(max_time),
            f"{addr}/{service}/{method}",
        ],
        capture_output=True,
        text=True,
    )
    raw = resp_file.read_bytes() if resp_file.exists() else b""
    headers_text = (headers_file.read_text(errors="replace")
                    if headers_file.exists() else "")
    grpc_status, grpc_message = parse_trailers(headers_text)
    return raw, grpc_status, grpc_message, proc.returncode, proc.stderr.strip()


def call_unary(addr, service, method, request_id, body, max_time, workdir,
               auth_header=None, transport="curl", tls=None):
    if transport == "h2":
        from urllib.parse import urlparse
        parsed = urlparse(addr)
        extra = []
        if auth_header:
            extra.append(("authorization", auth_header.split(": ", 1)[1]))
        code, raw, grpc_status, grpc_message = _h2_call(
            parsed.hostname, parsed.port or (443 if tls else 80),
            service, method, extra, body, max_time, tls,
        )
        returncode, stderr = code, grpc_message
    else:
        raw, grpc_status, grpc_message, returncode, stderr = _curl_call(
            addr, service, method, body, max_time, workdir, auth_header,
        )

    if returncode == 28:
        return CallResult(
            verdict="TIMEOUT", grpc_status=grpc_status,
            message=grpc_message or stderr or "no response bytes",
            bytes_received=len(raw), curl_code=returncode,
        )
    if grpc_status is None:
        return CallResult(
            verdict="TRANSPORT", grpc_status=None,
            message=f"transport failure (code {returncode}): {stderr or grpc_message or 'no grpc-status'}",
            bytes_received=len(raw), curl_code=returncode,
        )
    data_bytes, _trailer_bytes = split_grpc_frames(raw)
    summary = parse_query_response(data_bytes) if data_bytes else QueryResponseSummary()
    verdict = {
        0: "OK",
        3: "INVALID_ARG",
        12: "UNIMPLEMENTED",
        13: "UPSTREAM_FAIL",
    }.get(grpc_status, f"GRPC_{grpc_status}")
    return CallResult(
        verdict=verdict,
        grpc_status=grpc_status,
        message=grpc_message,
        bytes_received=len(raw),
        provider=summary.provider,
        batch_prefix=(summary.batch_id[:40] + "…")
        if summary.batch_id and len(summary.batch_id) > 40 else summary.batch_id,
        records=summary.records if grpc_status == 0 else None,
        raw_body=raw,
    )


def render_params(template, codes, date, now):
    def render(value):
        if value == CODES_PLACEHOLDER:
            return codes
        if value == INSTRUMENTS_PLACEHOLDER:
            return instruments_for(codes)
        if isinstance(value, str):
            return value.replace(DATE_PLACEHOLDER, date).replace(NOW_PLACEHOLDER, now)
        if isinstance(value, list):
            return [render(item) for item in value]
        if isinstance(value, dict):
            return {key: render(item) for key, item in value.items()}
        return value

    return {key: render(value) for key, value in template.items()}


# --------------------------------------------------------------------------
# Selftest: no network. Round-trip the wire codec and validate the table.
# --------------------------------------------------------------------------

def selftest():
    request_id = "selftest-req-1"
    schema = "market.historical_bars"
    params = json.dumps({"codes": ["600396"], "days": 120}).encode("utf-8")
    message = build_query_request(request_id, schema, 1, params)
    frame = grpc_frame(message)
    assert frame[0] == 0, "data frame flag"
    assert int.from_bytes(frame[1:5], "big") == len(message), "frame length"

    context_payload = None
    canonical_payload = None
    for number, wire, value in ProtoReader(message).fields():
        assert wire == 2
        if number == 1:
            context_payload = value
        elif number == 3:
            canonical_payload = value
    assert context_payload is not None and canonical_payload is not None

    fields = {n: v for n, _w, v in ProtoReader(context_payload).fields()}
    assert fields[1] == PROTOCOL_VERSION, "protocol version"
    assert fields[2].decode() == request_id, "request id"

    fields = {n: v for n, _w, v in ProtoReader(canonical_payload).fields()}
    assert fields[1].decode() == schema, "schema"
    assert fields[2] == 1, "schema version"
    assert fields[3].decode() == CANONICAL_JSON_CONTENT_TYPE, "content type"
    assert json.loads(fields[4].decode()) == {"codes": ["600396"], "days": 120}, "params"

    # Synthetic QueryResponse: provider Tdx, batch id, complete=true, 2 records.
    record = pb_string(1, "market.historical_bars") + pb_varint(2, 1) + pb_bytes(4, b"{}")
    response = (
        pb_string(1, request_id)
        + pb_varint(2, 1)
        + pb_string(4, "Tdx")
        + pb_string(5, "tdx-smart:1791905576:abc")
        + pb_varint(6, 1)
        + pb_bytes(9, record)
        + pb_bytes(9, record)
    )
    summary = parse_query_response(response)
    assert summary.provider == "Tdx"
    assert summary.batch_id == "tdx-smart:1791905576:abc"
    assert summary.complete is True
    assert summary.records == 2

    cap = pb_varint(1, 58) + pb_varint(3, 1) + pb_string(4, "Tdx")
    caps = parse_capabilities_response(pb_bytes(2, cap))
    assert len(caps) == 1 and caps[0].operation == 58 and caps[0].runtime_available

    data_bytes, _ = split_grpc_frames(grpc_frame(response))
    assert parse_query_response(data_bytes).records == 2

    status, message = parse_trailers(
        "HTTP/2 200\r\ngrpc-status: 13\r\n"
        "grpc-message: %E5%8F%96%E6%95%B0%E5%A4%B1%E8%B4%A5\r\n"
    )
    assert status == 13 and message == "取数失败", "percent-decoded grpc-message"

    # OP_TABLE integrity.
    declared = list(OP_TABLE)
    assert len(declared) == 40, f"expected 40 declared ops, got {len(declared)}"
    numbers = [entry["op"] for entry in declared]
    names = [entry["name"] for entry in declared]
    assert len(set(numbers)) == 40, "duplicate op numbers"
    assert len(set(names)) == 40, "duplicate op names"
    assert all(entry["version"] >= 1 for entry in declared), "schema versions positive"
    t0 = next(entry for entry in declared if entry["op"] == 58)
    assert t0["version"] == 2, "T0Evidence must stay frozen at v2"
    rendered = render_params(t0["params"], ["600396"], "2026-10-14", "2026-10-14T20:00:00+08:00")
    assert rendered["codes"] == ["600396"] and rendered["observed_at"].endswith("+08:00")
    assert instruments_for(["600396", "000001", "430001"])[2]["exchange"] == "Beijing"
    assert exchange_of("600396") == "Shanghai" and exchange_of("000001") == "Shenzhen"

    print("selftest OK: wire codec round-trip, response/capability/trailer parsing, 40-op table")
    return 0


# --------------------------------------------------------------------------
# Probe driver.
# --------------------------------------------------------------------------

@dataclass
class OpReport:
    op: int
    name: str
    schema: str
    version: int
    rounds: list = field(default_factory=list)

    @property
    def verdict(self):
        if len(self.rounds) == 1:
            return self.rounds[0].verdict
        distribution = {}
        for result in self.rounds:
            distribution[result.verdict] = distribution.get(result.verdict, 0) + 1
        parts = sorted(distribution.items())
        return "/".join(f"{verdict}x{count}" for verdict, count in parts)

    def to_json(self):
        return {
            "op": self.op,
            "name": self.name,
            "schema": self.schema,
            "schema_version": self.version,
            "verdict": self.verdict,
            "rounds": [
                {
                    "verdict": r.verdict,
                    "grpc_status": r.grpc_status,
                    "message": r.message,
                    "bytes": r.bytes_received,
                    "provider": r.provider,
                    "batch_prefix": r.batch_prefix,
                    "records": r.records,
                }
                for r in self.rounds
            ],
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--addr", default="http://127.0.0.1:18082", help="bridge base address")
    parser.add_argument("--max-time", type=float, default=20.0, help="per-call curl timeout seconds")
    parser.add_argument("--repeat", type=int, default=1, help="rounds per op (46/47 sampling)")
    parser.add_argument("--ops", default="", help="comma-separated op numbers (default: all 40)")
    parser.add_argument("--all", action="store_true",
                        help="also probe undeclared ops (19 MarketAnnouncements)")
    parser.add_argument("--codes", default="600396", help="comma-separated codes for per-code ops")
    parser.add_argument("--date", default=dt.date.today().isoformat(), help="trade date for date ops")
    parser.add_argument("--json", dest="json_path", default="", help="write machine-readable report")
    parser.add_argument("--selftest", action="store_true", help="offline wire/table checks, no network")
    parser.add_argument("--token-env", dest="token_env", default="",
                        help="environment variable holding a Bearer token; sends an "
                             "authorization header on every call (upstream auth)")
    parser.add_argument("--token-file", dest="token_file", default="",
                        help="file holding a Bearer token (alternative to --token-env)")
    parser.add_argument("--tls-ca", dest="tls_ca", default="",
                        help="CA bundle for mTLS (enables TLS with the h2 transport)")
    parser.add_argument("--tls-cert", dest="tls_cert", default="",
                        help="client certificate for mTLS")
    parser.add_argument("--tls-key", dest="tls_key", default="",
                        help="client private key for mTLS")
    parser.add_argument("--provider", dest="provider", default="",
                        help="preferred_provider for every call (table entry 'provider' wins)")
    parser.add_argument("--table", dest="table_path", default="",
                        help="JSON file with a custom op table (same shape as OP_TABLE); "
                             "replaces the built-in LocalBridgeV1 40-op table")
    args = parser.parse_args()

    if args.selftest:
        return selftest()

    transport = check_transport()
    auth_header = None
    token = ""
    if args.token_env:
        token = __import__("os").environ.get(args.token_env, "")
        if not token:
            print(f"token environment {args.token_env} is not set", file=sys.stderr)
            return 3
    elif args.token_file:
        token = Path(args.token_file).read_text(encoding="utf-8").strip()
    if token:
        auth_header = f"authorization: Bearer {token}"
    tls = None
    if args.tls_ca or args.tls_cert or args.tls_key:
        if not (args.tls_ca and args.tls_cert and args.tls_key):
            print("--tls-ca/--tls-cert/--tls-key must be supplied together",
                  file=sys.stderr)
            return 3
        if transport != "h2":
            print("mTLS probing requires the h2 transport (curl path is plaintext only)",
                  file=sys.stderr)
            return 3
        tls = {"ca": args.tls_ca, "cert": args.tls_cert, "key": args.tls_key}
    table_override = None
    if args.table_path:
        table_override = json.loads(Path(args.table_path).read_text(encoding="utf-8"))
        if not isinstance(table_override, list) or not table_override:
            print("--table must be a non-empty JSON array of op entries", file=sys.stderr)
            return 3
    codes = [c.strip() for c in args.codes.split(",") if c.strip()]
    now = dt.datetime.now(dt.timezone(dt.timedelta(hours=8))).isoformat(timespec="seconds")
    run_id = f"probe-{dt.datetime.now().strftime('%Y%m%d%H%M%S')}"

    # Capabilities cross-check (audit section 0: declared vs real).
    caps_request_id = f"{run_id}-capabilities"
    caps_body = grpc_frame(pb_bytes(1, pb_varint(1, PROTOCOL_VERSION) + pb_string(2, caps_request_id)))
    with tempfile.TemporaryDirectory(prefix="bridge-probe-caps-") as tmp:
        caps_result = call_unary(
            args.addr, SYSTEM_SERVICE, "GetCapabilities", caps_request_id,
            caps_body, args.max_time, Path(tmp), auth_header, transport, tls,
        )
    capabilities = []
    if caps_result.grpc_status == 0 and caps_result.raw_body:
        data_bytes, _trailer = split_grpc_frames(caps_result.raw_body)
        capabilities = parse_capabilities_response(data_bytes)
    declared_available = sum(1 for cap in capabilities if cap.runtime_available)

    table = list(table_override) if table_override is not None else list(OP_TABLE)
    if not table_override and args.all:
        table += UNDECLARED_OPS
    if args.ops:
        wanted = {int(item) for item in args.ops.split(",")}
        table = [entry for entry in table if entry["op"] in wanted]
        if not table:
            print(f"no ops match --ops {args.ops}", file=sys.stderr)
            return 3

    reports = []
    for entry in table:
        params = json.dumps(
            render_params(entry["params"], codes, args.date, now),
            ensure_ascii=False,
        ).encode("utf-8")
        provider = args.provider or entry.get("provider")
        body = grpc_frame(build_query_request(
            f"{run_id}-{entry['op']}", entry["schema"], entry["version"], params,
            provider,
        ))
        report = OpReport(op=entry["op"], name=entry["name"],
                          schema=entry["schema"], version=entry["version"])
        for round_index in range(max(1, args.repeat)):
            # Per-op, per-round isolated temp dir: files are fresh, deleted on
            # close, and can never leak across ops or rounds.
            with tempfile.TemporaryDirectory(prefix=f"bridge-probe-{entry['op']}-") as tmp:
                report.rounds.append(call_unary(
                    args.addr, MARKET_DATA_SERVICE, entry["name"],
                    f"{run_id}-{entry['op']}-r{round_index}", body,
                    args.max_time, Path(tmp), auth_header, transport, tls,
                ))
        last = report.rounds[-1]
        print(f"{entry['op']:>3} {entry['name']:<24} v{entry['version']} {report.verdict:<18} "
              f"{last.bytes_received:>7}B {last.provider or '-':<10} {last.message[:60]}")
        reports.append(report)

    ok = sum(1 for r in reports if r.rounds[-1].verdict == "OK")
    upstream = sum(1 for r in reports if r.rounds[-1].verdict == "UPSTREAM_FAIL")
    invalid = sum(1 for r in reports if r.rounds[-1].verdict == "INVALID_ARG")
    timeout = sum(1 for r in reports if r.rounds[-1].verdict == "TIMEOUT")
    other = len(reports) - ok - upstream - invalid - timeout
    print(f"\nsummary: OK={ok} upstream_fail={upstream} invalid_arg={invalid} "
          f"timeout={timeout} other={other} of {len(reports)} probed")
    if capabilities:
        declared_ops = {cap.operation for cap in capabilities}
        table_ops = {entry["op"] for entry in table}
        print(f"capabilities: {len(capabilities)} declared, {declared_available} runtime_available=true; "
              f"declared-not-in-table={sorted(declared_ops - table_ops)}, "
              f"table-not-declared={sorted(table_ops - declared_ops)}")
    else:
        print(f"warning: GetCapabilities unavailable (grpc-status {caps_result.grpc_status}); "
              f"declared-vs-real comparison skipped", file=sys.stderr)

    if args.json_path:
        report_doc = {
            "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "addr": args.addr,
            "max_time_seconds": args.max_time,
            "repeat": args.repeat,
            "codes": codes,
            "date": args.date,
            "summary": {"OK": ok, "upstream_fail": upstream, "invalid_arg": invalid,
                        "timeout": timeout, "other": other, "probed": len(reports)},
            "capabilities": [
                {"op": cap.operation, "runtime_available": cap.runtime_available,
                 "provider": cap.provider, "blocker": cap.blocker}
                for cap in capabilities
            ],
            "ops": [report.to_json() for report in reports],
        }
        Path(args.json_path).write_text(
            json.dumps(report_doc, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(f"report written to {args.json_path}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
