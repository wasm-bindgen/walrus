//! Tests that the emitted type section order is deterministic (canonical)
//! regardless of the order types appear in the original binary.
//!
//! This is a regression test for:
//! https://github.com/wasm-bindgen/wasm-bindgen/issues/5065
//!
//! Different Rust compiler versions can emit the same set of types in different
//! orders in the wasm type section.  walrus now sorts the type section
//! canonically on emit so that two semantically-identical modules always
//! produce identical bytes even when the input type ordering differs.

use walrus::{Module, ValType};

/// Emit a walrus module to bytes and print the type section via wasmprinter.
fn emit_and_print(module: &mut Module) -> String {
    let wasm = module.emit_wasm();
    wasmprinter::print_bytes(&wasm).expect("should print wasm")
}

/// Extract just the type lines from a wasmprinter output.
fn type_lines(wat: &str) -> Vec<String> {
    wat.lines()
        .filter(|l| l.trim_start().starts_with("(type"))
        .map(|l| l.trim().to_string())
        .collect()
}

/// Build a minimal module containing two function types in the given order and
/// return the emitted type section lines.
fn module_with_types_in_order(
    first: (&[ValType], &[ValType]),
    second: (&[ValType], &[ValType]),
) -> Vec<String> {
    let mut m = Module::default();
    m.types.add(first.0, first.1);
    m.types.add(second.0, second.1);
    let wat = emit_and_print(&mut m);
    type_lines(&wat)
}

// ---------------------------------------------------------------------------
// Core determinism test
// ---------------------------------------------------------------------------

/// Two modules that contain the same pair of types but in opposite insertion
/// orders must emit an identical type section.
#[test]
fn same_types_different_insertion_order_produce_identical_output() {
    let ty_a = (&[ValType::I32, ValType::I32][..], &[ValType::I32][..]);
    let ty_b = (
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32][..],
        &[ValType::I32][..],
    );

    let order1 = module_with_types_in_order(ty_a, ty_b);
    let order2 = module_with_types_in_order(ty_b, ty_a);

    assert_eq!(
        order1, order2,
        "type section should be identical regardless of insertion order"
    );
}

/// Canonical order: types with fewer / simpler parameters come before types
/// with more / larger parameters.
#[test]
fn canonical_order_simpler_types_first() {
    let ty_small = (&[ValType::I32, ValType::I32][..], &[ValType::I32][..]);
    let ty_large = (
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32][..],
        &[ValType::I32][..],
    );

    // Insert large first, small second — canonical output should still put
    // the smaller type at index 0.
    let lines = module_with_types_in_order(ty_large, ty_small);
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].contains("(param i32 i32) (result i32)"),
        "smaller type should be at index 0, got: {:?}",
        lines[0]
    );
    assert!(
        lines[1].contains("(param i32 i32 i32 i32) (result i32)"),
        "larger type should be at index 1, got: {:?}",
        lines[1]
    );
}

// ---------------------------------------------------------------------------
// Regression: the exact case from wasm-bindgen issue #5065
// ---------------------------------------------------------------------------

/// Simulate the non-determinism seen in wasm-bindgen#5065: the two types used
/// by `__wbindgen_malloc` and `__wbindgen_realloc` appeared in swapped order
/// depending on which Rust compiler produced the wasm binary.
#[test]
fn wasm_bindgen_malloc_realloc_types_are_stable() {
    // malloc: (i32, i32) -> i32
    let malloc_ty = (&[ValType::I32, ValType::I32][..], &[ValType::I32][..]);
    // realloc: (i32, i32, i32, i32) -> i32
    let realloc_ty = (
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32][..],
        &[ValType::I32][..],
    );

    // Emit with malloc first (as one compiler would produce).
    let malloc_first = module_with_types_in_order(malloc_ty, realloc_ty);
    // Emit with realloc first (as another compiler would produce).
    let realloc_first = module_with_types_in_order(realloc_ty, malloc_ty);

    assert_eq!(
        malloc_first, realloc_first,
        "malloc/realloc type indices must be stable regardless of original binary type order"
    );

    // Additionally verify the canonical order: malloc's type (2 params) < realloc's type (4 params).
    assert!(
        malloc_first[0].contains("(param i32 i32) (result i32)"),
        "malloc type (2 params) should be at index 0"
    );
    assert!(
        malloc_first[1].contains("(param i32 i32 i32 i32) (result i32)"),
        "realloc type (4 params) should be at index 1"
    );
}

// ---------------------------------------------------------------------------
// Larger type set
// ---------------------------------------------------------------------------

/// A module with many types inserted in various orders always produces the
/// same sorted output.
#[test]
fn many_types_sorted_deterministically() {
    let types: &[(&[ValType], &[ValType])] = &[
        (&[], &[]),
        (&[ValType::I32], &[]),
        (&[], &[ValType::I32]),
        (&[ValType::I32, ValType::I32], &[]),
        (&[ValType::I64], &[ValType::I64]),
        (&[ValType::F32], &[ValType::F32]),
        (&[ValType::F64], &[ValType::F64]),
    ];

    // Build module inserting types in forward order.
    let mut m1 = Module::default();
    for (params, results) in types {
        m1.types.add(params, results);
    }
    let wat1 = emit_and_print(&mut m1);
    let lines1 = type_lines(&wat1);

    // Build module inserting types in reverse order.
    let mut m2 = Module::default();
    for (params, results) in types.iter().rev() {
        m2.types.add(params, results);
    }
    let wat2 = emit_and_print(&mut m2);
    let lines2 = type_lines(&wat2);

    assert_eq!(
        lines1, lines2,
        "type sections should be identical regardless of insertion order"
    );
    assert_eq!(lines1.len(), types.len());
}

// ---------------------------------------------------------------------------
// GC types: supertype ordering must be preserved
// ---------------------------------------------------------------------------

/// When type A is a supertype of type B, A must still appear before B in the
/// emitted output even after canonical sorting.
#[test]
fn supertype_ordering_respected() {
    use walrus::{CompositeType, FunctionType};

    let mut module = Module::default();

    // Create a base type: () -> ()
    let base_id = module.types.add(&[], &[]);

    // Create a sub-type that has base_id as supertype.
    // We use add_composite to set the supertype field.
    let sub_comp = CompositeType::Function(FunctionType::new(
        vec![ValType::I32].into_boxed_slice(),
        vec![].into_boxed_slice(),
    ));
    let sub_id = module.types.add_composite(sub_comp, true, Some(base_id));

    let wasm = module.emit_wasm();
    let wat = wasmprinter::print_bytes(&wasm).expect("should print");
    let lines = type_lines(&wat);

    assert_eq!(lines.len(), 2, "should have exactly 2 types");

    // The base type (index 0) must appear before the sub type (index 1).
    // wasmprinter prints `(type (;0;) ...)` and `(type (;1;) ...)`.
    assert!(
        lines[0].contains(";0;"),
        "first line should be type 0: {:?}",
        lines[0]
    );
    assert!(
        lines[1].contains(";1;"),
        "second line should be type 1: {:?}",
        lines[1]
    );

    // Verify the sub type references its supertype.
    // wasmprinter renders sub types with `(sub ...)` syntax.
    assert!(
        lines[1].contains("sub") || lines[1].contains("(param i32)"),
        "second type should be the subtype: {:?}",
        lines[1]
    );

    let _ = sub_id; // used above
}
