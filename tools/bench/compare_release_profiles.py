#!/usr/bin/env python3
"""Evaluate default and candidate Cargo release profiles against fixed policy."""

import argparse
import json
import math
import platform
import re
import statistics
from pathlib import Path

MEASURED_RUNS = 5
MINIMUM_COMBINED_IMPROVEMENT = 0.05
MAXIMUM_WORKLOAD_REGRESSION = 0.05
MAXIMUM_BINARY_GROWTH = 0.20
ROUNDING_EPSILON = 1e-12
RUNNER_DESCRIPTION = "one warm-up per profile; five alternating measured rounds"
CANDIDATE_DESCRIPTION = 'lto="thin", codegen-units=1'
EXPECTED_WORKLOADS = {
    "tdx_bar_parse": 20_000,
    "json_normalize": 10_000,
    "zlib_decompress": 5_000,
    "zlib_roundtrip": 2_000,
}


def _fail(reason):
    return {
        "qualified": False,
        "reasons": [reason],
        "thresholds": _thresholds(),
        "workloads": {},
    }


def _thresholds():
    return {
        "measured_runs": MEASURED_RUNS,
        "minimum_combined_improvement": MINIMUM_COMBINED_IMPROVEMENT,
        "maximum_workload_regression": MAXIMUM_WORKLOAD_REGRESSION,
        "maximum_binary_growth": MAXIMUM_BINARY_GROWTH,
    }


def _profile_records(profile_name, profile):
    if not isinstance(profile, dict):
        raise ValueError(f"{profile_name} profile is not an object")
    size = profile.get("binary_bytes")
    if not isinstance(size, int) or isinstance(size, bool) or size <= 0:
        raise ValueError(f"{profile_name} binary_bytes must be a positive integer")
    runs = profile.get("runs")
    if not isinstance(runs, list) or len(runs) != MEASURED_RUNS:
        raise ValueError(f"{profile_name} must contain exactly five measured runs")

    records = {}
    run_sources = set()
    for run_index, run in enumerate(runs):
        if not isinstance(run, dict):
            raise ValueError(f"{profile_name} run {run_index} is not an object")
        source = run.get("source")
        expected_source = f"run-{run_index + 1}.json"
        if source != expected_source or source in run_sources:
            raise ValueError(
                f"{profile_name} run {run_index} source must equal {expected_source}"
            )
        run_sources.add(source)
        workloads = run.get("workloads") if isinstance(run, dict) else None
        if not isinstance(workloads, list) or not workloads:
            raise ValueError(
                f"{profile_name} run {run_index} must contain non-empty workloads"
            )
        seen = set()
        for record in workloads:
            if not isinstance(record, dict):
                raise ValueError(f"{profile_name} run {run_index} has a non-object record")
            workload = record.get("workload")
            iterations = record.get("iterations")
            elapsed_ns = record.get("elapsed_ns")
            throughput = record.get("throughput_per_second")
            checksum = record.get("checksum")
            if not isinstance(workload, str) or not workload:
                raise ValueError(f"{profile_name} run {run_index} has an invalid workload")
            if workload in seen:
                raise ValueError(
                    f"{profile_name} run {run_index} duplicates workload {workload}"
                )
            if (
                not isinstance(iterations, int)
                or isinstance(iterations, bool)
                or iterations <= 0
            ):
                raise ValueError(f"{profile_name} {workload} iterations must be positive")
            expected_iterations = EXPECTED_WORKLOADS.get(workload)
            if expected_iterations is None:
                raise ValueError(
                    f"{profile_name} run {run_index} contains unknown workload {workload}"
                )
            if iterations != expected_iterations:
                raise ValueError(
                    f"{profile_name} {workload} iterations must equal "
                    f"{expected_iterations}"
                )
            if (
                not isinstance(elapsed_ns, int)
                or isinstance(elapsed_ns, bool)
                or elapsed_ns <= 0
            ):
                raise ValueError(f"{profile_name} {workload} elapsed_ns must be positive")
            if not isinstance(checksum, int) or isinstance(checksum, bool):
                raise ValueError(f"{profile_name} {workload} checksum must be an integer")
            if (
                not isinstance(throughput, (int, float))
                or isinstance(throughput, bool)
                or not math.isfinite(throughput)
                or throughput <= 0
            ):
                raise ValueError(
                    f"{profile_name} {workload} throughput_per_second must be finite and positive"
                )
            expected_throughput = iterations * 1_000_000_000 / elapsed_ns
            if not math.isclose(
                throughput, expected_throughput, rel_tol=1e-9, abs_tol=1e-12
            ):
                raise ValueError(
                    f"{profile_name} {workload} throughput_per_second is inconsistent"
                )
            seen.add(workload)
            records.setdefault(workload, []).append(
                {
                    "iterations": iterations,
                    "elapsed_ns": elapsed_ns,
                    "checksum": checksum,
                    "throughput_per_second": throughput,
                }
            )
        if seen != set(EXPECTED_WORKLOADS):
            missing = sorted(set(EXPECTED_WORKLOADS) - seen)
            raise ValueError(
                f"{profile_name} run {run_index} workload set is incomplete; "
                f"missing {missing}"
            )

    for workload, values in records.items():
        if len(values) != MEASURED_RUNS:
            raise ValueError(
                f"{profile_name} workload {workload} must appear in every measured run"
            )
    return size, records


def _validate_metadata(metadata):
    if not isinstance(metadata, dict):
        raise ValueError("metadata object is missing")
    required = {"revision", "rustc", "cargo", "platform", "runner", "candidate"}
    if set(metadata) != required:
        raise ValueError("metadata fields must exactly match the benchmark schema")
    revision = metadata["revision"]
    if not isinstance(revision, str) or re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise ValueError("metadata revision must be a full lowercase Git SHA-1")
    for field, prefix in (("rustc", "rustc "), ("cargo", "cargo ")):
        value = metadata[field]
        if not isinstance(value, str) or not value.startswith(prefix):
            raise ValueError(f"metadata {field} must contain the exact tool version")
    platform_value = metadata["platform"]
    if not isinstance(platform_value, str) or not platform_value.strip():
        raise ValueError("metadata platform must be non-empty")
    if metadata["runner"] != RUNNER_DESCRIPTION:
        raise ValueError("metadata runner does not match the fixed protocol")
    if metadata["candidate"] != CANDIDATE_DESCRIPTION:
        raise ValueError("metadata candidate does not match the fixed profile")


def evaluate(evidence):
    """Return a structured qualification report without raising on bad evidence."""
    if not isinstance(evidence, dict) or evidence.get("schema") != 1:
        return _fail("evidence schema must equal 1")
    profiles = evidence.get("profiles")
    if not isinstance(profiles, dict):
        return _fail("profiles object is missing")

    try:
        _validate_metadata(evidence.get("metadata"))
        default_size, default_records = _profile_records(
            "default", profiles.get("default")
        )
        candidate_size, candidate_records = _profile_records(
            "candidate", profiles.get("candidate")
        )
    except ValueError as error:
        return _fail(str(error))

    reasons = []
    default_workloads = set(default_records)
    candidate_workloads = set(candidate_records)
    if default_workloads != candidate_workloads:
        reasons.append("default and candidate workload sets differ")

    workload_reports = {}
    ratios = []
    for workload in sorted(default_workloads & candidate_workloads):
        default_values = default_records[workload]
        candidate_values = candidate_records[workload]
        iterations = {row["iterations"] for row in default_values + candidate_values}
        if len(iterations) != 1:
            reasons.append(f"{workload} iterations differ across profiles or runs")
        checksums = {row["checksum"] for row in default_values + candidate_values}
        if len(checksums) != 1:
            reasons.append(f"{workload} checksum differs across profiles or runs")

        default_median = statistics.median(row["elapsed_ns"] for row in default_values)
        candidate_median = statistics.median(
            row["elapsed_ns"] for row in candidate_values
        )
        ratio = candidate_median / default_median
        regression = ratio - 1.0
        ratios.append(ratio)
        workload_reports[workload] = {
            "iterations": next(iter(iterations)) if len(iterations) == 1 else None,
            "checksum": next(iter(checksums)) if len(checksums) == 1 else None,
            "default_median_elapsed_ns": default_median,
            "candidate_median_elapsed_ns": candidate_median,
            "candidate_to_default_ratio": ratio,
            "regression": regression,
        }
        if regression > MAXIMUM_WORKLOAD_REGRESSION + ROUNDING_EPSILON:
            reasons.append(
                f"{workload} regression {regression:.6f} exceeds "
                f"{MAXIMUM_WORKLOAD_REGRESSION:.6f}"
            )

    combined_ratio = (
        math.exp(sum(math.log(ratio) for ratio in ratios) / len(ratios))
        if ratios
        else math.inf
    )
    combined_improvement = 1.0 - combined_ratio
    if combined_improvement + ROUNDING_EPSILON < MINIMUM_COMBINED_IMPROVEMENT:
        reasons.append(
            f"combined improvement {combined_improvement:.6f} is below "
            f"{MINIMUM_COMBINED_IMPROVEMENT:.6f}"
        )

    binary_growth = candidate_size / default_size - 1.0
    if binary_growth > MAXIMUM_BINARY_GROWTH + ROUNDING_EPSILON:
        reasons.append(
            f"binary growth {binary_growth:.6f} exceeds "
            f"{MAXIMUM_BINARY_GROWTH:.6f}"
        )

    return {
        "qualified": not reasons,
        "reasons": reasons,
        "thresholds": _thresholds(),
        "combined_candidate_to_default_ratio": combined_ratio,
        "combined_improvement": combined_improvement,
        "binary": {
            "default_bytes": default_size,
            "candidate_bytes": candidate_size,
            "growth": binary_growth,
        },
        "workloads": workload_reports,
    }


def _load_benchmark_runs(profile_name, directory):
    paths = sorted(directory.glob("run-*.json"))
    if len(paths) != MEASURED_RUNS:
        raise ValueError(
            f"{profile_name} benchmark directory must contain exactly five run JSON files"
        )
    runs = []
    for path in paths:
        document = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(document, dict) or document.get("schema") != 1:
            raise ValueError(f"{path} benchmark schema must equal 1")
        workloads = document.get("workloads")
        if not isinstance(workloads, list) or not workloads:
            raise ValueError(f"{path} benchmark workloads are missing")
        runs.append({"source": path.name, "workloads": workloads})
    return runs


def collect_evidence(
    default_directory,
    candidate_directory,
    default_binary,
    candidate_binary,
    metadata,
):
    """Assemble tracked benchmark output into the comparison evidence schema."""
    for path in (default_binary, candidate_binary):
        if not path.is_file():
            raise ValueError(f"benchmark binary is missing: {path}")
    return {
        "schema": 1,
        "metadata": {
            "revision": metadata["revision"],
            "rustc": metadata["rustc"],
            "cargo": metadata["cargo"],
            "platform": metadata.get("platform", platform.platform()),
            "runner": RUNNER_DESCRIPTION,
            "candidate": CANDIDATE_DESCRIPTION,
        },
        "profiles": {
            "default": {
                "binary_bytes": default_binary.stat().st_size,
                "runs": _load_benchmark_runs("default", default_directory),
            },
            "candidate": {
                "binary_bytes": candidate_binary.stat().st_size,
                "runs": _load_benchmark_runs("candidate", candidate_directory),
            },
        },
    }


def _write_json(path, value):
    output = json.dumps(value, indent=2, sort_keys=True)
    path.write_text(f"{output}\n", encoding="utf-8")
    return output


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    evaluate_parser = subparsers.add_parser("evaluate")
    evaluate_parser.add_argument("evidence", type=Path)
    evaluate_parser.add_argument("--report", type=Path)

    collect_parser = subparsers.add_parser("collect")
    collect_parser.add_argument("--default-dir", type=Path, required=True)
    collect_parser.add_argument("--candidate-dir", type=Path, required=True)
    collect_parser.add_argument("--default-binary", type=Path, required=True)
    collect_parser.add_argument("--candidate-binary", type=Path, required=True)
    collect_parser.add_argument("--evidence", type=Path, required=True)
    collect_parser.add_argument("--report", type=Path, required=True)
    collect_parser.add_argument("--revision", required=True)
    collect_parser.add_argument("--rustc", required=True)
    collect_parser.add_argument("--cargo", required=True)
    collect_parser.add_argument("--platform", required=True)
    arguments = parser.parse_args()

    if arguments.command == "collect":
        evidence = collect_evidence(
            arguments.default_dir,
            arguments.candidate_dir,
            arguments.default_binary,
            arguments.candidate_binary,
            {
                "revision": arguments.revision,
                "rustc": arguments.rustc,
                "cargo": arguments.cargo,
                "platform": arguments.platform,
            },
        )
        _write_json(arguments.evidence, evidence)
    else:
        evidence = json.loads(arguments.evidence.read_text(encoding="utf-8"))

    report = evaluate(evidence)
    output = json.dumps(report, indent=2, sort_keys=True)
    print(output)
    if arguments.report is not None:
        _write_json(arguments.report, report)
    return 0 if report["qualified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
