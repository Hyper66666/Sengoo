# Cast Semantics Fix - 2026-03-11

## Problem

The LLVM and JIT backends had inconsistent cast semantics for integer widening operations:

- **LLVM backend** (`compiler/src/codegen/mod.rs:1888`): Used `sext` (sign extension) for Int->Int widening
- **JIT backend** (`compiler/src/codegen/jit.rs:831`): Used `zext` (zero extension) for i32->i64 widening

This inconsistency meant that negative integers would be handled differently:
- LLVM: `-1` as i32 -> `-1` as i64 (sign preserved via sext)
- JIT: `-1` as i32 -> `4294967295` as i64 (treated as unsigned via zext)

## Solution

Fixed the JIT backend to use `sext` instead of `zext` for integer widening, matching the LLVM backend behavior.

### Changed File

**`compiler/src/codegen/jit.rs:831`**

```diff
-                                        "{} = zext i32 {} to i64\n",
+                                        "{} = sext i32 {} to i64\n",
```

## Cast Semantics Reference

The unified cast semantics across both backends are now:

| Source Type | Target Type | LLVM Instruction | Purpose |
|-------------|-------------|------------------|---------|
| Int(smaller) | Int(larger) | `sext` | Sign extension - preserves sign of negative numbers |
| Bool (i1) | Int | `zext` | Zero extension - bool is unsigned (0 or 1) |
| Int(larger) | Int(smaller) | `trunc` | Truncation - discards high bits |
| Int | Bool (i1) | `trunc` | Truncation to single bit |
| Float | Int | `fptosi` | Float to signed integer |
| Int | Float | `sitofp` | Signed integer to float |

### Code Locations

- **LLVM backend**: `compiler/src/codegen/mod.rs`
  - Lines 1888-1900: Int->Int sext
  - Lines 1968-1978: Bool->Int zext
  - Lines 1982-1992: Int->Bool trunc

- **JIT backend**: `compiler/src/codegen/jit.rs`
  - Line 831: i32->i64 sext (fixed)

## Testing

Added `compiler/src/tests/cast_semantics_tests.rs` with:

1. **Baseline test**: Verifies same-width operations compile correctly
2. **Documentation test**: Documents that mixed-width operations are currently rejected by the type checker
3. **Verification test**: Documents the expected cast semantics in code comments

### Current Type System Limitation

The Sengoo type checker currently requires exact type matches in binary operations. The `check_binary()` function (in `compiler/src/typeck/check.rs:1984`) calls `unify()` which enforces strict type equality.

Implicit integer widening only happens at the MIR lowering stage (in `reconcile_binary_operand_types()` at `compiler/src/mir/lowering.rs:2231`), but the type checker rejects mixed-width operations before they reach MIR.

This means:
- ✅ `let x: i64 = 10; let y: i64 = 20; x + y` - compiles
- ❌ `let x: i32 = 10; let y: i64 = 20; x + y` - rejected by type checker

The MIR lowering code is ready to handle mixed-width operations correctly, but the type checker needs to be relaxed to allow them through.

## Test Results

All 318 compiler tests pass, including the 3 new cast semantics tests.

```
test result: ok. 318 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

## Impact

This fix ensures consistent behavior between LLVM and JIT execution modes, preventing subtle bugs where negative integers would be misinterpreted when using the JIT backend.

## Future Work

To fully test cast semantics through source code compilation, the type checker would need to be modified to:

1. Allow mixed-width integer operations in `check_binary()`
2. Defer the actual type reconciliation to MIR lowering (which already handles it correctly)
3. Add comprehensive integration tests for mixed-width operations

The MIR lowering infrastructure is already in place and correct - only the type checker needs adjustment.
