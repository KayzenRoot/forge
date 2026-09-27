#!/usr/bin/env python3
"""Regression tests for exact-binding M00 candidate performance budgets."""

from __future__ import annotations

import copy
import math
import unittest

from check_m00_performance import (
    COMMON_RELATIVE_ALLOWANCE_PERCENT,
    candidate_reference_identity_errors,
    candidate_workload_reference_errors,
    median_mad_budget,
)


class CandidateBudgetReferenceTests(unittest.TestCase):
    candidate_sha = "a" * 40
    candidate_tree_sha = "b" * 40
    baseline_sha = "c" * 40
    environment = {
        "platform": "Windows-test",
        "processor": "Test CPU",
        "architecture": "x86_64",
        "rustc_vv": "rustc test",
        "cargo": "cargo test",
    }

    def setUp(self) -> None:
        self.outer_medians = [100, 101, 99, 100, 102, 98, 100, 101, 99, 100]
        summary = median_mad_budget(self.outer_medians)
        self.reference = {
            "schema_version": 1,
            "work_order": "FGE-004-M00",
            "reference_kind": "repeated_candidate_baseline_not_external_slo",
            "candidate_sha": self.candidate_sha,
            "candidate_tree_sha": self.candidate_tree_sha,
            "baseline_sha": self.baseline_sha,
            "outer_runs": 10,
            "inner_samples_per_workload": 5,
            "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
            "environment": self.environment.copy(),
            "workloads": {},
        }
        self.workload = {
            "definition_sha256": "d" * 64,
            "iterations_per_sample": 500,
            "inner_samples_per_outer_run": 5,
            "outer_medians_ns": self.outer_medians.copy(),
            "reference_median_ns": summary["median_ns"],
            "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
            "upper_budget_ns": math.ceil(
                summary["median_ns"] * (100 + COMMON_RELATIVE_ALLOWANCE_PERCENT) / 100
            ),
            "measurement_summary": summary,
        }

    def identity_errors(self, reference: object | None = None) -> list[str]:
        return candidate_reference_identity_errors(
            self.reference if reference is None else reference,
            candidate_sha=self.candidate_sha,
            candidate_tree_sha=self.candidate_tree_sha,
            baseline_sha=self.baseline_sha,
            outer_runs=10,
            environment=self.environment,
        )

    def workload_errors(self, reference: object | None = None) -> list[str]:
        return candidate_workload_reference_errors(
            self.workload if reference is None else reference,
            current_definition_sha256="d" * 64,
            current_iterations_per_sample=500,
            current_inner_samples=5,
            expected_outer_runs=10,
        )

    def test_exact_candidate_baseline_and_environment_binding_passes(self) -> None:
        self.assertEqual(self.identity_errors(), [])

    def test_stale_candidate_tree_or_baseline_is_rejected(self) -> None:
        for field in ("candidate_sha", "candidate_tree_sha", "baseline_sha"):
            with self.subTest(field=field):
                altered = copy.deepcopy(self.reference)
                altered[field] = "e" * 40
                self.assertIn(f"{field}_mismatch", self.identity_errors(altered))

    def test_changed_run_count_environment_or_reference_kind_is_rejected(self) -> None:
        for field, value in (
            ("outer_runs", 9),
            ("inner_samples_per_workload", 4),
            ("environment", {"platform": "other"}),
            ("reference_kind", "external_slo"),
        ):
            with self.subTest(field=field):
                altered = copy.deepcopy(self.reference)
                altered[field] = value
                self.assertTrue(self.identity_errors(altered))

    def test_boolean_schema_version_is_rejected(self) -> None:
        altered = copy.deepcopy(self.reference)
        altered["schema_version"] = True
        self.assertIn("schema_version_mismatch", self.identity_errors(altered))

    def test_measured_workload_reference_passes(self) -> None:
        self.assertEqual(self.workload_errors(), [])

    def test_claimed_median_must_match_repeated_measurements(self) -> None:
        altered = copy.deepcopy(self.workload)
        altered["reference_median_ns"] += 10
        self.assertIn("reference_median_mismatch", self.workload_errors(altered))

    def test_summary_and_budget_must_match_repeated_measurements(self) -> None:
        altered = copy.deepcopy(self.workload)
        altered["measurement_summary"]["median_ns"] += 1
        altered["upper_budget_ns"] += 100
        errors = self.workload_errors(altered)
        self.assertIn("measurement_summary_mismatch", errors)
        self.assertIn("upper_budget_mismatch", errors)

    def test_workload_definition_and_sample_shape_are_bound(self) -> None:
        for field, value, expected_error in (
            ("definition_sha256", "f" * 64, "definition_sha256_mismatch"),
            ("iterations_per_sample", 499, "iterations_per_sample_mismatch"),
            ("inner_samples_per_outer_run", 4, "inner_samples_per_outer_run_mismatch"),
        ):
            with self.subTest(field=field):
                altered = copy.deepcopy(self.workload)
                altered[field] = value
                self.assertIn(expected_error, self.workload_errors(altered))

    def test_invalid_or_truncated_outer_medians_are_rejected(self) -> None:
        altered = copy.deepcopy(self.workload)
        altered["outer_medians_ns"] = [True] * 10
        self.assertIn("outer_medians_invalid", self.workload_errors(altered))
        altered["outer_medians_ns"] = [100] * 9
        self.assertIn("outer_medians_invalid", self.workload_errors(altered))


if __name__ == "__main__":
    unittest.main(verbosity=2)
