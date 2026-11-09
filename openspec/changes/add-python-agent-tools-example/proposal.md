# Add a Python consumer of Sengoo tool schemas

Use the existing typed Python wheel backend with the real sgschema package to
validate JSON Agent tool arguments. This standalone consumer demonstrates a
stable small ABI, exact diagnostics and offline binary use. It does not claim
whole-Agent bindings or add Python-specific execution to the Agent kernel.

All validation logic is Sengoo; Python is the consuming application and external
verification driver. No shared backend, compiler, native code or credentials
are changed by this example.
