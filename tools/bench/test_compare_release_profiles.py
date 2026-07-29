import unittest

from tools.bench.compare_release_profiles import EXPECTED_WORKLOADS, evaluate


def evidence(
    default_times=(100, 100, 100, 100),
    candidate_times=(90, 90, 90, 90),
    default_checksum=7,
    candidate_checksum=7,
    default_size=1_000,
    candidate_size=1_000,
):
    workloads = tuple(EXPECTED_WORKLOADS)

    def profile(times, checksum, size):
        return {
            "binary_bytes": size,
            "runs": [
                {
                    "schema": 1,
                    "source": f"run-{run_index}.json",
                    "workloads": [
                        {
                            "workload": workload,
                            "iterations": EXPECTED_WORKLOADS[workload],
                            "elapsed_ns": elapsed,
                            "throughput_per_second": (
                                EXPECTED_WORKLOADS[workload] * 1_000_000_000 / elapsed
                            ),
                            "checksum": checksum + index,
                        }
                        for index, (workload, elapsed) in enumerate(
                            zip(workloads, times, strict=True)
                        )
                    ]
                }
                for run_index in range(1, 6)
            ],
        }

    default = profile(default_times, default_checksum, default_size)
    candidate = profile(candidate_times, candidate_checksum, candidate_size)
    return {
        "schema": 1,
        "metadata": {
            "revision": "1" * 40,
            "rustc": "rustc 1.95.0 (59807616e 2026-04-14)",
            "cargo": "cargo 1.95.0 (f2d3ce0bd 2026-03-21)",
            "platform": "test-platform",
            "runner": "one warm-up per profile; five alternating measured rounds",
            "default": "lto=false, codegen-units=16",
            "candidate": 'lto="thin", codegen-units=1',
        },
        "profiles": {"default": default, "candidate": candidate},
    }


class ReleaseProfilePolicyTests(unittest.TestCase):
    def test_candidate_passes_all_thresholds_with_identical_checksums(self):
        report = evaluate(evidence())
        self.assertTrue(report["qualified"])
        self.assertGreaterEqual(report["combined_improvement"], 0.05)

    def test_checksum_mismatch_fails_closed(self):
        report = evaluate(evidence(candidate_checksum=8))
        self.assertFalse(report["qualified"])
        self.assertIn("checksum", " ".join(report["reasons"]))

    def test_combined_improvement_must_reach_five_percent(self):
        report = evaluate(evidence(candidate_times=(96, 96, 96, 96)))
        self.assertFalse(report["qualified"])
        self.assertIn("combined improvement", " ".join(report["reasons"]))

    def test_one_workload_cannot_regress_more_than_five_percent(self):
        report = evaluate(
            evidence(
                default_times=(100, 100, 100, 100),
                candidate_times=(70, 70, 106, 70),
            )
        )
        self.assertFalse(report["qualified"])
        self.assertIn("zlib_decompress regression", " ".join(report["reasons"]))

    def test_binary_growth_cannot_exceed_twenty_percent(self):
        self.assertTrue(
            evaluate(evidence(default_size=1_000, candidate_size=1_200))["qualified"]
        )
        report = evaluate(evidence(default_size=1_000, candidate_size=1_201))
        self.assertFalse(report["qualified"])
        self.assertIn("binary growth", " ".join(report["reasons"]))

    def test_missing_or_inconsistent_runs_fail_closed(self):
        sample = evidence()
        sample["profiles"]["candidate"]["runs"].pop()
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("exactly five", " ".join(report["reasons"]))

        sample = evidence()
        sample["profiles"]["candidate"]["runs"][0]["workloads"][0]["iterations"] = 9
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("iterations", " ".join(report["reasons"]))

    def test_missing_workload_metadata_and_throughput_fail_closed(self):
        sample = evidence()
        sample["profiles"]["candidate"]["runs"][0]["workloads"].pop()
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("workload set is incomplete", " ".join(report["reasons"]))

        sample = evidence()
        del sample["metadata"]["revision"]
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("metadata fields", " ".join(report["reasons"]))

        sample = evidence()
        del sample["profiles"]["default"]["runs"][0]["workloads"][0][
            "throughput_per_second"
        ]
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("throughput_per_second", " ".join(report["reasons"]))

    def test_run_sources_and_exact_iteration_counts_fail_closed(self):
        sample = evidence()
        sample["profiles"]["default"]["runs"][1]["source"] = "run-1.json"
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("source must equal", " ".join(report["reasons"]))

        sample = evidence()
        workload = sample["profiles"]["candidate"]["runs"][0]["workloads"][0]
        workload["iterations"] += 1
        workload["throughput_per_second"] = (
            workload["iterations"] * 1_000_000_000 / workload["elapsed_ns"]
        )
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("iterations must equal", " ".join(report["reasons"]))

    def test_boolean_float_schema_and_forged_tool_versions_fail_closed(self):
        for invalid_schema in (True, 1.0):
            sample = evidence()
            sample["schema"] = invalid_schema
            report = evaluate(sample)
            self.assertFalse(report["qualified"])
            self.assertIn("schema must be integer", " ".join(report["reasons"]))

        sample = evidence()
        sample["profiles"]["default"]["runs"][0]["schema"] = True
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("run 0 schema must be integer", " ".join(report["reasons"]))

        for field, forged in (("rustc", "rustc forged"), ("cargo", "cargo forged")):
            sample = evidence()
            sample["metadata"][field] = forged
            report = evaluate(sample)
            self.assertFalse(report["qualified"])
            self.assertIn(
                f"metadata {field} must contain the exact tool version",
                " ".join(report["reasons"]),
            )

    def test_default_profile_metadata_is_required_and_exact(self):
        sample = evidence()
        del sample["metadata"]["default"]
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("metadata fields", " ".join(report["reasons"]))

        sample = evidence()
        sample["metadata"]["default"] = "forged"
        report = evaluate(sample)
        self.assertFalse(report["qualified"])
        self.assertIn("metadata default", " ".join(report["reasons"]))


if __name__ == "__main__":
    unittest.main()
