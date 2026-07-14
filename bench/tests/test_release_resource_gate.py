import importlib.util
import re
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release_resource_gate.py"
BUDGETS_DOC = Path(__file__).resolve().parents[1] / "PRODUCTION_BUDGETS.md"
SPEC = importlib.util.spec_from_file_location("release_resource_gate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GATE)


def report(**overrides):
    metrics = {
        "sgc_artifact_bytes": 10 * 1024 * 1024,
        "program_artifact_bytes": 2 * 1024 * 1024,
        "startup": {"avg_ms": 20.0},
        "check": {"avg_ms": 50.0},
        "full_build_ms": 500.0,
        "runtime": {"avg_ms": 10.0},
    }
    metrics.update(overrides)
    return {"metrics": metrics}


class ReleaseResourceGateTests(unittest.TestCase):
    def test_default_full_build_budget_matches_documented_threshold(self):
        documented = BUDGETS_DOC.read_text(encoding="utf-8")
        match = re.search(
            r"- median of repeated forced optimized builds: ([0-9,]+) ms;",
            documented,
        )
        self.assertIsNotNone(match, documented)
        self.assertEqual(GATE.DEFAULT_BUDGETS["full_build_ms"], 5_000.0)
        self.assertEqual(float(match.group(1).replace(",", "")), 5_000.0)

    def test_accepts_metrics_within_every_budget(self):
        sample = report()
        self.assertEqual(GATE.evaluate_report(sample, dict(GATE.DEFAULT_BUDGETS)), [])
        self.assertTrue(sample["passed"])

    def test_uses_repeated_full_build_summary_for_budget_decision(self):
        sample = report(
            full_build={
                "samples_ms": [4800.0, 4900.0, 5000.0],
                "avg_ms": 4900.0,
                "median_ms": 4900.0,
                "max_ms": 5000.0,
            }
        )
        sample["metrics"].pop("full_build_ms")
        self.assertEqual(GATE.evaluate_report(sample, dict(GATE.DEFAULT_BUDGETS)), [])
        self.assertEqual(sample["measured_for_budget"]["full_build_ms"], 4900.0)

    def test_repeated_full_build_drift_over_budget_fails(self):
        sample = report(
            full_build={
                "samples_ms": [4800.0, 5100.0, 5300.0],
                "avg_ms": 5066.67,
                "median_ms": 5100.0,
                "max_ms": 5300.0,
            }
        )
        sample["metrics"].pop("full_build_ms")
        violations = GATE.evaluate_report(sample, dict(GATE.DEFAULT_BUDGETS))
        self.assertIn("full_build_ms=5100.00 exceeds budget 5000.00", violations)
        self.assertFalse(sample["passed"])

    def test_reports_each_exceeded_budget(self):
        sample = report(
            sgc_artifact_bytes=21 * 1024 * 1024,
            startup={"avg_ms": 251.0},
            runtime={"avg_ms": 251.0},
        )
        violations = GATE.evaluate_report(sample, dict(GATE.DEFAULT_BUDGETS))
        self.assertEqual(len(violations), 3)
        self.assertFalse(sample["passed"])


if __name__ == "__main__":
    unittest.main()
