"""Offline controls for the fixed September CFFEX research sample."""

from __future__ import annotations

import os
import unittest
from dataclasses import replace
from pathlib import Path

import cffex_september_evidence as research

from cffex_september_evidence import (
    EvidenceError,
    EvidenceErrorCode,
    SeptemberInputs,
    verify_september_evidence,
)


class FixedSampleTests(unittest.TestCase):
    @staticmethod
    def saved_inputs() -> SeptemberInputs:
        root = Path(os.environ.get(
            "CFFEX_EVIDENCE_DIR",
            str(Path(__file__).resolve().parents[2]
                / "target/cffex-confirmation-controls-20261002"),
        ))
        # Required original inputs: missing files are errors, never skipped PASS.
        return SeptemberInputs(
            (root / "jystz-index.html").read_bytes(),
            (root / "20260918-48624.html").read_bytes(),
            (root / "20260918-48624.headers.txt").read_bytes(),
        )
    def test_saved_sample_has_four_exact_prices_and_date_precision(self) -> None:
        evidence = verify_september_evidence(self.saved_inputs())
        self.assertEqual(
            [(fact.contract, fact.price_text, fact.unit) for fact in evidence.facts],
            [("IF2609", "4508.38", "点"), ("IH2609", "2862.60", "点"),
             ("IC2609", "7789.72", "点"), ("IM2609", "7679.21", "点")],
        )
        self.assertEqual(evidence.delivery_date, "2026-09-18")
        self.assertEqual(evidence.publication_date, "2026-09-18")
        self.assertEqual(evidence.publication_precision, "Date")

    def test_same_length_price_change_is_digest_rejection(self) -> None:
        saved = self.saved_inputs()
        changed = replace(saved, notice=saved.notice.replace(b"4508.38", b"4508.39"))
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(changed)
        self.assertIs(caught.exception.code, EvidenceErrorCode.DIGEST_MISMATCH)
        self.assertEqual(caught.exception.field, "notice")

    def test_synthetic_index_cannot_borrow_next_entry_date(self) -> None:
        # Pre-agreed internal semantic seam; no source candidate is constructed.
        synthetic = ('<div class="notice_list"><ul><li>'
                     '<a href="/cn/jystz/20260918/48624.html" '
                     'title="关于股指期货和股指期权合约交割的通知">'
                     '关于股指期货和股指期权合约交割的通知</a></li>'
                     '<li><a class="time">2026-09-18</a></li></ul></div>').encode()
        with self.assertRaises(EvidenceError) as caught:
            research._verify_index(synthetic)
        self.assertIs(caught.exception.code, EvidenceErrorCode.INDEX_IDENTITY_MISMATCH)

    def test_synthetic_wrong_source_price_is_not_accepted(self) -> None:
        raw = self.saved_inputs().notice.replace(b"4508.38", b"4508.39")
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(raw)
        self.assertIs(caught.exception.code, EvidenceErrorCode.PRICE_UNIT_MISMATCH)

    def test_synthetic_content_length_must_bind_detail_bytes(self) -> None:
        changed = self.saved_inputs().headers.replace(b"33725", b"33724")
        with self.assertRaises(EvidenceError) as caught:
            research._parse_headers(changed, 33725)
        self.assertIs(caught.exception.code, EvidenceErrorCode.HTTP_METADATA_MISMATCH)

    def test_http_date_and_etag_do_not_mint_publication_or_revision(self) -> None:
        evidence = verify_september_evidence(self.saved_inputs())
        self.assertEqual(evidence.state, "ResearchCandidate")
        self.assertEqual(evidence.admission, "NotAdmitted")
        self.assertEqual(evidence.acquisition_scheme, "http")
        self.assertEqual(evidence.issue_number, "中金所发〔2026〕47号")
        self.assertEqual(evidence.http_metadata.date, "Thu, 01 Oct 2026 18:12:57 GMT")
        self.assertEqual(evidence.http_metadata.last_modified, "Fri, 18 Sep 2026 08:09:06 GMT")
        self.assertEqual(evidence.http_metadata.etag, '"83bd-65bbd6a062c80"')
        self.assertIsNone(evidence.published_at)
        self.assertIsNone(evidence.source_instant)
        self.assertIsNone(evidence.local_observed_at)
        self.assertIsNone(evidence.last_trading_date)
        self.assertEqual(evidence.source_timezone, "NotProvided")
        self.assertEqual(evidence.method, "NotProvided")
        self.assertEqual(evidence.official_revision, "NotProvided")
        self.assertEqual(evidence.replaces, "NotProvided")
        self.assertEqual(evidence.withdrawal, "NotProvided")
        self.assertEqual(evidence.finality, "Unknown")
        self.assertEqual(evidence.correction_coverage, "Unknown")

    def test_synthetic_missing_visible_date_is_publication_error(self) -> None:
        changed = self.saved_inputs().notice.replace(b'class="fxleft"', b'class="gone"')
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(changed)
        self.assertIs(caught.exception.code, EvidenceErrorCode.PUBLICATION_DATE_MISMATCH)

    def test_synthetic_second_issue_is_not_silently_ignored(self) -> None:
        saved = self.saved_inputs().notice
        changed = saved.replace('中金所发〔2026〕47号'.encode(),
                                '中金所发〔2026〕47号</p><p>中金所发〔2026〕48号'.encode())
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(changed)
        self.assertIs(caught.exception.code, EvidenceErrorCode.ISSUE_MISMATCH)

    def assert_notice_change_rejected(self, old: str, new: str,
                                     code: EvidenceErrorCode) -> None:
        raw = self.saved_inputs().notice
        self.assertIn(old.encode(), raw)
        synthetic = raw.replace(old.encode(), new.encode())
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(synthetic)
        self.assertIs(caught.exception.code, code)

    def test_public_role_swapping_is_not_identity_binding(self) -> None:
        saved = self.saved_inputs()
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(replace(saved, index=saved.notice, notice=saved.index))
        self.assertIs(caught.exception.code, EvidenceErrorCode.INPUT_LENGTH_MISMATCH)
        self.assertEqual(caught.exception.field, "index")

    def test_public_missing_body_is_not_verified_empty(self) -> None:
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(replace(self.saved_inputs(), notice=b""))
        self.assertIs(caught.exception.code, EvidenceErrorCode.INPUT_LENGTH_MISMATCH)

    def test_public_index_and_headers_are_individually_pinned(self) -> None:
        saved = self.saved_inputs()
        for role, changed in (
            ("index", saved.index.replace(b"48624", b"48625")),
            ("headers", saved.headers.replace(b"33725", b"33724")),
        ):
            with self.subTest(role=role), self.assertRaises(EvidenceError) as caught:
                verify_september_evidence(replace(saved, **{role: changed}))
            self.assertIs(caught.exception.code, EvidenceErrorCode.DIGEST_MISMATCH)
            self.assertEqual(caught.exception.field, role)

    def test_public_mutable_bytes_are_rejected(self) -> None:
        saved = self.saved_inputs()
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(replace(saved, notice=bytearray(saved.notice)))
        self.assertIs(caught.exception.code, EvidenceErrorCode.DECODE_REJECTED)

    def test_candidate_preserves_three_original_identities_and_http_uris(self) -> None:
        evidence = verify_september_evidence(self.saved_inputs())
        self.assertEqual(
            [(original.role, original.byte_length, original.sha256)
             for original in evidence.originals],
            [("index", 36196, "ca247c952dacfc08ee5d36240de8e678ceb2aa085b4ba8821dbe7267e6058da6"),
             ("notice", 33725, "2242c4a5eeff6738c8cf77932a1589323817c06b3f7009c8b9b6c499a7221366"),
             ("headers", 341, "2940014b82a5e4f9dad7857e7d1d2a92d8ca17ce691ee186b7b58239f08ca1ed")],
        )
        self.assertEqual(evidence.index_uri, "http://www.cffex.com.cn/cn/jystz.html")
        self.assertEqual(
            evidence.notice_uri, "http://www.cffex.com.cn/cn/jystz/20260918/48624.html",
        )
        self.assertEqual(evidence.index_date, "2026-09-18")
        self.assertEqual(evidence.publisher, "Cffex")
        self.assertEqual(evidence.http_metadata.content_length, 33725)

    def test_synthetic_one_missing_future_rejects_whole_batch(self) -> None:
        self.assert_notice_change_rejected(
            "中证500股指期货IC2609合约的交割结算价为7789.72点；", "",
            EvidenceErrorCode.CONTRACT_SET_MISMATCH,
        )

    def test_synthetic_option_cannot_replace_future(self) -> None:
        self.assert_notice_change_rejected("股指期货IC2609", "股指期权MO2609",
                                           EvidenceErrorCode.PRICE_UNIT_MISMATCH)

    def test_synthetic_near_match_contract_is_not_exact_contract(self) -> None:
        self.assert_notice_change_rejected("股指期货IC2609", "股指期货IC26090",
                                           EvidenceErrorCode.CONTRACT_SET_MISMATCH)

    def test_synthetic_duplicate_future_is_rejected_even_with_same_price(self) -> None:
        line = "中证500股指期货IC2609合约的交割结算价为7789.72点；"
        self.assert_notice_change_rejected(line, line + "<br/>" + line,
                                           EvidenceErrorCode.CONFLICTING_FACT)

    def test_synthetic_conflicting_future_is_not_first_match_success(self) -> None:
        line = "中证500股指期货IC2609合约的交割结算价为7789.72点；"
        self.assert_notice_change_rejected(line, line + "<br/>" + line.replace(".72", ".73"),
                                           EvidenceErrorCode.CONFLICTING_FACT)

    def test_synthetic_currency_unit_cannot_become_points(self) -> None:
        self.assert_notice_change_rejected("7789.72点", "7789.72元",
                                           EvidenceErrorCode.PRICE_UNIT_MISMATCH)

    def test_synthetic_price_text_keeps_its_trailing_zero(self) -> None:
        self.assert_notice_change_rejected("2862.60点", "2862.6点",
                                           EvidenceErrorCode.PRICE_UNIT_MISMATCH)

    def test_synthetic_wrong_option_month_breaks_sentence_relationship(self) -> None:
        self.assert_notice_change_rejected("股指期权IO2609", "股指期权IO2610",
                                           EvidenceErrorCode.CONTRACT_SET_MISMATCH)

    def test_synthetic_wrong_product_name_cannot_borrow_other_price(self) -> None:
        self.assert_notice_change_rejected("中证500股指期货IC2609", "中证1000股指期货IC2609",
                                           EvidenceErrorCode.CONTRACT_SET_MISMATCH)

    def test_synthetic_wrong_delivery_day_is_not_inferred(self) -> None:
        self.assert_notice_change_rejected("于2026年9月18日进行交割", "于2026年9月19日进行交割",
                                           EvidenceErrorCode.CONTRACT_SET_MISMATCH)

    def test_synthetic_wrong_issue_number_is_rejected(self) -> None:
        self.assert_notice_change_rejected("中金所发〔2026〕47号", "中金所发〔2026〕48号",
                                           EvidenceErrorCode.ISSUE_MISMATCH)

    def test_synthetic_wrong_visible_publication_date_is_rejected(self) -> None:
        self.assert_notice_change_rejected("2026-09-18", "2026-09-19",
                                           EvidenceErrorCode.PUBLICATION_DATE_MISMATCH)

    def test_synthetic_added_midnight_is_not_original_date_precision(self) -> None:
        self.assert_notice_change_rejected("2026-09-18", "2026-09-18T00:00:00+08:00",
                                           EvidenceErrorCode.PUBLICATION_DATE_MISMATCH)

    def test_synthetic_hidden_publication_label_is_not_visible_original(self) -> None:
        self.assert_notice_change_rejected('class="fxleft"', 'class="fxleft" hidden',
                                           EvidenceErrorCode.PUBLICATION_DATE_MISMATCH)

    def test_synthetic_duplicate_visible_publication_labels_are_ambiguous(self) -> None:
        self.assert_notice_change_rejected(
            '<div class="fxleft"><a>2026-09-18</a></div>',
            '<div class="fxleft"><a>2026-09-18</a></div>'
            '<div class="fxleft"><a>2026-09-18</a></div>',
            EvidenceErrorCode.AMBIGUOUS_DOCUMENT,
        )

    def test_synthetic_script_price_is_not_a_visible_future_fact(self) -> None:
        line = "中证500股指期货IC2609合约的交割结算价为7789.72点；"
        self.assert_notice_change_rejected(line, "<script>" + line + "</script>",
                                           EvidenceErrorCode.CONTRACT_SET_MISMATCH)

    def test_synthetic_utf8_failure_is_typed(self) -> None:
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(b"\xff")
        self.assertIs(caught.exception.code, EvidenceErrorCode.DECODE_REJECTED)

    def test_synthetic_semantics_cannot_mint_a_public_candidate(self) -> None:
        saved = self.saved_inputs()
        synthetic = saved.notice.replace(b"<div class=nan>", b'<div class="nan">')
        semantics = research._parse_notice(synthetic)
        self.assertNotIsInstance(semantics, research.SeptemberResearchEvidence)
        self.assertFalse(hasattr(semantics, "admission"))
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(replace(saved, notice=synthetic))
        self.assertIs(caught.exception.code, EvidenceErrorCode.INPUT_LENGTH_MISMATCH)

    def test_synthetic_index_wrong_title_path_date_and_duplicate_are_rejected(self) -> None:
        saved = self.saved_inputs().index
        target = ('<li><a href="/cn/jystz/20260918/48624.html" '
                  'title="关于股指期货和股指期权合约交割的通知">'
                  '关于股指期货和股指期权合约交割的通知</a>'
                  '<a class="time">2026-09-18</a></li>')
        variants = (
            saved.replace(b"48624.html", b"48625.html"),
            saved.replace('title="关于股指期货和股指期权合约交割的通知"'.encode(),
                          'title="不同通知"'.encode()),
            saved.replace(b"2026-09-18", b"2026-09-19"),
            ('<div class="notice_list">' + target + target + '</div>').encode(),
        )
        for number, synthetic in enumerate(variants):
            with self.subTest(number=number), self.assertRaises(EvidenceError) as caught:
                research._verify_index(synthetic)
            self.assertIs(caught.exception.code, EvidenceErrorCode.INDEX_IDENTITY_MISMATCH)

    def test_synthetic_http_status_type_duplicate_and_missing_metadata_are_rejected(self) -> None:
        headers = self.saved_inputs().headers
        variants = (
            headers.replace(b"200 OK", b"404 Not Found"),
            headers.replace(b"text/html", b"application/json"),
            headers.replace(
                b"Content-Length: 33725",
                b"Content-Length: 33725\r\nContent-Length: 33725",
            ),
            headers.replace(b'ETag: "83bd-65bbd6a062c80"\r\n', b""),
            headers.replace(b"\r\n", b"\n"),
        )
        for number, synthetic in enumerate(variants):
            with self.subTest(number=number), self.assertRaises(EvidenceError) as caught:
                research._parse_headers(synthetic, 33725)
            self.assertIs(caught.exception.code, EvidenceErrorCode.HTTP_METADATA_MISMATCH)

    def test_synthetic_nested_li_cannot_supply_the_target_date(self) -> None:
        synthetic = ('<div class="notice_list"><ul><li>'
                     '<a href="/cn/jystz/20260918/48624.html" '
                     'title="关于股指期货和股指期权合约交割的通知">'
                     '关于股指期货和股指期权合约交割的通知</a>'
                     '<li><a class="time">2026-09-18</a></li>'
                     '</li></ul></div>').encode()
        with self.assertRaises(EvidenceError) as caught:
            research._verify_index(synthetic)
        self.assertIs(caught.exception.code, EvidenceErrorCode.INDEX_IDENTITY_MISMATCH)

    def test_synthetic_duplicate_style_attributes_are_ambiguous(self) -> None:
        self.assert_notice_change_rejected(
            'class="fxleft"', 'class="fxleft" style="display:none" style="display:block"',
            EvidenceErrorCode.AMBIGUOUS_DOCUMENT,
        )

    def test_synthetic_body_label_cannot_replace_publication_metadata(self) -> None:
        saved = self.saved_inputs().notice
        label = b'<div class="fxleft"><a>2026-09-18</a></div>'
        self.assertIn(label, saved)
        synthetic = saved.replace(label, b"").replace(b'<div class="jysggnr">',
                                                      b'<div class="jysggnr">' + label)
        with self.assertRaises(EvidenceError) as caught:
            research._parse_notice(synthetic)
        self.assertIs(caught.exception.code, EvidenceErrorCode.PUBLICATION_DATE_MISMATCH)

    def test_synthetic_split_conflicting_price_sentence_is_not_ignored(self) -> None:
        line = "中证500股指期货IC2609合约的交割结算价为7789.72点；"
        extra = "中证500股指期货IC2609合约的交割结算价<br/>为7789.73点；"
        self.assert_notice_change_rejected(line, line + "<br/>" + extra,
                                           EvidenceErrorCode.PRICE_UNIT_MISMATCH)

    def test_other_month_is_typed_rejection_not_september_confirmation(self) -> None:
        inputs = SeptemberInputs(b"", b"", b"", month="2026-10")
        with self.assertRaises(EvidenceError) as caught:
            verify_september_evidence(inputs)
        self.assertIs(caught.exception.code, EvidenceErrorCode.UNSUPPORTED_MONTH)


if __name__ == "__main__":
    unittest.main()
