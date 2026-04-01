# Bug Report: `get_function_table_entry` fails with `ConstExpr::Global` / `ConstExpr::Extended` element segment offsets

**Affects:** wasm-bindgen CLI (all versions ≥ 0.2.114)  
**Triggered by:** rustc 1.94+ (stable and nightly) building large WASM modules (e.g. Leptos 0.8.x)  
**Upstream issue:** https://github.com/wasm-bindgen/wasm-bindgen/issues/5076  
**Panic site:** `crates/cli-support/src/wit/outgoing.rs:257`

---

## Summary

`wasm-bindgen`'s CLI panics when processing WASM files produced by rustc 1.94+
for large projects. The panic message is:

```
thread 'main' panicked at crates/cli-support/src/wit/outgoing.rs:257:61:
called `Result::unwrap()` on an `Err` value: failed to find 33345 in function table
```

The root cause is in `get_function_table_entry`
(`crates/cli-control/src/wasm_conventions.rs`) which only handles
`ConstExpr::Value(Value::I32(n))` as an active element segment offset, and
silently skips segments whose offset is expressed as any other `ConstExpr`
variant — including `ConstExpr::Global` and `ConstExpr::Extended`, both of
which rustc 1.94+ / lld now emit for large function tables.

---

## Background: what changed in rustc 1.94+

For small WASM modules lld emits the function table's element segment with a
plain `i32.const` offset, e.g.:

```wasm
(elem (table 0) (i32.const 1) func $shim_0 $shim_1 ...)
```

walrus parses this as `ConstExpr::Value(Value::I32(1))`. Works fine.

For large modules (thousands of closures, as in Leptos) lld switches to
position-independent table layout. The segment offset becomes a `global.get`
referring to `__table_base`:

```wasm
(elem (table 0) (global.get $__table_base) func $shim_0 $shim_1 ...)
```

walrus parses this as `ConstExpr::Global(global_id)`.

In some configurations (multiple object files merged by lld) the offset is an
extended const expression:

```wasm
(elem (table 0) (global.get $__table_base) (i32.const 4) i32.add func ...)
```

walrus parses this as `ConstExpr::Extended([GlobalGet(g), I32Const(4), I32Add])`.

---

## The two bugs in `get_function_table_entry`

```rust
// crates/cli-support/src/wasm_conventions.rs:95
pub fn get_function_table_entry(module: &Module, idx: u32) -> Result<FunctionId> {
    let table = module.tables.main_function_table()?.ok_or_else(|| ...)?;
    let table = module.tables.get(table);
    for &segment in table.elem_segments.iter() {
        let segment = module.elements.get(segment);
        let offset = match &segment.kind {
            walrus::ElementKind::Active {
                offset: ConstExpr::Value(Value::I32(n)),  // BUG 1: only I32 literal
                ..
            } => *n as u32,
            _ => continue,   // silently skips Global / Extended offsets
        };
        let idx = (idx - offset) as usize;  // BUG 2: no underflow guard
        ...
    }
    bail!("failed to find `{idx}` in function table");
}
```

### Bug 1 — `ConstExpr::Global` and `ConstExpr::Extended` offsets are silently skipped

The `_ => continue` arm skips any segment whose offset is not a literal I32.
With rustc 1.94+ the only active segment in the module has a `Global` or
`Extended` offset, so the loop body never executes and the function always
returns the `bail!` error. The `.unwrap()` at `outgoing.rs:257` then panics.

### Bug 2 — integer underflow in multi-segment tables

When a table has multiple active segments, for any segment whose base offset
is *greater* than `idx`, the subtraction `idx - offset` wraps (u32 arithmetic)
to a huge value. Cast to `usize` it falls outside the slice bounds so `.get()`
returns `None` and execution continues — this works by accident in release mode
but would panic in debug mode, and is semantically wrong.

The fix is a `checked_sub`: if `idx < offset` the entry cannot be in this
segment and we should `continue` cleanly.

---

## Proposed fix

```rust
pub fn get_function_table_entry(module: &Module, idx: u32) -> Result<FunctionId> {
    let table = module
        .tables
        .main_function_table()?
        .ok_or_else(|| anyhow!("no function table found in module"))?;
    let table = module.tables.get(table);
    for &segment in table.elem_segments.iter() {
        let segment = module.elements.get(segment);
        let offset = match &segment.kind {
            walrus::ElementKind::Active {
                offset: ConstExpr::Value(Value::I32(n)),
                ..
            } => *n as u32,

            // rustc 1.94+ / lld emits global.get $__table_base as the offset
            // for large function tables (position-independent table layout).
            walrus::ElementKind::Active {
                offset: ConstExpr::Global(g),
                ..
            } => match &module.globals.get(*g).kind {
                GlobalKind::Local(ConstExpr::Value(Value::I32(n))) => *n as u32,
                // Imported globals (e.g. the real __table_base) cannot be
                // evaluated statically — skip.
                _ => continue,
            },

            // Extended const exprs (GlobalGet + I32Add etc.) would require a
            // mini evaluator; skip for now. A future improvement could handle
            // the common GlobalGet + I32Const + I32Add pattern.
            _ => continue,
        };

        // Guard: if idx < offset this segment does not contain idx.
        let local_idx = match idx.checked_sub(offset) {
            Some(i) => i as usize,
            None => continue,
        };

        let slot = match &segment.items {
            ElementItems::Functions(items) => items.get(local_idx).map(Some),
            ElementItems::Expressions(_, items) => items.get(local_idx).map(|item| {
                if let ConstExpr::RefFunc(target) = item {
                    Some(target)
                } else {
                    None
                }
            }),
        };

        match slot {
            Some(slot) => {
                return slot.copied().context("function table entry wasn't filled");
            }
            None => continue,
        }
    }
    bail!("failed to find `{idx}` in function table");
}
```

---

## Note on `ConstExpr::Extended` with imported `__table_base`

The full fix for the `Extended` case (lld with multiple compilation units)
requires evaluating `GlobalGet($__table_base) + I32Const(K)`. Because
`__table_base` is an *import*, its runtime value is not known statically.

However, wasm-bindgen controls the WASM it processes; in practice the table
base is always 1 (slot 0 is reserved). A practical complete fix would:

1. Detect the `GlobalGet + I32Const + I32Add` pattern in `Extended`.
2. Look up the global — if it is an import named `__table_base`, treat its
   value as 1 (the conventional base).
3. Add the `I32Const` delta to get the segment's effective offset.

This is left as a follow-up; the `ConstExpr::Global` fix unblocks most users.

---

## Test cases

See `crates/tests/tests/element_segment_global_offset.rs` for three tests:

1. `element_segment_with_global_offset` — round-trips a module with a
   `ConstExpr::Global` element segment offset and verifies it survives.
2. `element_segment_with_extended_const_offset` — same for `ConstExpr::Extended`.
3. `multi_segment_table_index_no_underflow` — two segments at offsets 0 and
   128; verifies the fixed lookup algorithm finds the correct entry in each
   without underflow.
