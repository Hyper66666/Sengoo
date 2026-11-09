"""Artifact-set identity checks; no compiler or credentials are involved."""
from pathlib import Path
import tempfile
import unittest

from package_ecosystem import AGENT_PACKAGES, PACKAGES, bundle_name, normalize_agent_dependencies, sha256, verify


class DistributionTests(unittest.TestCase):
    def prepare(self, root, profile):
        names = PACKAGES if profile == "foundation" else AGENT_PACKAGES
        files = [f"{p}-0.1.0.tar.gz" for p in names] + ["ecosystem-manifest.json", bundle_name(profile)]
        for name in files:
            (root / name).write_bytes(b"synthetic artifact identity fixture")
        (root / "SHA256SUMS").write_text("".join(f"{sha256(root / name)}  {name}\n" for name in files), encoding="utf-8")
        return files

    def test_explicit_profiles_cannot_be_substituted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); self.prepare(root, "agents"); verify(root, "agents")
            with self.assertRaisesRegex(ValueError, "exact official artifact set"):
                verify(root)  # Default foundation interface is preserved.

    def test_old_profile_and_corruption(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); names = self.prepare(root, "foundation"); verify(root)
            (root / names[0]).write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                verify(root)

    def test_checksum_name_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); self.prepare(root, "agents")
            (root / "SHA256SUMS").write_text("0" * 64 + "  ../unexpected\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "invalid checksum filename"):
                verify(root, "agents")

    def test_preview_paths_are_explicitly_normalized(self):
        import tomllib
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "Sengoo.toml"
            manifest.write_text('[package]\nname="sgmcp"\n[dependencies]\nsgschema={path="../sgschema"}\n[[test]]\npath="tests/protocol.sg"\n', encoding="utf-8")
            self.assertTrue(normalize_agent_dependencies(root))
            parsed = tomllib.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(parsed["dependencies"]["sgschema"], {"version": "=0.1.0", "registry": "official"})
            self.assertEqual(parsed["test"][0]["path"], "tests/protocol.sg")
            manifest.write_text('[package]\nname="sgagent"\n[dependencies]\nsgschema={path="../sgschema"}\nsgsqlite={path="../sgsqlite"}\n', encoding="utf-8")
            self.assertTrue(normalize_agent_dependencies(root))
            parsed = tomllib.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(set(parsed["dependencies"]), {"sgschema", "sgsqlite"})
            self.assertTrue(all(d == {"version": "=0.1.0", "registry": "official"} for d in parsed["dependencies"].values()))
            manifest.write_text('[dependencies]\nunknown={path="D:/private/checkout"}\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unsupported agent preview dependency"):
                normalize_agent_dependencies(root)


if __name__ == "__main__": unittest.main()
