from __future__ import annotations

import copy
import unittest

from check_tdx_native_boundary import validate_boundary_data


NATIVE = "magic-tdx-native-bridge"
DISCOVERY = f"crates/{NATIVE}/src/discovery.rs"


def valid_snapshot() -> dict:
    safe_roots = {
        "crates/magic-market-core/src/lib.rs": "#![forbid(unsafe_code)]\n",
        "crates/magic-market-monitor/src/lib.rs": "#![forbid(unsafe_code)]\n",
        "crates/magic-tdx-local-rs/src/lib.rs": "#![forbid(unsafe_code)]\n",
        "crates/magic-market-monitor-server/src/main.rs": "#![forbid(unsafe_code)]\n",
    }
    native_sources = {
        f"crates/{NATIVE}/src/main.rs": (
            "#![deny(unsafe_code)]\n#![deny(unsafe_op_in_unsafe_fn)]\nfn main() {}\n"
        ),
        DISCOVERY: (
            "#![cfg_attr(windows, allow(unsafe_code))]\n"
            "#[cfg(windows)] fn probe(p: *const u8) { let _ = unsafe { *p }; }\n"
        ),
    }
    manifest = {
        "package": {"name": NATIVE, "publish": False},
        "bin": [{"name": NATIVE, "path": "src/main.rs"}],
        "dependencies": {"serde": "1"},
        "lints": {
            "rust": {
                "unsafe_code": "deny",
                "unsafe_op_in_unsafe_fn": "deny",
            }
        },
    }
    return {
        "native_manifest": manifest,
        "native_main": native_sources[f"crates/{NATIVE}/src/main.rs"],
        "native_has_lib": False,
        "rust_sources": {**native_sources, **safe_roots},
        "safe_roots": safe_roots,
        "native_sources": native_sources,
        "reverse_manifests": {
            name: {"package": {"name": name}}
            for name in (
                "magic-market-core",
                "magic-market-monitor",
                "magic-tdx-local-rs",
                "magic-market-monitor-server",
            )
        },
    }


def validate(snapshot: dict) -> list[str]:
    return validate_boundary_data(**snapshot)


class TdxNativeBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.snapshot = valid_snapshot()

    def assert_error_contains(self, fragment: str) -> None:
        errors = validate(self.snapshot)
        self.assertTrue(
            any(fragment in error for error in errors),
            f"expected {fragment!r} in {errors!r}",
        )

    def test_valid_fail_closed_boundary_passes(self) -> None:
        self.assertEqual(validate(self.snapshot), [])

    def test_unsafe_outside_discovery_fails(self) -> None:
        self.snapshot["rust_sources"]["crates/magic-tdx-local-rs/src/extra.rs"] = (
            "fn bad(p: *const u8) { let _ = unsafe { *p }; }\n"
        )
        self.assert_error_contains("unsafe Rust is outside")

    def test_native_manifest_must_be_bin_only_nonpublishable_and_deny_unsafe(self) -> None:
        self.snapshot["native_has_lib"] = True
        self.snapshot["native_manifest"]["package"]["publish"] = True
        self.snapshot["native_manifest"]["lints"]["rust"]["unsafe_code"] = "allow"
        errors = validate(self.snapshot)
        self.assertTrue(any("publish = false" in error for error in errors))
        self.assertTrue(any("bin-only" in error for error in errors))
        self.assertTrue(any("unsafe_code" in error for error in errors))

    def test_legacy_compatibility_source_is_prohibited(self) -> None:
        self.snapshot["native_sources"][
            f"crates/{NATIVE}/src/compatibility.rs"
        ] = (
            "struct CompatibilityProfile;\n"
            "const APPROVED_PROFILES: &[CompatibilityProfile] = &[];\n"
        )
        self.assert_error_contains("unapproved source file")
        self.assert_error_contains("legacy native compatibility artifact")

    def test_account_or_trading_native_name_fails(self) -> None:
        prohibited = "Get" + "Order" + "Str"
        self.snapshot["native_sources"][f"crates/{NATIVE}/src/account.rs"] = (
            f'const PROHIBITED: &str = "{prohibited}";\n'
        )
        self.assert_error_contains("forbidden account/trading")

    def test_native_module_loading_stays_blocked(self) -> None:
        path = f"crates/{NATIVE}/src/main.rs"
        self.snapshot["native_sources"][path] += "fn load() { LoadLibraryW(); }\n"
        self.assert_error_contains("native module loading is prohibited")

    def test_legacy_vendor_module_name_stays_blocked(self) -> None:
        path = f"crates/{NATIVE}/src/main.rs"
        self.snapshot["native_sources"][path] += (
            'const LEGACY: &str = "TPythClient.dll";\n'
        )
        self.assert_error_contains("legacy native compatibility artifact")

    def test_network_dependency_and_source_are_prohibited(self) -> None:
        self.snapshot["native_manifest"]["dependencies"]["ureq"] = "2"
        path = f"crates/{NATIVE}/src/main.rs"
        self.snapshot["native_sources"][path] += (
            'const ENDPOINT: &str = "http://127.0.0.1";\n'
        )
        errors = validate(self.snapshot)
        self.assertTrue(any("dependency is not allowed" in error for error in errors))
        self.assertTrue(any("network access is prohibited" in error for error in errors))

    def test_localized_version_string_lookup_is_prohibited(self) -> None:
        path = f"crates/{NATIVE}/src/discovery.rs"
        self.snapshot["native_sources"][path] += (
            'const LOCALIZED: &str = "\\\\StringFileInfo\\\\040904b0\\\\FileVersion";\n'
        )
        self.assert_error_contains("localized executable version lookup is prohibited")

    def test_native_path_dependency_and_reverse_dependency_fail(self) -> None:
        self.snapshot["native_manifest"]["dependencies"]["magic-market-core"] = {
            "path": "../magic-market-core"
        }
        self.snapshot["reverse_manifests"]["magic-tdx-local-rs"] = {
            "dependencies": {NATIVE: {"path": f"../{NATIVE}"}}
        }
        errors = validate(self.snapshot)
        self.assertTrue(any("must not use path dependencies" in error for error in errors))
        self.assertTrue(any("must not depend on the native bridge" in error for error in errors))

    def test_unsafe_words_in_comments_strings_and_identifiers_do_not_false_positive(self) -> None:
        self.snapshot["rust_sources"]["crates/magic-tdx-local-rs/src/text.rs"] = (
            '// unsafe { comment_only() }\n'
            'const TEXT: &str = "unsafe { string_only() }";\n'
            'const RAW: &str = r#"unsafe { raw_string_only() }"#;\n'
            "fn unsafe_name_is_an_identifier() {}\n"
        )
        self.assertEqual(validate(self.snapshot), [])

    def test_snapshot_inputs_are_independent_for_mutation(self) -> None:
        other = copy.deepcopy(self.snapshot)
        other["native_manifest"]["package"]["publish"] = True
        self.assertEqual(validate(self.snapshot), [])
        self.assertNotEqual(validate(other), [])


if __name__ == "__main__":
    unittest.main()
