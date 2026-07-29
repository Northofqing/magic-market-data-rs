import unittest

from tools.bench.compare_release_profiles import evaluate


def evidence(
    default_times=(100, 100, 100),
    candidate_times=(90, 90, 90),
    default_checksum=7,
    candidate_checksum=7,
    default_size=1_000,
    candidate_size=1_000,
):
    workloads = ("tdx_bar_parse", "json_normalize", "zlib_decompress")

    def profile(times, checksum, size):
        return {
            "binary_bytes": size,
            "runs": [
                {
                    "workloads": [
                        {
                            "workload": workload,
                            "iterations": 10,
                            "elapsed_ns": elapsed,
                            "checksum": checksum + index,
                        }
                        for index, (workload, elapsed) in enumerate(
                            zip(workloads, times, strict=True)
                        )
                    ]
                }
                for _ in range(5)
            ],
        }

    default = profile(default_times, default_checksum, default_size)
    candidate = profile(candidate_times, candidate_checksum, candidate_size)
    return {"schema": 1, "profiles": {"default": default, "candidate": candidate}}


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
        report = evaluate(evidence(candidate_times=(96, 96, 96)))
        self.assertFalse(report["qualified"])
        self.assertIn("combined improvement", " ".join(report["reasons"]))

    def test_one_workload_cannot_regress_more_than_five_percent(self):
        report = evaluate(
            evidence(default_times=(100, 100, 100), candidate_times=(70, 70, 106))
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


if __name__ == "__main__":
    unittest.main()
