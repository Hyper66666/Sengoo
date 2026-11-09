"""Negative controls for the historical linker qualification manifest chain."""
from pathlib import Path
import hashlib
import json
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from linker_evidence import load_linker


class LinkerEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="sg-linker-qualification-")
        self.root = Path(self.temp.name)
        self.child_dir = self.root / "target/foundation"
        self.child_dir.mkdir(parents=True)
        self.original = self.root / "implementation.sg"
        self.original.write_bytes(b"qualified SG linker source")
        self.tool = self.root / "seed.exe"
        self.tool.write_bytes(b"seed identity")
        self.binary = self.child_dir / "sg-linker.exe"
        self.binary.write_bytes(b"qualified SG linker binary")
        self.child = {"schema": 1, "stage": "sg-coff-pe-linker-foundation", "result": "passed", "changed_sources": [], "changed_tools": [],
                      "sources": {"implementation.sg": self.sha(self.original)}, "steps": [{"name": "real-qualified-build"}],
                      "tools": {"seed": {"path": str(self.tool), "sha256": self.sha(self.tool)}}, "artifacts": {"sg-linker.exe": self.sha(self.binary)}}
        self.child_path = self.child_dir / "evidence.json"
        self.save(self.child_path, self.child)
        self.wrapper_path = self.root / "target/copied/evidence.json"
        self.snapshot = self.wrapper_path.parent / "qualified-source.sg"
        self.snapshot.parent.mkdir(parents=True)
        self.snapshot.write_bytes(self.original.read_bytes())
        self.snapshot_key = str(self.snapshot.relative_to(self.root))
        self.wrapper = {"schema": 1, "stage": "sg-source-sg-link-execute", "result": "passed", "changed_sources": [], "changed_tools": [],
                        "sources": {str(self.child_path.relative_to(self.root)): self.sha(self.child_path), self.snapshot_key: self.sha(self.snapshot)},
                        "steps": [{"name": "qualified-consumer"}], "tools": {"linker": {"path": str(self.binary), "sha256": self.sha(self.binary)}},
                        "qualifications": {"linker": {"path": str(self.child_path), "sha256": self.sha(self.child_path), "steps": 1,
                                                      "sources": {"implementation.sg": {"sha256": self.sha(self.original), "snapshot": self.snapshot_key}}}}}
        self.save(self.wrapper_path, self.wrapper)

    @staticmethod
    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    @staticmethod
    def save(path, value):
        path.write_text(json.dumps(value), encoding="utf-8")

    def tearDown(self):
        self.temp.cleanup()

    def test_direct_qualification_requires_current_source(self):
        self.assertEqual(load_linker(self.root, self.child_path)["binary_sha256"], self.sha(self.binary))
        self.original.write_bytes(b"unqualified current rewrite")
        with self.assertRaises(ValueError):
            load_linker(self.root, self.child_path)

    def test_complete_copied_sources_keep_historical_qualification(self):
        self.original.write_bytes(b"new checkout implementation")
        result = load_linker(self.root, self.wrapper_path)
        self.assertEqual(len(result["sources"]), 1)
        self.assertEqual(len(result["chain"]), 2)
        self.assertEqual(result["sources"]["implementation.sg"]["path"], str(self.snapshot))

    def test_missing_source_binding_is_rejected(self):
        self.wrapper["qualifications"]["linker"]["sources"].clear()
        self.save(self.wrapper_path, self.wrapper)
        with self.assertRaises(ValueError):
            load_linker(self.root, self.wrapper_path)

    def test_rehashed_snapshot_cannot_replace_original_qualification(self):
        self.snapshot.write_bytes(b"changed copied implementation")
        new_digest = self.sha(self.snapshot)
        self.wrapper["sources"][self.snapshot_key] = new_digest
        self.wrapper["qualifications"]["linker"]["sources"]["implementation.sg"]["sha256"] = new_digest
        self.save(self.wrapper_path, self.wrapper)
        with self.assertRaises(ValueError):
            load_linker(self.root, self.wrapper_path)

    def test_parent_must_bind_child_report_hash(self):
        self.wrapper["sources"][str(self.child_path.relative_to(self.root))] = "0" * 64
        self.save(self.wrapper_path, self.wrapper)
        with self.assertRaises(ValueError):
            load_linker(self.root, self.wrapper_path)

    def test_modified_executable_or_tool_is_rejected(self):
        self.binary.write_bytes(b"new linker build")
        with self.assertRaises(ValueError):
            load_linker(self.root, self.wrapper_path)
        self.binary.write_bytes(b"qualified SG linker binary")
        self.tool.write_bytes(b"different producer seed")
        with self.assertRaises(ValueError):
            load_linker(self.root, self.wrapper_path)

    def test_failed_or_changed_input_qualification_is_rejected(self):
        for key, value in (("result", "failed"), ("changed_sources", ["implementation.sg"]), ("changed_tools", ["seed"])):
            changed = dict(self.wrapper, **{key: value})
            self.save(self.wrapper_path, changed)
            with self.assertRaises(ValueError):
                load_linker(self.root, self.wrapper_path)

    def test_module_consumer_can_be_verified_recursively(self):
        module_path = self.root / "target/module/evidence.json"
        module_path.parent.mkdir(parents=True)
        copied_binary = module_path.parent / "sg-linker.exe"
        copied_binary.write_bytes(self.binary.read_bytes())
        tool = {"path": str(copied_binary), "sha256": self.sha(copied_binary)}
        module = {"schema": 1, "stage": "sg-native-module-graph", "result": "passed", "changed_sources": [], "changed_tools": [],
                  "sources": {str(self.wrapper_path.relative_to(self.root)): self.sha(self.wrapper_path), self.snapshot_key: self.sha(self.snapshot)},
                  "steps": [{"name": "module-sg-link"}], "tools": {"sg-linker": tool},
                  "linker_qualification": {"evidence": {"path": str(self.wrapper_path), "sha256": self.sha(self.wrapper_path)},
                                           "sources": {"implementation.sg": {"snapshot": self.snapshot_key, "sha256": self.sha(self.snapshot)}}, "binary": tool}}
        self.save(module_path, module)
        self.assertEqual(len(load_linker(self.root, module_path)["chain"]), 3)
        self.snapshot.write_bytes(b"changed after module qualification")
        with self.assertRaises(ValueError):
            load_linker(self.root, module_path)


if __name__ == "__main__":
    unittest.main()
