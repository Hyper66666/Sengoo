"""Self-contained ctypes adapter copied into each generated wheel."""
import ctypes as _ct
import json as _json
import hashlib as _hashlib
from pathlib import Path as _Path
import struct as _struct


class SengooError(Exception):
    """An explicit Result<T, i64> failure from Sengoo."""
    def __init__(self, code):
        self.code = code
        super().__init__(f"Sengoo operation failed (code {code})")


class _Input(_ct.Structure):
    _fields_ = [("bits", _ct.c_int64), ("data", _ct.c_void_p), ("len", _ct.c_int64)]


class _Output(_ct.Structure):
    _fields_ = [("status", _ct.c_int64), ("bits", _ct.c_int64), ("data", _ct.c_void_p),
                ("len", _ct.c_int64), ("owner", _ct.c_int64), ("kind", _ct.c_int64)]


def _i64(value):
    if type(value) is not int:
        raise TypeError("expected int (bool is not accepted as i64)")
    if not -(1 << 63) <= value < (1 << 63):
        raise OverflowError("integer is outside the Sengoo i64 range")
    return value


def _f64(value):
    if type(value) not in (int, float):
        raise TypeError("expected float or int (bool is not accepted as f64)")
    return float(value)


def _convert(value, ty, slot, keepalive):
    if ty == "unit":
        if value is not None:
            raise TypeError("expected None")
    elif ty == "i64":
        slot.bits = _i64(value)
    elif ty == "f64":
        slot.bits = _struct.unpack("=q", _struct.pack("=d", _f64(value)))[0]
    elif ty == "bool":
        if type(value) is not bool:
            raise TypeError("expected bool")
        slot.bits = int(value)
    elif ty in ("str", "string"):
        if type(value) is not str:
            raise TypeError("expected str")
        if "\0" in value:
            raise ValueError("Sengoo strings cannot contain NUL")
        encoded = value.encode("utf-8", errors="strict")
        buffer = _ct.create_string_buffer(encoded)
        keepalive.append(buffer)
        slot.data = _ct.cast(buffer, _ct.c_void_p)
        slot.len = len(encoded)
    elif ty in ("vec_i64", "vec_f64"):
        if isinstance(value, (str, bytes, bytearray)):
            raise TypeError("expected a numeric sequence")
        try:
            values = list(value)
        except TypeError as error:
            raise TypeError("expected a numeric sequence") from error
        convert = _i64 if ty == "vec_i64" else _f64
        ctype = _ct.c_int64 if ty == "vec_i64" else _ct.c_double
        # Validate every element before any Sengoo memory is allocated.
        converted = [convert(item) for item in values]
        buffer = (ctype * len(converted))(*converted)
        keepalive.append(buffer)
        slot.data = _ct.cast(buffer, _ct.c_void_p)
        slot.len = len(converted)
    else:
        raise ImportError(f"unsupported parameter type in binding descriptor: {ty}")


def _copy_output(output, ty):
    if output.status == 1:
        raise SengooError(output.bits)
    if output.status:
        raise MemoryError("Sengoo binding conversion failed")
    if ty == "unit":
        return None
    if ty == "i64":
        return output.bits
    if ty == "f64":
        return _struct.unpack("=d", _struct.pack("=q", output.bits))[0]
    if ty == "bool":
        return bool(output.bits)
    if output.len < 0 or (output.len and not output.data):
        raise RuntimeError("invalid Sengoo output buffer")
    if ty == "string":
        value = _ct.string_at(output.data, output.len).decode("utf-8", errors="strict")
        if "\0" in value:
            raise ValueError("Sengoo strings cannot contain NUL")
        return value
    if ty in ("vec_i64", "vec_f64"):
        if not output.len:
            return []
        ctype = _ct.c_int64 if ty == "vec_i64" else _ct.c_double
        return list(_ct.cast(output.data, _ct.POINTER(ctype * output.len)).contents)
    raise ImportError(f"unsupported return type in binding descriptor: {ty}")


def install(namespace, filename, library_name):
    directory = _Path(filename).resolve().parent
    descriptor = _json.loads((directory / "_bindings.json").read_text(encoding="utf-8"))
    if descriptor.get("schema_version") != 1 or descriptor.get("binding_abi_version") != 1:
        raise ImportError("unsupported Sengoo binding ABI; rebuild this wheel")
    encoded = _json.dumps(descriptor["functions"], ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if _hashlib.sha256(encoded).hexdigest() != descriptor["identity"]:
        raise ImportError("Sengoo binding descriptor checksum does not match its functions")
    # PyDLL retains the GIL; RTLD_LOCAL is deliberate for independent modules.
    library = _ct.PyDLL(str(directory / library_name), mode=getattr(_ct, "RTLD_LOCAL", 0))
    abi = library.__sgpy_abi_v1
    abi.argtypes = []
    abi.restype = _ct.c_char_p
    if abi() != descriptor["identity"].encode("ascii"):
        raise ImportError("Sengoo library and binding descriptor do not match")
    release = library.__sgpy_release_v1
    release.argtypes = [_ct.POINTER(_Output)]
    release.restype = None

    def create(function):
        native = getattr(library, function["symbol"])
        native.argtypes = [_ct.POINTER(_Input), _ct.POINTER(_Output)]
        native.restype = None
        params = function["params"]

        def call(*args, **kwargs):
            if len(args) > len(params):
                raise TypeError(f"{function['name']} expects {len(params)} arguments")
            values = list(args)
            for index in range(len(args), len(params)):
                name = params[index]["name"]
                if name not in kwargs:
                    raise TypeError(f"missing argument: {name}")
                values.append(kwargs.pop(name))
            if kwargs:
                raise TypeError(f"unexpected or duplicate arguments: {', '.join(kwargs)}")
            inputs = (_Input * len(params))()
            keepalive = []
            for value, parameter, slot in zip(values, params, inputs):
                _convert(value, parameter["ty"], slot, keepalive)
            output = _Output()
            try:
                native(inputs, _ct.byref(output))
                return _copy_output(output, function["returns"])
            finally:
                # This bridge owns all native memory. Python only copies it.
                release(_ct.byref(output))

        call.__name__ = function["name"]
        call.__qualname__ = function["name"]
        call.__module__ = namespace["__name__"]
        call.__doc__ = "Generated synchronous Sengoo binding (GIL held)."
        return call

    namespace["SengooError"] = SengooError
    namespace["__all__"] = ["SengooError"]
    for function in descriptor["functions"]:
        namespace[function["name"]] = create(function)
        namespace["__all__"].append(function["name"])
