//! Tests for `Module::get_function_table_entry` and element segment handling
//! with `ConstExpr::Global` and `ConstExpr::Extended` offsets.
//!
//! See `BUG_REPORT_ELEMENT_SEGMENT_OFFSET.md` at the repo root for full
//! context. In short: wasm-bindgen's `get_function_table_entry` only handled
//! `ConstExpr::Value(I32)` as an active element segment offset and silently
//! skipped segments with `Global` or `Extended` offsets, which rustc 1.94+ /
//! lld now emits for large WASM modules.
//!
//! These tests verify:
//!
//! 1. A module with a `ConstExpr::Global` element segment offset round-trips
//!    correctly through walrus emit+parse.
//! 2. A module with a `ConstExpr::Extended` (GlobalGet + I32Add) offset also
//!    round-trips correctly.
//! 3. `Module::get_function_table_entry` correctly resolves a table index that
//!    belongs to the *second* of two active segments, without integer underflow
//!    when computing the local index for the first segment.

use walrus::{
    ir::Value, ConstExpr, ConstOp, ElementItems, ElementKind, FunctionBuilder, Module,
    ModuleConfig, RefType, ValType,
};

// ---------------------------------------------------------------------------
// Test 1 — active element segment with ConstExpr::Global offset
// ---------------------------------------------------------------------------
/// Verifies that a `ConstExpr::Global` element segment offset survives a
/// walrus round-trip (emit → parse).
///
/// This mirrors the offset lld emits for large WASM modules:
/// `(elem (table 0) (global.get $__table_base) func ...)`.
#[test]
fn element_segment_with_global_offset() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    // Global acting as table base (mirrors __table_base).
    // Must be immutable — the Wasm spec only allows global.get of immutable
    // globals in constant expressions (element segment offsets).
    let base_global =
        module
            .globals
            .add_local(ValType::I32, false, false, ConstExpr::Value(Value::I32(1)));
    module.exports.add("__table_base", base_global);

    let builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("f", func_id);

    let table_id = module.tables.add_local(false, 2, None, RefType::FUNCREF);

    // Offset = global.get $base_global → ConstExpr::Global
    let elem_id = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Global(base_global),
        },
        ElementItems::Functions(vec![func_id]),
    );
    module
        .tables
        .get_mut(table_id)
        .elem_segments
        .insert(elem_id);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let table2 = module2
        .tables
        .main_function_table()
        .expect("main_function_table query failed")
        .expect("no function table found");
    let segments: Vec<_> = module2.tables.get(table2).elem_segments.iter().collect();
    assert_eq!(segments.len(), 1, "expected exactly one element segment");

    let seg = module2.elements.get(*segments[0]);
    assert!(
        matches!(
            seg.kind,
            ElementKind::Active {
                offset: ConstExpr::Global(_),
                ..
            }
        ),
        "offset should be ConstExpr::Global after round-trip, got: {:?}",
        seg.kind
    );
}

// ---------------------------------------------------------------------------
// Test 2 — active element segment with ConstExpr::Extended offset
// ---------------------------------------------------------------------------
/// Verifies that a `ConstExpr::Extended` element segment offset
/// (`global.get $base + i32.const 4`) survives a walrus round-trip.
///
/// lld emits this pattern when linking multiple object files:
/// `(elem (table 0) (global.get $__table_base) (i32.const K) i32.add func ...)`.
#[test]
fn element_segment_with_extended_const_offset() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    // Must be immutable for use in a constant expression.
    let base_global =
        module
            .globals
            .add_local(ValType::I32, false, false, ConstExpr::Value(Value::I32(0)));
    module.exports.add("__table_base", base_global);

    let builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("f", func_id);

    let table_id = module.tables.add_local(false, 8, None, RefType::FUNCREF);

    // Offset = global.get $base + i32.const 4 → ConstExpr::Extended
    let elem_id = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Extended(vec![
                ConstOp::GlobalGet(base_global),
                ConstOp::I32Const(4),
                ConstOp::I32Add,
            ]),
        },
        ElementItems::Functions(vec![func_id]),
    );
    module
        .tables
        .get_mut(table_id)
        .elem_segments
        .insert(elem_id);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let table2 = module2
        .tables
        .main_function_table()
        .expect("main_function_table query failed")
        .expect("no function table found");
    let segments: Vec<_> = module2.tables.get(table2).elem_segments.iter().collect();
    assert_eq!(segments.len(), 1, "expected exactly one element segment");

    let seg = module2.elements.get(*segments[0]);
    assert!(
        matches!(
            seg.kind,
            ElementKind::Active {
                offset: ConstExpr::Extended(_),
                ..
            }
        ),
        "offset should be ConstExpr::Extended after round-trip, got: {:?}",
        seg.kind
    );
}

// ---------------------------------------------------------------------------
// Test 3 — multi-segment table, correct lookup without underflow
// ---------------------------------------------------------------------------
/// Two active segments at offsets 0 (func_a) and 128 (func_b).
///
/// Verifies that `Module::get_function_table_entry` correctly resolves index
/// 128 to func_b and index 0 to func_a, and that the two are distinct.
///
/// The buggy wasm-bindgen code computed `(idx - offset) as usize` with plain
/// u32 arithmetic. When looking up idx=128 against segment A (offset=0), that
/// gives local_idx=128 which is out of bounds — fine. But if the segments were
/// ordered B then A, looking up idx=0 against segment B (offset=128) would
/// compute `(0u32 - 128u32) as usize` = a huge number in release mode, which
/// `.get()` returns None for — accidentally fine but semantically wrong, and a
/// panic in debug mode. The `checked_sub` guard in `get_function_table_entry`
/// makes the intent explicit.
#[test]
fn multi_segment_table_index_no_underflow() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let func_a = {
        let b = FunctionBuilder::new(&mut module.types, &[], &[]);
        let id = b.finish(vec![], &mut module.funcs);
        module.exports.add("func_a", id);
        id
    };
    let func_b = {
        let b = FunctionBuilder::new(&mut module.types, &[], &[]);
        let id = b.finish(vec![], &mut module.funcs);
        module.exports.add("func_b", id);
        id
    };

    let table_id = module.tables.add_local(false, 256, None, RefType::FUNCREF);

    // Segment A: offset 0 → func_a
    let seg_a = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Value(Value::I32(0)),
        },
        ElementItems::Functions(vec![func_a]),
    );
    module.tables.get_mut(table_id).elem_segments.insert(seg_a);

    // Segment B: offset 128 → func_b
    let seg_b = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Value(Value::I32(128)),
        },
        ElementItems::Functions(vec![func_b]),
    );
    module.tables.get_mut(table_id).elem_segments.insert(seg_b);

    // Round-trip so IDs are stable.
    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let found_128 = module2.get_function_table_entry(128);
    assert!(
        found_128.is_ok(),
        "lookup of table index 128 should succeed, got: {:?}",
        found_128
    );

    let found_0 = module2.get_function_table_entry(0);
    assert!(
        found_0.is_ok(),
        "lookup of table index 0 should succeed, got: {:?}",
        found_0
    );

    assert_ne!(
        found_0.unwrap(),
        found_128.unwrap(),
        "indices 0 and 128 should map to different functions"
    );
}
