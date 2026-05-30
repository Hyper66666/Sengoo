import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock


BENCH_ROOT = Path(__file__).resolve().parents[1]
TMP_ROOT = Path(__file__).resolve().parent / "_case_tmp"
REAL_PATH_EXISTS = Path.exists
REAL_PATH_MKDIR = Path.mkdir
REAL_PATH_WRITE_TEXT = Path.write_text


def load_bench_module():
    module_path = BENCH_ROOT / "llm_scheduler_bench.py"
    spec = importlib.util.spec_from_file_location("llm_scheduler_bench_under_test", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def make_output_dir(name: str) -> Path:
    TMP_ROOT.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix=f"llm-scheduler-{name}-", dir=TMP_ROOT))


def skip_build_binary_path(bench_root: Path, mod) -> Path:
    return (
        bench_root
        / ".llm-scheduler-work"
        / "sengoo-scheduler-runner"
        / "target"
        / "release"
        / mod.exe_name("sengoo_scheduler_runner")
    )


def sample_measurement(checksum: int, loop_ms: float, kernel_iters: int) -> dict[str, object]:
    return {
        "process_wall_samples_ms": [loop_ms + 1.0],
        "init_samples_ms": [1.0],
        "loop_samples_ms": [loop_ms],
        "total_samples_ms": [loop_ms + 1.0],
        "tokens_per_sec_samples": [1000.0 + kernel_iters],
        "process_wall_avg_ms": loop_ms + 1.0,
        "init_avg_ms": 1.0,
        "loop_avg_ms": loop_ms,
        "loop_p50_ms": loop_ms,
        "total_avg_ms": loop_ms + 1.0,
        "tokens_per_sec_avg": 1000.0 + kernel_iters,
        "tokens_per_sec_p50": 1000.0 + kernel_iters,
        "checksum": checksum,
        "checksum_consistent": True,
        "total_tokens": 64,
        "steps": 10,
    }


class LlmSchedulerBenchSmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.mod = load_bench_module()

    def test_ci_smoke_preset_overrides_to_fixed_tiny_workload(self) -> None:
        args = self.mod.parse_args(["--preset", "ci-smoke", "--requests", "999", "--samples", "9"])
        self.mod.apply_preset(args)

        self.assertEqual(args.preset, "ci-smoke")
        self.assertEqual(args.requests, 64)
        self.assertEqual(args.max_batch, 8)
        self.assertEqual(args.max_new_per_step, 4)
        self.assertEqual(args.max_len, 6)
        self.assertEqual(args.samples, 1)
        self.assertEqual(args.warmup, 0)
        self.assertEqual(args.light_kernel_iters, 0)
        self.assertEqual(args.heavy_kernel_iters, 8)

    def test_smoke_alias_maps_to_ci_smoke(self) -> None:
        args = self.mod.parse_args(["--smoke"])
        self.mod.apply_preset(args)

        self.assertEqual(args.preset, "ci-smoke")
        self.assertEqual(args.requests, 64)
        self.assertEqual(args.samples, 1)
        self.assertEqual(args.warmup, 0)

    def test_ci_smoke_run_writes_standard_report_schema(self) -> None:
        bench_root = make_output_dir("bench-root")
        expected_binary = skip_build_binary_path(bench_root, self.mod)
        out_dir = make_output_dir("report")
        out_path = out_dir / "ci-smoke-report.json"
        args = self.mod.parse_args(["--preset", "ci-smoke", "--skip-build", "--out", str(out_path)])
        self.mod.apply_preset(args)
        written: dict[str, object] = {}

        def write_text_side_effect(
            path: Path,
            data: str,
            encoding: str | None = None,
            errors: str | None = None,
            newline: str | None = None,
        ) -> int:
            if path == out_path.resolve():
                written["payload"] = json.loads(data)
                return len(data)
            return REAL_PATH_WRITE_TEXT(path, data, encoding=encoding, errors=errors, newline=newline)

        with mock.patch.object(self.mod, "resolve_sengoo_root", return_value=bench_root), mock.patch.object(
            self.mod,
            "prepare_workload_with_fallback",
            return_value=(
                bench_root / ".llm-scheduler-work" / "workload",
                bench_root / ".llm-scheduler-work" / "python_scheduler_runner.py",
                bench_root / ".llm-scheduler-work",
            ),
        ), mock.patch.object(
            self.mod,
            "measure_runner",
            side_effect=[
                sample_measurement(101, 4.0, 0),
                sample_measurement(101, 2.0, 0),
                sample_measurement(202, 8.0, 8),
                sample_measurement(202, 6.0, 8),
            ],
        ), mock.patch.object(
            self.mod.Path,
            "exists",
            autospec=True,
            side_effect=lambda path: path == expected_binary or REAL_PATH_EXISTS(path),
        ), mock.patch.object(
            self.mod.Path,
            "mkdir",
            autospec=True,
            return_value=None,
        ), mock.patch.object(
            self.mod.Path,
            "write_text",
            autospec=True,
            side_effect=write_text_side_effect,
        ), mock.patch.object(self.mod, "print_table"):
            report, written_path = self.mod.run_benchmark(args, bench_root=bench_root)

        self.assertEqual(written_path, out_path.resolve())
        self.assertEqual(report["scenario"], "llm-scheduler-bench")
        self.assertEqual(report["inputs"]["requests"], 64)
        self.assertEqual(report["inputs"]["max_batch"], 8)
        self.assertEqual(report["inputs"]["samples"], 1)
        self.assertEqual(len(report["scenarios"]), 2)

        written_payload = written["payload"]
        self.assertIsInstance(written_payload, dict)
        self.assertEqual(written_payload["scenario"], "llm-scheduler-bench")
        self.assertEqual(written_payload["inputs"]["max_new_per_step"], 4)
        self.assertIn("python", written_payload["scenarios"][0])
        self.assertIn("sengoo", written_payload["scenarios"][0])
        self.assertIn("comparison", written_payload["scenarios"][0])

    def test_ci_smoke_run_fails_on_checksum_mismatch(self) -> None:
        bench_root = make_output_dir("bench-root")
        expected_binary = skip_build_binary_path(bench_root, self.mod)
        args = self.mod.parse_args(["--preset", "ci-smoke", "--skip-build"])
        self.mod.apply_preset(args)

        with mock.patch.object(self.mod, "resolve_sengoo_root", return_value=bench_root), mock.patch.object(
            self.mod,
            "prepare_workload_with_fallback",
            return_value=(
                bench_root / ".llm-scheduler-work" / "workload",
                bench_root / ".llm-scheduler-work" / "python_scheduler_runner.py",
                bench_root / ".llm-scheduler-work",
            ),
        ), mock.patch.object(
            self.mod,
            "measure_runner",
            side_effect=[
                sample_measurement(101, 4.0, 0),
                sample_measurement(999, 2.0, 0),
            ],
        ), mock.patch.object(
            self.mod.Path,
            "exists",
            autospec=True,
            side_effect=lambda path: path == expected_binary or REAL_PATH_EXISTS(path),
        ), mock.patch.object(
            self.mod.Path,
            "mkdir",
            autospec=True,
            return_value=None,
        ), mock.patch.object(self.mod, "print_table"):
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                self.mod.run_benchmark(args, bench_root=bench_root)

    def test_skip_build_falls_back_to_temp_work_dir_when_preferred_dir_is_blocked(self) -> None:
        bench_root = make_output_dir("bench-root")
        expected_binary = skip_build_binary_path(bench_root, self.mod)
        out_path = make_output_dir("report") / "ci-smoke-report.json"
        args = self.mod.parse_args(["--preset", "ci-smoke", "--skip-build", "--out", str(out_path)])
        self.mod.apply_preset(args)

        fallback_dir = make_output_dir("fallback-work")
        seen_dirs: list[Path] = []
        preferred_dir = bench_root / ".llm-scheduler-work"
        written: dict[str, object] = {}

        def prepare_workload_side_effect(work_dir: Path) -> tuple[Path, Path]:
            seen_dirs.append(work_dir)
            return work_dir / "workload", work_dir / "python_scheduler_runner.py"

        def mkdir_side_effect(path: Path, mode: int = 0o777, parents: bool = False, exist_ok: bool = False) -> None:
            if path == preferred_dir:
                raise PermissionError("blocked")
            return None

        def write_text_side_effect(
            path: Path,
            data: str,
            encoding: str | None = None,
            errors: str | None = None,
            newline: str | None = None,
        ) -> int:
            if path == out_path.resolve():
                written["payload"] = json.loads(data)
                return len(data)
            return REAL_PATH_WRITE_TEXT(path, data, encoding=encoding, errors=errors, newline=newline)

        with mock.patch.object(self.mod, "resolve_sengoo_root", return_value=bench_root), mock.patch.object(
            self.mod,
            "prepare_workload",
            side_effect=prepare_workload_side_effect,
        ), mock.patch.object(
            self.mod,
            "make_temp_work_dir",
            return_value=fallback_dir,
        ), mock.patch.object(
            self.mod.Path,
            "mkdir",
            autospec=True,
            side_effect=mkdir_side_effect,
        ), mock.patch.object(
            self.mod.Path,
            "exists",
            autospec=True,
            side_effect=lambda path: path == expected_binary or REAL_PATH_EXISTS(path),
        ), mock.patch.object(
            self.mod.Path,
            "write_text",
            autospec=True,
            side_effect=write_text_side_effect,
        ), mock.patch.object(
            self.mod,
            "measure_runner",
            side_effect=[
                sample_measurement(101, 4.0, 0),
                sample_measurement(101, 2.0, 0),
                sample_measurement(202, 8.0, 8),
                sample_measurement(202, 6.0, 8),
            ],
        ), mock.patch.object(self.mod, "print_table"):
            report, written_path = self.mod.run_benchmark(args, bench_root=bench_root)

        self.assertEqual(
            seen_dirs,
            [fallback_dir],
        )
        self.assertEqual(written_path, out_path.resolve())
        self.assertEqual(report["scenario"], "llm-scheduler-bench")
        self.assertEqual(report["inputs"]["preset"], "ci-smoke")
        self.assertIsInstance(written["payload"], dict)


if __name__ == "__main__":
    unittest.main()
