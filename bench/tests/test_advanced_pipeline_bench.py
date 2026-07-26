import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "advanced_pipeline_bench.py"
SPEC = importlib.util.spec_from_file_location("advanced_pipeline_bench", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
BENCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BENCH)


class AdvancedPipelineBenchTests(unittest.TestCase):
    def test_sengoo_build_command_selects_source_runtime_before_subcommand(self):
        command = BENCH.sengoo_build_cmd(
            Path("sgc"),
            Path("main.sg"),
            2,
            True,
            emit_llvm=True,
            output=Path("main.ll"),
        )
        self.assertEqual(
            command,
            [
                "sgc",
                "--runtime-mode",
                "source-development",
                "build",
                "main.sg",
                "-O",
                "2",
                "--emit-llvm",
                "-o",
                "main.ll",
                "--force-rebuild",
            ],
        )


if __name__ == "__main__":
    unittest.main()
