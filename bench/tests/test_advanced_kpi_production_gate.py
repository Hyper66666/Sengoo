import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "bench" / "scripts" / "advanced-kpi-gate.py"
SAMPLE = ROOT / "bench" / "sample-frontend-1000k-gate-ok.json"
BASELINE = ROOT / "bench" / "frontend-memory-baseline.json"
BASELINE_DOC = ROOT / "bench" / "FRONTEND_BASELINE.md"


class AdvancedKpiProductionGateTests(unittest.TestCase):
    def baseline_profile(self):
        return json.loads(BASELINE.read_text(encoding="utf-8"))

    def raw_baseline_path(self):
        baseline = self.baseline_profile()
        return ROOT / Path(baseline["baseline_report_path"])

    def full_shape_report(self):
        baseline = self.baseline_profile()
        report = json.loads(self.raw_baseline_path().read_text(encoding="utf-8"))
        report["host"]["actions_run"] = baseline["baseline_actions_run"]
        report["config"]["scale_iterations_by_loc"] = {
            "1000": 5,
            "10000": 5,
            "100000": 5,
            "1000000": 5,
        }
        report["config"]["memory_iters_by_loc"] = {
            "10000": 3,
            "100000": 3,
            "1000000": 3,
        }
        report["config"]["memory_command_timeout_s"] = 300
        report["config"]["scale_command_timeout_s"] = 300
        report["config"]["reachability_iters"] = 5
        report["config"]["reachability_profiles"] = [
            "all_reachable",
            "half_reachable",
            "library_entryless",
        ]
        report["fairness"] = {
            "cpp": "precompiled header (PCH) enabled",
            "rust": "cargo incremental enabled (CARGO_INCREMENTAL=1)",
            "memory_compare_rust": (
                "direct rustc compile-to-object when rustup toolchain path is available"
            ),
        }
        report["phase_deltas"] = {
            "incremental_vs_target_ms": {},
            "scale_100k_vs_target_ms": {},
        }
        report["rollback_evidence"] = {
            "schema_version": 1,
            "baseline_profile_path": str(BASELINE.resolve()),
            "baseline_report_id": baseline["baseline_report_id"],
            "baseline_report_path": baseline["baseline_report_path"],
            "comparisons": [],
            "reasons": [],
            "gate_decision": "pass",
        }
        report["notes"] = [
            "Scale curve e2e includes link time for compiled languages.",
            "Compile-memory comparison tracks peak process RSS per compiler command.",
        ]
        return report

    def run_gate(self, sample, baseline_profile=BASELINE):
        with tempfile.TemporaryDirectory(prefix="sengoo-kpi-gate-") as temp:
            sample_path = Path(temp) / "sample.json"
            decision_path = Path(temp) / "decision.json"
            sample_path.write_text(json.dumps(sample), encoding="utf-8")
            return subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--mode",
                    "hard",
                    "--sample",
                    str(sample_path),
                    "--baseline-profile",
                    str(baseline_profile),
                    "--skip-absolute-targets",
                    "--decision-out",
                    str(decision_path),
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

    def baseline_sample(self):
        sample = json.loads(SAMPLE.read_text(encoding="utf-8"))
        baseline = json.loads(BASELINE.read_text(encoding="utf-8"))["metrics"]
        for bucket in ("100000", "1000000"):
            sengoo = sample["scale_curve"][bucket]["sengoo"]
            sengoo["compile_frontend_llvm_avg_ms"] = baseline[bucket][
                "compile_frontend_llvm_avg_ms"
            ]
            sengoo["e2e_avg_ms"] = baseline[bucket]["e2e_avg_ms"]
            memory = sample["compile_memory_compare"][bucket]
            memory["sengoo"]["peak_rss_mb_avg"] = baseline[bucket][
                "peak_rss_mb_avg"
            ]
            # Deliberately violate the cross-language ratio. The production
            # gate treats this as trend evidence while retaining Sengoo RSS.
            memory["cpp"]["peak_rss_mb_avg"] = 1.0
        return sample

    def test_baseline_profile_points_to_retained_raw_report(self):
        baseline = self.baseline_profile()
        raw_baseline = self.raw_baseline_path()
        self.assertTrue(baseline["baseline_report_path"].startswith("bench/results/"))
        self.assertTrue(baseline["baseline_report_path"].endswith("-advanced-pipeline.json"))
        self.assertTrue(raw_baseline.is_file())

    def test_bootstrap_baseline_is_explicitly_marked_pending_raw_ci_artifact(self):
        baseline = self.baseline_profile()
        report = json.loads(self.raw_baseline_path().read_text(encoding="utf-8"))
        docs = BASELINE_DOC.read_text(encoding="utf-8")

        self.assertTrue(baseline["bootstrap_pending_raw_ci_report"])
        self.assertEqual(
            baseline["baseline_report_id"],
            f"{report['generated_at_unix_ms']}-advanced-pipeline",
        )
        self.assertEqual(
            report["host"]["actions_run"],
            baseline["baseline_actions_run"],
        )
        self.assertIn("bootstrap", docs.lower())
        self.assertIn("pending the next perf-smoke artifact upload", docs)
        self.assertIn("reconstructed", "\n".join(report["notes"]).lower())

    def test_gate_accepts_full_shape_baseline_report_metadata(self):
        with tempfile.TemporaryDirectory(prefix="sengoo-full-baseline-") as temp:
            temp_root = Path(temp)
            raw_report_path = temp_root / "raw-advanced-pipeline.json"
            baseline_profile_path = temp_root / "frontend-memory-baseline.json"

            raw_report_path.write_text(
                json.dumps(self.full_shape_report(), indent=2),
                encoding="utf-8",
            )
            baseline = self.baseline_profile()
            baseline.pop("bootstrap_pending_raw_ci_report", None)
            baseline["baseline_report_path"] = str(raw_report_path)
            baseline_profile_path.write_text(
                json.dumps(baseline, indent=2),
                encoding="utf-8",
            )

            completed = self.run_gate(
                self.baseline_sample(),
                baseline_profile=baseline_profile_path,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_gate_rejects_trimmed_baseline_report_metadata(self):
        with tempfile.TemporaryDirectory(prefix="sengoo-trimmed-baseline-") as temp:
            temp_root = Path(temp)
            trimmed_report_path = temp_root / "trimmed-advanced-pipeline.json"
            trimmed_baseline_path = temp_root / "frontend-memory-baseline.json"

            trimmed_report = self.full_shape_report()
            trimmed_report.pop("rollback_evidence", None)
            trimmed_report["config"].pop("scale_iterations_by_loc", None)
            trimmed_report_path.write_text(
                json.dumps(trimmed_report, indent=2),
                encoding="utf-8",
            )

            baseline = self.baseline_profile()
            baseline.pop("bootstrap_pending_raw_ci_report", None)
            baseline["baseline_report_path"] = str(trimmed_report_path)
            trimmed_baseline_path.write_text(
                json.dumps(baseline, indent=2),
                encoding="utf-8",
            )

            completed = self.run_gate(
                self.baseline_sample(),
                baseline_profile=trimmed_baseline_path,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn(
                "missing required producer metadata",
                completed.stdout + completed.stderr,
            )

    def test_current_baseline_passes_while_cross_language_ratios_are_informational(self):
        completed = self.run_gate(self.baseline_sample())
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_frontend_regression_over_thirty_percent_fails(self):
        sample = self.baseline_sample()
        sample["scale_curve"]["1000000"]["sengoo"][
            "compile_frontend_llvm_avg_ms"
        ] *= 1.31
        completed = self.run_gate(sample)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn(
            "frontend_time/1000000 regression exceeded",
            completed.stdout + completed.stderr,
        )

    def test_checked_in_sample_decision_uses_current_baseline_and_policy(self):
        decision = json.loads(
            (
                ROOT / "bench" / "sample-frontend-1000k-gate-ok-advanced-gate.json"
            ).read_text(encoding="utf-8")
        )
        baseline = self.baseline_profile()
        self.assertEqual(decision["baseline_report_id"], baseline["baseline_report_id"])
        self.assertEqual(
            decision["baseline_report_path"], baseline["baseline_report_path"]
        )
        self.assertEqual(
            decision["thresholds"]["frontend_regression_pct"],
            {"100000": 30.0, "1000000": 30.0},
        )
        self.assertEqual(
            decision["thresholds"]["full_build_regression_pct"],
            {"100000": 30.0, "1000000": 30.0},
        )
        self.assertEqual(
            decision["thresholds"]["rss_regression_pct"],
            {"100000": 30.0, "1000000": 30.0},
        )
        self.assertEqual(
            decision["thresholds"]["frontend_share_regression_pp"],
            {"1000000": 10.0},
        )
        self.assertEqual(decision["gate_decision"], "pass")


if __name__ == "__main__":
    unittest.main()
