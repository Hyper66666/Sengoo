//! Python interoperability utilities for Sengoo runtime.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

/// Thin wrapper around a held Python GIL token.
pub struct PythonInterop<'py> {
    py: Python<'py>,
}

impl<'py> PythonInterop<'py> {
    /// Create interop wrapper from a held Python token.
    pub fn new(py: Python<'py>) -> Self {
        Self { py }
    }

    /// Import a Python module.
    pub fn import(&self, name: &str) -> Result<Py<PyAny>, PyErr> {
        self.py.import(name).map(|m| m.into_any().unbind())
    }

    /// Evaluate a Python expression.
    pub fn eval(&self, code: &str) -> Result<Py<PyAny>, PyErr> {
        let code = std::ffi::CString::new(code).unwrap();
        self.py.eval(&code, None, None).map(|obj| obj.unbind())
    }

    /// Call a Python callable with positional arguments.
    pub fn call(
        &self,
        func: &Py<PyAny>,
        args: impl IntoPyObject<'py, Target = PyTuple>,
    ) -> Result<Py<PyAny>, PyErr> {
        func.call(self.py, args, None)
    }
}

impl<'py> Default for PythonInterop<'py> {
    fn default() -> Self {
        // SAFETY: default construction is expected to happen under a held GIL.
        Self::new(unsafe { Python::assume_gil_acquired() })
    }
}

/// Convert Sengoo host values into Python objects.
pub trait ToPython {
    fn to_py(self, py: Python) -> PyObject;
}

/// Convert Python objects into Sengoo host values.
pub trait FromPython: Sized {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr>;
}

impl ToPython for i32 {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl ToPython for i64 {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl ToPython for f64 {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl ToPython for bool {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl ToPython for String {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl ToPython for &str {
    fn to_py(self, py: Python) -> PyObject {
        self.into_py(py)
    }
}

impl FromPython for i32 {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr> {
        obj.extract(py)
    }
}

impl FromPython for i64 {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr> {
        obj.extract(py)
    }
}

impl FromPython for f64 {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr> {
        obj.extract(py)
    }
}

impl FromPython for bool {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr> {
        obj.extract(py)
    }
}

impl FromPython for String {
    fn from_py(py: Python, obj: PyObject) -> Result<Self, PyErr> {
        obj.extract(py)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_eval() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let interp = PythonInterop::new(py);
            let result = interp.eval("1 + 1").unwrap();
            let value: i32 = result.extract(py).unwrap();
            assert_eq!(value, 2);
        });
    }
}
