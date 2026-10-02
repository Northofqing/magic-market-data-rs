"""Zero-network research verification; not an admitted market-data product."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from decimal import Decimal
from enum import Enum
from hashlib import sha256
from html.parser import HTMLParser

TITLE = "关于股指期货和股指期权合约交割的通知"
NOTICE_PATH = "/cn/jystz/20260918/48624.html"
SOURCE_DATE = "2026-09-18"
ISSUE = "中金所发〔2026〕47号"
CONTRACTS = ("IF2609", "IH2609", "IC2609", "IM2609")
PRICES = ("4508.38", "2862.60", "7789.72", "7679.21")
PRODUCT_NAMES = dict(zip(CONTRACTS, ("沪深300", "上证50", "中证500", "中证1000")))
OPTIONS = {"IF2609": "IO2609", "IH2609": "HO2609", "IM2609": "MO2609"}


class EvidenceErrorCode(Enum):
    UNSUPPORTED_MONTH = "UnsupportedMonth"
    INPUT_LENGTH_MISMATCH = "InputLengthMismatch"
    DIGEST_MISMATCH = "DigestMismatch"
    INDEX_IDENTITY_MISMATCH = "IndexIdentityMismatch"
    DECODE_REJECTED = "DecodeRejected"
    PUBLICATION_DATE_MISMATCH = "PublicationDateMismatch"
    AMBIGUOUS_DOCUMENT = "AmbiguousDocument"
    ISSUE_MISMATCH = "IssueMismatch"
    CONTRACT_SET_MISMATCH = "ContractSetMismatch"
    PRICE_UNIT_MISMATCH = "PriceUnitMismatch"
    CONFLICTING_FACT = "ConflictingFact"
    HTTP_METADATA_MISMATCH = "HttpMetadataMismatch"


class EvidenceError(ValueError):
    def __init__(self, code: EvidenceErrorCode, field: str) -> None:
        self.code = code
        self.field = field
        super().__init__(f"{code.value}: {field}")


@dataclass(frozen=True)
class SeptemberInputs:
    index: bytes
    notice: bytes
    headers: bytes
    month: str = "2026-09"


@dataclass(frozen=True)
class OriginalIdentity:
    role: str
    byte_length: int
    sha256: str


@dataclass(frozen=True)
class HttpMetadata:
    status: str
    content_type: str
    content_length: int
    date: str
    last_modified: str
    etag: str


_PINS = (
    OriginalIdentity(
        "index", 36196,
        "ca247c952dacfc08ee5d36240de8e678ceb2aa085b4ba8821dbe7267e6058da6",
    ),
    OriginalIdentity(
        "notice", 33725,
        "2242c4a5eeff6738c8cf77932a1589323817c06b3f7009c8b9b6c499a7221366",
    ),
    OriginalIdentity(
        "headers", 341,
        "2940014b82a5e4f9dad7857e7d1d2a92d8ca17ce691ee186b7b58239f08ca1ed",
    ),
)


@dataclass(frozen=True)
class SettlementFact:
    contract: str
    price_text: str
    unit: str


@dataclass(frozen=True)
class _NoticeFacts:
    """Parsed semantics only: deliberately not a source/admission candidate."""

    facts: tuple[SettlementFact, ...]
    delivery_date: str
    publication_date: str
    issue_number: str


@dataclass(frozen=True)
class SeptemberResearchEvidence:
    facts: tuple[SettlementFact, ...]
    delivery_date: str
    publication_date: str
    issue_number: str
    http_metadata: HttpMetadata
    originals: tuple[OriginalIdentity, ...] = field(default=_PINS, init=False)
    title: str = field(default=TITLE, init=False)
    publisher: str = field(default="Cffex", init=False)
    index_uri: str = field(default="http://www.cffex.com.cn/cn/jystz.html", init=False)
    notice_uri: str = field(default="http://www.cffex.com.cn" + NOTICE_PATH, init=False)
    index_date: str = field(default=SOURCE_DATE, init=False)
    publication_precision: str = field(default="Date", init=False)
    acquisition_scheme: str = field(default="http", init=False)
    acquisition_record: str = field(default="HistoricalWindowsPacket3", init=False)
    state: str = field(default="ResearchCandidate", init=False)
    admission: str = field(default="NotAdmitted", init=False)
    published_at: None = field(default=None, init=False)
    source_instant: None = field(default=None, init=False)
    local_observed_at: None = field(default=None, init=False)
    last_trading_date: None = field(default=None, init=False)
    source_timezone: str = field(default="NotProvided", init=False)
    method: str = field(default="NotProvided", init=False)
    official_revision: str = field(default="NotProvided", init=False)
    replaces: str = field(default="NotProvided", init=False)
    withdrawal: str = field(default="NotProvided", init=False)
    finality: str = field(default="Unknown", init=False)
    correction_coverage: str = field(default="Unknown", init=False)


@dataclass
class _Node:
    tag: str
    attrs: dict[str, str | None] = field(default_factory=dict)
    children: list[_Node | str] = field(default_factory=list)


class _Document(HTMLParser):
    _VOID = frozenset(
        "area base br col embed hr img input link meta param source track wbr".split()
    )

    def __init__(self, raw: bytes) -> None:
        super().__init__(convert_charrefs=True)
        self.root = _Node("root")
        self.stack = [self.root]
        try:
            self.feed(raw.decode("utf-8", errors="strict"))
            self.close()
        except (UnicodeDecodeError, ValueError) as error:
            raise EvidenceError(EvidenceErrorCode.DECODE_REJECTED, "html") from error

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        node = _Node(tag, dict(attrs))
        self.stack[-1].children.append(node)
        if tag not in self._VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        if tag not in self._VOID:
            self.handle_endtag(tag)

    def handle_endtag(self, tag: str) -> None:
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                del self.stack[index:]
                break

    def handle_data(self, data: str) -> None:
        self.stack[-1].children.append(data)


def _visible(node: _Node) -> bool:
    style = re.sub(r"\s+", "", (node.attrs.get("style") or "").lower())
    return not (
        node.tag in {"script", "style", "template", "noscript", "head"}
        or "hidden" in node.attrs
        or node.attrs.get("aria-hidden") == "true"
        or "display:none" in style
        or "visibility:hidden" in style
    )


def _nodes(node: _Node, class_name: str) -> list[_Node]:
    if not _visible(node):
        return []
    result = [node] if class_name in (node.attrs.get("class") or "").split() else []
    for child in node.children:
        if isinstance(child, _Node):
            result.extend(_nodes(child, class_name))
    return result


def _text(node: _Node) -> str:
    if not _visible(node):
        return ""
    if node.tag == "br":
        return "\n"
    text = "".join(_text(child) if isinstance(child, _Node) else child for child in node.children)
    return text + ("\n" if node.tag in {"p", "div", "li"} else "")


def _one(root: _Node, class_name: str) -> _Node:
    found = _nodes(root, class_name)
    if len(found) != 1:
        raise EvidenceError(EvidenceErrorCode.AMBIGUOUS_DOCUMENT, class_name)
    return found[0]


def _elements(node: _Node, tag: str) -> list[_Node]:
    if not _visible(node):
        return []
    result = [node] if node.tag == tag else []
    for child in node.children:
        if isinstance(child, _Node):
            result.extend(_elements(child, tag))
    return result


def _verify_index(raw: bytes) -> None:
    listing = _one(_Document(raw).root, "notice_list")
    matching_entries = 0
    for item in _elements(listing, "li"):
        anchors = _elements(item, "a")
        dates = [_text(date).strip() for date in _nodes(item, "time")]
        candidates = [anchor for anchor in anchors if (
            anchor.attrs.get("href") == NOTICE_PATH
            or (SOURCE_DATE in dates and (
                anchor.attrs.get("title") == TITLE or _text(anchor).strip() == TITLE
            ))
        )]
        if not candidates:
            continue
        if len(_elements(item, "li")) != 1:
            raise EvidenceError(EvidenceErrorCode.INDEX_IDENTITY_MISMATCH, "nested li date")
        matching_entries += 1
        if len(candidates) != 1:
            raise EvidenceError(EvidenceErrorCode.INDEX_IDENTITY_MISMATCH, "index notice")
        anchor = candidates[0]
        if (anchor.attrs.get("href") != NOTICE_PATH
                or anchor.attrs.get("title") != TITLE or _text(anchor).strip() != TITLE):
            raise EvidenceError(EvidenceErrorCode.INDEX_IDENTITY_MISMATCH, "index title/path")
        if dates != [SOURCE_DATE]:
            raise EvidenceError(EvidenceErrorCode.INDEX_IDENTITY_MISMATCH, "same li date")
    if matching_entries != 1:
        raise EvidenceError(EvidenceErrorCode.INDEX_IDENTITY_MISMATCH, "unique index entry")


def _parse_headers(raw: bytes, notice_length: int) -> HttpMetadata:
    try:
        text = raw.decode("ascii", errors="strict")
    except UnicodeDecodeError as error:
        raise EvidenceError(EvidenceErrorCode.DECODE_REJECTED, "headers") from error
    lines = text.split("\r\n")
    if not lines or lines[0] != "HTTP/1.1 200 OK" or lines[-2:] != ["", ""]:
        raise EvidenceError(EvidenceErrorCode.HTTP_METADATA_MISMATCH, "status/framing")
    fields: dict[str, str] = {}
    for line in lines[1:-2]:
        name, separator, value = line.partition(":")
        if not separator or not re.fullmatch(r"[A-Za-z0-9-]+", name) or name.lower() in fields:
            raise EvidenceError(EvidenceErrorCode.HTTP_METADATA_MISMATCH, "header uniqueness")
        fields[name.lower()] = value.strip()
    required = ("content-type", "content-length", "date", "last-modified", "etag")
    if any(not fields.get(name) for name in required):
        raise EvidenceError(EvidenceErrorCode.HTTP_METADATA_MISMATCH, "required headers")
    if fields["content-type"] != "text/html" or fields["content-length"] != str(notice_length):
        raise EvidenceError(EvidenceErrorCode.HTTP_METADATA_MISMATCH, "content identity")
    return HttpMetadata(lines[0], fields["content-type"], notice_length,
                        fields["date"], fields["last-modified"], fields["etag"])


def _parse_notice(raw: bytes) -> _NoticeFacts:
    root = _Document(raw).root
    article = _one(root, "jysggright")
    if _text(_one(article, "title_xqy")).strip() != TITLE:
        raise EvidenceError(EvidenceErrorCode.AMBIGUOUS_DOCUMENT, "title")
    publication_nodes = _nodes(article, "fxleft")
    if not publication_nodes:
        raise EvidenceError(EvidenceErrorCode.PUBLICATION_DATE_MISMATCH, "visible detail date")
    publication_date = _text(_one(article, "fxleft")).strip()
    if publication_date != SOURCE_DATE:
        raise EvidenceError(EvidenceErrorCode.PUBLICATION_DATE_MISMATCH, "detail date")
    body = _text(_one(article, "jysggnr"))
    lines = [line.strip() for line in body.splitlines() if line.strip()]
    if [line for line in lines if "中金所发" in line] != [ISSUE]:
        raise EvidenceError(EvidenceErrorCode.ISSUE_MISMATCH, "issue number")
    delivery_lines = [line for line in lines if "进行交割" in line]
    if delivery_lines != ["IF2609等合约于2026年9月18日进行交割，各合约的交割结算价具体如下："]:
        raise EvidenceError(EvidenceErrorCode.CONTRACT_SET_MISMATCH, "delivery date")
    facts: dict[str, SettlementFact] = {}
    pattern = re.compile(
        r"(?P<name>沪深300|上证50|中证500|中证1000)股指期货(?P<contract>[A-Z]{2}[0-9]+)合约"
        r"(?:和(?P<option_name>沪深300|上证50|中证1000)股指期权(?P<option>[A-Z]{2}[0-9]+)月份合约)?"
        r"的交割结算价为(?P<price>[0-9]+(?:\.[0-9]+)?)(?P<unit>[^；。]+)[；。]"
    )
    for line in lines:
        if "的交割结算价为" not in line:
            continue
        match = pattern.fullmatch(line)
        if match is None:
            raise EvidenceError(EvidenceErrorCode.PRICE_UNIT_MISMATCH, "pricing sentence")
        contract, price, unit = match.group("contract", "price", "unit")
        if contract not in CONTRACTS or match.group("name") != PRODUCT_NAMES[contract]:
            raise EvidenceError(EvidenceErrorCode.CONTRACT_SET_MISMATCH, "future contract")
        if contract in facts:
            raise EvidenceError(EvidenceErrorCode.CONFLICTING_FACT, "duplicate future")
        if match.group("option") is not None and (
            match.group("option") != OPTIONS.get(contract)
            or match.group("option_name") != PRODUCT_NAMES[contract]
        ):
            raise EvidenceError(EvidenceErrorCode.CONTRACT_SET_MISMATCH, "option relationship")
        if unit != "点" or not Decimal(price).is_finite() or Decimal(price) <= 0:
            raise EvidenceError(EvidenceErrorCode.PRICE_UNIT_MISMATCH, "price/unit")
        facts[contract] = SettlementFact(contract, price, unit)
    if set(facts) != set(CONTRACTS):
        raise EvidenceError(EvidenceErrorCode.CONTRACT_SET_MISMATCH, "four futures")
    if tuple(facts[key].price_text for key in CONTRACTS) != PRICES:
        raise EvidenceError(EvidenceErrorCode.PRICE_UNIT_MISMATCH, "saved source prices")
    return _NoticeFacts(
        tuple(facts[key] for key in CONTRACTS), SOURCE_DATE, publication_date, ISSUE,
    )


def verify_september_evidence(inputs: SeptemberInputs) -> SeptemberResearchEvidence:
    if inputs.month != "2026-09":
        raise EvidenceError(EvidenceErrorCode.UNSUPPORTED_MONTH, "month")
    for identity in _PINS:
        raw = getattr(inputs, identity.role)
        if not isinstance(raw, bytes):
            raise EvidenceError(EvidenceErrorCode.DECODE_REJECTED, identity.role)
        if len(raw) != identity.byte_length:
            raise EvidenceError(EvidenceErrorCode.INPUT_LENGTH_MISMATCH, identity.role)
        if sha256(raw).hexdigest() != identity.sha256:
            raise EvidenceError(EvidenceErrorCode.DIGEST_MISMATCH, identity.role)
    _verify_index(inputs.index)
    metadata = _parse_headers(inputs.headers, len(inputs.notice))
    parsed = _parse_notice(inputs.notice)
    return SeptemberResearchEvidence(parsed.facts, parsed.delivery_date,
                                     parsed.publication_date, parsed.issue_number, metadata)
