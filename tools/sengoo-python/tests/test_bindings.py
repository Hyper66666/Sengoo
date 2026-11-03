"""Boundary tests for the public source-build configuration and ctypes safety."""
import ctypes
import json
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

from sengoo_python import backend
from sengoo_python import runtime


class ConversionTests(unittest.TestCase):
    def test_integer_range_and_bool_separation(self):
        for value in (-(1 << 63), (1 << 63) - 1):
            self.assertEqual(runtime._i64(value), value)
        for value in (-(1 << 63) - 1, 1 << 63):
            with self.assertRaises(OverflowError):
                runtime._i64(value)
        for value in (True, 1.0, "1"):
            with self.assertRaises(TypeError):
                runtime._i64(value)

    def test_unicode_and_nul(self):
        slot, alive = runtime._Input(), []
        runtime._convert("你好 🐍", "string", slot, alive)
        self.assertEqual(ctypes.string_at(slot.data).decode(), "你好 🐍")
        with self.assertRaises(ValueError):
            runtime._convert("a\0b", "str", slot, alive)
        with self.assertRaises(UnicodeError):
            runtime._convert("\ud800", "str", slot, alive)

    def test_invalid_sequence_cannot_create_partial_native_resources(self):
        slot, alive = runtime._Input(), []
        with self.assertRaises(TypeError):
            runtime._convert([1, 2, "bad"], "vec_i64", slot, alive)
        self.assertEqual(alive, [])
        self.assertIsNone(slot.data)
        runtime._convert([], "vec_f64", slot, alive)
        self.assertEqual(slot.len, 0)

    def test_errors_preserve_explicit_status_code(self):
        output = runtime._Output(status=1, bits=987)
        with self.assertRaises(runtime.SengooError) as raised:
            runtime._copy_output(output, "i64")
        self.assertEqual(raised.exception.code, 987)
        with self.assertRaises(MemoryError):
            runtime._copy_output(runtime._Output(status=-1), "string")

    def test_float_bits_and_copied_array(self):
        bits = struct.unpack("=q", struct.pack("=d", -1.25))[0]
        self.assertEqual(runtime._copy_output(runtime._Output(bits=bits), "f64"), -1.25)
        buffer = (ctypes.c_double * 2)(1.5, 2.5)
        output = runtime._Output(data=ctypes.cast(buffer, ctypes.c_void_p), len=2)
        copied = runtime._copy_output(output, "vec_f64")
        buffer[0] = 99
        self.assertEqual(copied, [1.5, 2.5])

    def test_output_nul_and_invalid_utf8_fail_during_copy(self):
        for data, error in ((b"a\0b", ValueError), (b"\xff", UnicodeError)):
            buffer = ctypes.create_string_buffer(data)
            output = runtime._Output(data=ctypes.cast(buffer, ctypes.c_void_p), len=len(data))
            with self.assertRaises(error):
                runtime._copy_output(output, "string")

    def test_cleanup_runs_when_python_output_conversion_fails(self):
        class Native:
            def __call__(self, args, output):
                target = ctypes.cast(output, ctypes.POINTER(runtime._Output)).contents
                target.status = 1
                target.bits = 42
        class Abi:
            def __call__(self):
                return descriptor["identity"].encode()
        class Release:
            calls = 0
            def __call__(self, output):
                self.calls += 1
        release = Release()
        library = mock.Mock()
        setattr(library, "__sgpy_abi_v1", Abi())
        setattr(library, "__sgpy_release_v1", release)
        library.fn = Native()
        descriptor = {"schema_version": 1, "binding_abi_version": 1, "identity": "identity",
                      "functions": [{"name": "operation", "symbol": "fn", "params": [], "returns": "i64"}]}
        descriptor["identity"] = hashlib.sha256(json.dumps(descriptor["functions"], separators=(",", ":")).encode()).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "_bindings.json").write_text(json.dumps(descriptor))
            namespace = {"__name__": "fixture"}
            with mock.patch.object(runtime._ct, "PyDLL", return_value=library):
                runtime.install(namespace, root / "__init__.py", "native.so")
                with self.assertRaises(runtime.SengooError):
                    namespace["operation"]()
        self.assertEqual(release.calls, 1)

    def test_late_input_failure_never_invokes_native_or_release(self):
        descriptor = {"schema_version": 1, "binding_abi_version": 1, "functions": [
            {"name": "operation", "symbol": "fn", "params": [
                {"name": "label", "ty": "string"}, {"name": "values", "ty": "vec_i64"},
                {"name": "last", "ty": "i64"}], "returns": "unit"}]}
        descriptor["identity"] = hashlib.sha256(json.dumps(descriptor["functions"], separators=(",", ":")).encode()).hexdigest()
        library = mock.Mock()
        getattr(library, "__sgpy_abi_v1").return_value = descriptor["identity"].encode()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "_bindings.json").write_text(json.dumps(descriptor), encoding="utf-8")
            namespace = {"__name__": "fixture"}
            with mock.patch.object(runtime._ct, "PyDLL", return_value=library):
                runtime.install(namespace, root / "__init__.py", "native.so")
                for invalid, exception in ((True, TypeError), (1 << 63, OverflowError)):
                    with self.assertRaises(exception):
                        namespace["operation"]("already converted 世界", [1, 2, 3], invalid)
        library.fn.assert_not_called()
        getattr(library, "__sgpy_release_v1").assert_not_called()


class BackendTests(unittest.TestCase):
    def test_sdist_rejects_local_dependencies_that_would_be_omitted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Sengoo.lock").write_text("version = 2\n")
            (root / "src").mkdir()
            (root / "src/lib.sg").write_text("pub def value() -> i64 { 1 }")
            manifest = root / "Sengoo.toml"
            manifest.write_text('[lib]\npath = "src/lib.sg"\n[dependencies]\noutside = { path = "../outside" }\n')
            with self.assertRaisesRegex(ValueError, "inside the Python project"):
                backend._validate_sdist_inputs(root, manifest)
            manifest.write_text('[lib]\npath = "src/lib.sg"\n')
            backend._validate_sdist_inputs(root, manifest)

    def test_old_and_mismatched_toolchains_are_rejected(self):
        with mock.patch.object(backend, "_tool", return_value="sgc"):
            with mock.patch.object(backend.subprocess, "run", return_value=mock.Mock(stdout="sgpm 0.2.0 (abc)\n")):
                with self.assertRaisesRegex(RuntimeError, ">=0.2.1"):
                    backend._ensure_toolchain("sgpm")
            outputs = [mock.Mock(stdout="sgpm 0.2.1-dev.0 (abc)\n"), mock.Mock(stdout="sgc 0.2.1-dev.0 (def)\n")]
            with mock.patch.object(backend.subprocess, "run", side_effect=outputs):
                with self.assertRaisesRegex(RuntimeError, "same toolchain build"):
                    backend._ensure_toolchain("sgpm")

    def test_linux_tags_are_not_falsely_manylinux(self):
        with mock.patch.object(backend.sysconfig, "get_platform", return_value="linux-x86_64"):
            self.assertEqual(backend._platform_tag(), "linux_x86_64")

    def test_macos_tag_uses_binary_deployment_and_native_architecture(self):
        output = mock.Mock(stdout="Load command 8\n cmd LC_BUILD_VERSION\n cmdsize 32\n platform 1\n minos 11.0\n sdk 14.4\n")
        with mock.patch.object(backend.sys, "platform", "darwin"), mock.patch.object(backend.platform, "machine", return_value="arm64"), mock.patch.object(backend.subprocess, "run", return_value=output):
            self.assertEqual(backend._platform_tag(Path("native.dylib")), "macosx_11_0_arm64")

    def test_stub_contains_type_mappings(self):
        descriptor = {"functions": [{"name": "batch", "params": [{"name": "values", "ty": "vec_f64"}], "returns": "vec_i64"}]}
        self.assertIn("def batch(values: Sequence[float]) -> list[int]", backend._stub(descriptor))


if __name__ == "__main__":
    unittest.main()
