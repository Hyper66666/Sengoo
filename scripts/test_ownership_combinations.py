"""Check seeded diversity, bounds, exact Drop oracles and retained failure logs."""
from collections import Counter
import contextlib
import hashlib
import io
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent
GENERATOR = runpy.run_path(str(SCRIPTS / "ownership-combinations.py"))


class OwnershipCombinationsTests(unittest.TestCase):
    def test_fixed_seed_is_reproducible_and_versioned(self):
        first = GENERATOR["corpus"]()
        self.assertEqual(first, GENERATOR["corpus"]())
        encoded = json.dumps(first, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(encoded).hexdigest(),
            "c835da24db33a6210ed1762339d965cb87f96932c2cad3d0e008867cdf28df14")
        self.assertEqual(GENERATOR["VERSION"], 1)

    def test_seeds_vary_real_ownership_shapes(self):
        shapes = {json.dumps(GENERATOR["corpus"](seed, 1)[0]["shape"], sort_keys=True)
            for seed in range(64)}
        self.assertGreater(len(shapes), 40)

    def test_declared_bounds_hold_at_seed_and_case_boundaries(self):
        seeds = [0, 1, GENERATOR["DEFAULT_SEED"], GENERATOR["MASK"], *range(256)]
        for seed in seeds:
            for index in (0, 1, 2, 3, GENERATOR["MAX_CASES"] - 1):
                case = GENERATOR["Program"](seed, index).generate()
                nodes = 0
                ids = []
                for node in case["shape"]:
                    stats = GENERATOR["statistics"](node)
                    self.assertLessEqual(stats["depth"], GENERATOR["MAX_DEPTH"])
                    nodes += stats["nodes"]
                    ids.extend(GENERATOR["resource_ids"](node))
                self.assertLessEqual(nodes, GENERATOR["MAX_NODES"])
                self.assertEqual(len(ids), len(set(ids)))
                self.assertLessEqual(max(ids) - min(ids), 99)

    def test_oracle_models_each_execution_path_independently(self):
        for case in GENERATOR["corpus"]()[:4]:
            model = case["path_oracle"]
            expected = Counter(model["aggregate_resource_ids"] * model["aggregate_runs"])
            expected[model["aggregate_drop_id"]] += model["aggregate_runs"]
            expected[model["branch_true_id"]] += model["branch_true_runs"]
            expected[model["branch_false_id"]] += model["branch_false_runs"]
            expected.update(model["loop_resource_ids"])
            self.assertEqual(dict(expected), case["expected_drop_counts"])
            # The aggregate's own destructor is observable but owns no malloc.
            self.assertEqual(sum(expected.values()) - model["aggregate_runs"], case["expected_resources"])

    def test_exact_oracle_accepts_order_variation_and_rejects_balanced_wrong_drops(self):
        case = GENERATOR["corpus"](0, 1)[0]
        ids = list(Counter(case["expected_drop_counts"]).elements())
        stdout = "\n".join(map(str, reversed(ids))) + "\nownership-generated: passed\n"
        GENERATOR["verify_output"](case, stdout)
        ids[0] = ids[-1]  # Total drops still balance; the per-owner counts do not.
        with self.assertRaisesRegex(RuntimeError, "exact Drop oracle failed"):
            GENERATOR["verify_output"](case, "\n".join(map(str, ids)) + "\nownership-generated: passed\n")
        with self.assertRaisesRegex(RuntimeError, "missing successful"):
            GENERATOR["verify_output"](case, stdout.replace("ownership-generated: passed", ""))

    def test_negative_cases_require_stable_diagnostics(self):
        cases = GENERATOR["rejected_cases"](123)
        self.assertEqual({case["diagnostic"] for case in cases}, {
            "unsupported-partial-move-with-drop", "unsupported-closure-capture", "borrow-escapes-owner"})
        self.assertTrue(all(case["expect"] == "diagnostic" and case["diagnostic"] in case["source"] for case in cases))

    def test_saved_manifest_has_reproducible_source_digests(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = GENERATOR["write_corpus"](root, 5, 2)
            self.assertEqual(len(manifest["cases"]), 5)
            self.assertEqual(json.loads((root / "manifest.json").read_text(encoding="utf-8")), json.loads(json.dumps(manifest)))
            for case in manifest["cases"]:
                source = Path(case["source"])
                self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), case["source_sha256"])
                self.assertEqual(json.loads((source.parent / "case.json").read_text(encoding="utf-8")), json.loads(json.dumps(case)))

    def test_invalid_bounds_do_not_silently_reduce_coverage(self):
        for seed, cases in ((-1, 4), (1 << 64, 4), (0, 0), (0, 33)):
            with self.assertRaises(ValueError):
                GENERATOR["corpus"](seed, cases)
        result = subprocess.run([sys.executable, str(SCRIPTS / "ownership-combinations.py"), "--cases", "0"],
            capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)

    def test_failed_build_retains_seed_source_logs_and_failed_evidence(self):
        gate = runpy.run_path(str(SCRIPTS / "ownership-sanitizer-gate.py"))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sgc = root / "sgc"
            sgc.touch()
            output = root / "evidence.json"
            commands = [subprocess.CompletedProcess([], 0, "clang test\n", ""),
                subprocess.CompletedProcess([], 0, "sgc test\n", ""),
                subprocess.CompletedProcess([], 3, "partial compiler output\n", "reproduced compiler failure\n")]
            argv = ["gate", "--sgc", str(sgc), "--clang", sys.executable,
                "--generated-seed", "123", "--generated-cases", "1", "--output", str(output)]
            # Force an actual gate failure after corpus creation, without
            # requiring native tools to run these portable generator tests.
            with patch.object(sys, "argv", argv), patch.dict(gate["main"].__globals__,
                    {"windows_environment": lambda environment: environment}), \
                    patch("shutil.which", return_value=sys.executable), \
                    patch("subprocess.check_output", side_effect=["0123456789\n", ""]), \
                    patch("subprocess.run", side_effect=commands), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(gate["main"](), 1)
            evidence = json.loads(output.read_text(encoding="utf-8"))
            self.assertFalse(evidence["ok"])
            self.assertIn("reproduced compiler failure", evidence["error"])
            retained = Path(evidence["artifacts"])
            self.assertTrue(retained.is_dir())
            self.assertTrue((retained / "ownership.sg").is_file())
            self.assertEqual(evidence["generated_corpus"]["seed"], 123)
            for case in evidence["generated_cases"]:
                self.assertTrue(Path(case["source"]).is_file())
                self.assertEqual(case["status"], "not_run")
            failed = evidence["commands"][-1]
            self.assertEqual(failed["exit_code"], 3)
            self.assertEqual(Path(failed["stdout_log"]).read_text(encoding="utf-8"), "partial compiler output\n")
            self.assertIn("reproduced compiler failure", Path(failed["stderr_log"]).read_text(encoding="utf-8"))
            self.assertTrue(Path(failed["stderr_log"]).with_name("002.command.json").is_file())


if __name__ == "__main__":
    unittest.main()
