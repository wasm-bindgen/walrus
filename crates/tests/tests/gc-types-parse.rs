//! Tests for parsing GC type definitions (struct, array, rec groups, sub types).
//!
//! These tests verify that walrus can parse wasm modules containing GC type
//! definitions into the correct internal representation. Full round-trip tests
//! (parse -> emit -> parse) will be added in Phase 3 when emission is implemented.

use walrus::{FieldType, HeapType, Module, RefType, StorageType, ValType};

/// Parse a WAT string into a walrus Module.
fn parse_wat(wat: &str) -> Module {
    let wasm = wat::parse_str(wat).expect("valid WAT");
    Module::from_buffer(&wasm).expect("should parse wasm with GC types")
}

// ---------------------------------------------------------------------------
// Struct types
// ---------------------------------------------------------------------------

#[test]
fn parse_simple_struct() {
    let module = parse_wat(
        r#"(module
            (type (struct (field i32) (field (mut f64))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let ty = types[0];
    assert!(ty.is_struct());
    assert!(ty.is_final); // default is final

    let st = ty.as_struct().unwrap();
    assert_eq!(st.fields.len(), 2);

    // field 0: immutable i32
    assert_eq!(st.fields[0].element_type, StorageType::Val(ValType::I32));
    assert!(!st.fields[0].mutable);

    // field 1: mutable f64
    assert_eq!(st.fields[1].element_type, StorageType::Val(ValType::F64));
    assert!(st.fields[1].mutable);
}

#[test]
fn parse_struct_with_ref_fields() {
    let module = parse_wat(
        r#"(module
            (type (struct
                (field funcref)
                (field (mut externref))
                (field anyref)
            ))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let st = types[0].as_struct().unwrap();
    assert_eq!(st.fields.len(), 3);

    assert_eq!(
        st.fields[0].element_type,
        StorageType::Val(ValType::Ref(RefType::FUNCREF))
    );
    assert!(!st.fields[0].mutable);

    assert_eq!(
        st.fields[1].element_type,
        StorageType::Val(ValType::Ref(RefType::EXTERNREF))
    );
    assert!(st.fields[1].mutable);

    assert_eq!(
        st.fields[2].element_type,
        StorageType::Val(ValType::Ref(RefType::ANYREF))
    );
}

#[test]
fn parse_struct_with_packed_fields() {
    let module = parse_wat(
        r#"(module
            (type (struct (field i8) (field (mut i16))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let st = types[0].as_struct().unwrap();
    assert_eq!(st.fields.len(), 2);

    assert_eq!(st.fields[0].element_type, StorageType::I8);
    assert!(!st.fields[0].mutable);

    assert_eq!(st.fields[1].element_type, StorageType::I16);
    assert!(st.fields[1].mutable);
}

#[test]
fn parse_empty_struct() {
    let module = parse_wat(
        r#"(module
            (type (struct))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let st = types[0].as_struct().unwrap();
    assert_eq!(st.fields.len(), 0);
}

// ---------------------------------------------------------------------------
// Array types
// ---------------------------------------------------------------------------

#[test]
fn parse_simple_array() {
    let module = parse_wat(
        r#"(module
            (type (array (mut i32)))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let ty = types[0];
    assert!(ty.is_array());
    assert!(ty.is_final);

    let at = ty.as_array().unwrap();
    assert_eq!(at.field.element_type, StorageType::Val(ValType::I32));
    assert!(at.field.mutable);
}

#[test]
fn parse_immutable_array() {
    let module = parse_wat(
        r#"(module
            (type (array f64))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    let at = types[0].as_array().unwrap();
    assert_eq!(at.field.element_type, StorageType::Val(ValType::F64));
    assert!(!at.field.mutable);
}

#[test]
fn parse_array_with_packed_type() {
    let module = parse_wat(
        r#"(module
            (type (array (mut i8)))
            (type (array i16))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    let a0 = types[0].as_array().unwrap();
    assert_eq!(a0.field.element_type, StorageType::I8);
    assert!(a0.field.mutable);

    let a1 = types[1].as_array().unwrap();
    assert_eq!(a1.field.element_type, StorageType::I16);
    assert!(!a1.field.mutable);
}

#[test]
fn parse_array_of_refs() {
    let module = parse_wat(
        r#"(module
            (type (array (mut anyref)))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    let at = types[0].as_array().unwrap();
    assert_eq!(
        at.field.element_type,
        StorageType::Val(ValType::Ref(RefType::ANYREF))
    );
    assert!(at.field.mutable);
}

// ---------------------------------------------------------------------------
// Sub types
// ---------------------------------------------------------------------------

#[test]
fn parse_sub_type_non_final() {
    let module = parse_wat(
        r#"(module
            (type (sub (struct (field i32))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let ty = types[0];
    assert!(!ty.is_final); // sub without "final" is non-final
    assert!(ty.supertype.is_none());
    assert!(ty.is_struct());
}

#[test]
fn parse_sub_final_type() {
    let module = parse_wat(
        r#"(module
            (type (sub final (struct (field i32))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    let ty = types[0];
    assert!(ty.is_final);
    assert!(ty.supertype.is_none());
    assert!(ty.is_struct());
}

#[test]
fn parse_sub_type_with_supertype() {
    let module = parse_wat(
        r#"(module
            (type $parent (sub (struct (field i32))))
            (type $child (sub $parent (struct (field i32) (field i64))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    let parent = types[0];
    assert!(!parent.is_final);
    assert!(parent.supertype.is_none());

    let child = types[1];
    assert!(!child.is_final);
    assert!(child.supertype.is_some());
    assert_eq!(child.supertype.unwrap(), parent.id());

    let child_st = child.as_struct().unwrap();
    assert_eq!(child_st.fields.len(), 2);
    assert_eq!(
        child_st.fields[0].element_type,
        StorageType::Val(ValType::I32)
    );
    assert_eq!(
        child_st.fields[1].element_type,
        StorageType::Val(ValType::I64)
    );
}

#[test]
fn parse_sub_final_with_supertype() {
    let module = parse_wat(
        r#"(module
            (type $parent (sub (struct (field i32))))
            (type (sub final $parent (struct (field i32) (field f32))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    let child = types[1];
    assert!(child.is_final);
    assert_eq!(child.supertype.unwrap(), types[0].id());
}

// ---------------------------------------------------------------------------
// Rec groups
// ---------------------------------------------------------------------------

#[test]
fn parse_explicit_rec_group_forward_refs() {
    let module = parse_wat(
        r#"(module
            (rec
                (type $a (struct (field (ref $b))))
                (type $b (struct (field (ref $a))))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    let type_a = types[0];
    let type_b = types[1];

    // type $a has a field referencing $b (non-nullable: `ref $b`)
    let sa = type_a.as_struct().unwrap();
    assert_eq!(sa.fields.len(), 1);
    match sa.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert!(!rt.nullable);
            assert_eq!(rt.heap_type, HeapType::Concrete(type_b.id()));
        }
        _ => panic!("expected ref type field"),
    }

    // type $b has a field referencing $a (non-nullable: `ref $a`)
    let sb = type_b.as_struct().unwrap();
    assert_eq!(sb.fields.len(), 1);
    match sb.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert!(!rt.nullable);
            assert_eq!(rt.heap_type, HeapType::Concrete(type_a.id()));
        }
        _ => panic!("expected ref type field"),
    }

    // Verify rec group structure
    let rec_groups = module.types.rec_groups();
    assert_eq!(rec_groups.len(), 1);
    assert_eq!(rec_groups[0].types.len(), 2);
    assert_eq!(rec_groups[0].types[0], type_a.id());
    assert_eq!(rec_groups[0].types[1], type_b.id());
}

#[test]
fn parse_rec_group_with_sub_types() {
    let module = parse_wat(
        r#"(module
            (rec
                (type $base (sub (struct (field i32))))
                (type $derived (sub $base (struct (field i32) (field (ref $base)))))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    let base = types[0];
    assert!(!base.is_final);
    assert!(base.supertype.is_none());

    let derived = types[1];
    assert!(!derived.is_final);
    assert_eq!(derived.supertype.unwrap(), base.id());

    let ds = derived.as_struct().unwrap();
    assert_eq!(ds.fields.len(), 2);
    match ds.fields[1].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert_eq!(rt.heap_type, HeapType::Concrete(base.id()));
        }
        _ => panic!("expected ref type field"),
    }
}

#[test]
fn parse_multiple_rec_groups() {
    let module = parse_wat(
        r#"(module
            (rec
                (type $a (struct (field i32)))
                (type $b (struct (field f64)))
            )
            (rec
                (type $c (struct (field (ref $a))))
                (type $d (struct (field (ref $b))))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 4);

    let rec_groups = module.types.rec_groups();
    assert_eq!(rec_groups.len(), 2);
    assert_eq!(rec_groups[0].types.len(), 2);
    assert_eq!(rec_groups[1].types.len(), 2);

    // $c references $a (from first rec group)
    let sc = types[2].as_struct().unwrap();
    match sc.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert_eq!(rt.heap_type, HeapType::Concrete(types[0].id()));
        }
        _ => panic!("expected ref type"),
    }

    // $d references $b (from first rec group)
    let sd = types[3].as_struct().unwrap();
    match sd.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert_eq!(rt.heap_type, HeapType::Concrete(types[1].id()));
        }
        _ => panic!("expected ref type"),
    }
}

// ---------------------------------------------------------------------------
// Mixed type kinds
// ---------------------------------------------------------------------------

#[test]
fn parse_mixed_func_struct_array() {
    let module = parse_wat(
        r#"(module
            (type $s (struct (field i32)))
            (type $a (array (mut i8)))
            (type $f (func (param (ref $s)) (result i32)))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 3);

    assert!(types[0].is_struct());
    assert!(types[1].is_array());
    assert!(types[2].is_function());

    // The function type has a concrete ref param
    let ft = types[2].as_function().unwrap();
    assert_eq!(ft.params().len(), 1);
    assert_eq!(ft.results().len(), 1);
    match ft.params()[0] {
        ValType::Ref(rt) => {
            assert!(!rt.nullable); // (ref $s) is non-nullable
            assert_eq!(rt.heap_type, HeapType::Concrete(types[0].id()));
        }
        _ => panic!("expected ref param"),
    }
}

#[test]
fn parse_struct_referencing_later_type() {
    // A struct at index 0 referencing an array at index 1 (backward is always fine).
    // Note: forward refs across singleton rec groups are NOT allowed by the spec,
    // but later type referencing earlier type is always valid.
    let module = parse_wat(
        r#"(module
            (type $arr (array (mut i32)))
            (type $s (struct (field (ref $arr))))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 2);

    assert!(types[0].is_array());
    assert!(types[1].is_struct());

    let st = types[1].as_struct().unwrap();
    match st.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert_eq!(rt.heap_type, HeapType::Concrete(types[0].id()));
        }
        _ => panic!("expected ref type field"),
    }
}

// ---------------------------------------------------------------------------
// Rec group tracking
// ---------------------------------------------------------------------------

#[test]
fn singleton_types_get_individual_rec_groups() {
    let module = parse_wat(
        r#"(module
            (type (func (param i32) (result i32)))
            (type (struct (field f32)))
            (type (array (mut i64)))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 3);

    // Each singleton type gets its own rec group
    let rec_groups = module.types.rec_groups();
    assert_eq!(rec_groups.len(), 3);
    for rg in rec_groups {
        assert_eq!(rg.types.len(), 1);
    }
}

#[test]
fn rec_group_for_type_lookup() {
    let module = parse_wat(
        r#"(module
            (type $solo (func))
            (rec
                (type $a (struct (field i32)))
                (type $b (struct (field i64)))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();

    // $solo's rec group should have 1 member
    let solo_rg = module.types.rec_group_for_type(types[0].id()).unwrap();
    assert_eq!(solo_rg.types.len(), 1);

    // $a and $b should share the same rec group with 2 members
    let a_rg = module.types.rec_group_for_type(types[1].id()).unwrap();
    assert_eq!(a_rg.types.len(), 2);

    let b_rg = module.types.rec_group_for_type(types[2].id()).unwrap();
    assert_eq!(b_rg.types.len(), 2);
    assert_eq!(a_rg.types, b_rg.types);
}

// ---------------------------------------------------------------------------
// Concrete ref type in various positions
// ---------------------------------------------------------------------------

#[test]
fn parse_nullable_vs_nonnullable_concrete_refs() {
    let module = parse_wat(
        r#"(module
            (type $s (struct (field i32)))
            (type (struct
                (field (ref $s))
                (field (ref null $s))
            ))
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    let outer = types[1].as_struct().unwrap();

    // (ref $s) - non-nullable
    match outer.fields[0].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert!(!rt.nullable);
            assert_eq!(rt.heap_type, HeapType::Concrete(types[0].id()));
        }
        _ => panic!("expected ref type"),
    }

    // (ref null $s) - nullable
    match outer.fields[1].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert!(rt.nullable);
            assert_eq!(rt.heap_type, HeapType::Concrete(types[0].id()));
        }
        _ => panic!("expected ref type"),
    }
}

// ---------------------------------------------------------------------------
// Complex / realistic scenarios
// ---------------------------------------------------------------------------

#[test]
fn parse_linked_list_type() {
    let module = parse_wat(
        r#"(module
            (rec
                (type $node (sub (struct
                    (field $val i32)
                    (field $next (mut (ref null $node)))
                )))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 1);

    let node = types[0];
    assert!(!node.is_final); // sub = non-final
    assert!(node.supertype.is_none());

    let st = node.as_struct().unwrap();
    assert_eq!(st.fields.len(), 2);

    // field $val: i32
    assert_eq!(st.fields[0].element_type, StorageType::Val(ValType::I32));
    assert!(!st.fields[0].mutable);

    // field $next: (mut (ref null $node)) - self-referential
    assert_eq!(
        st.fields[1],
        FieldType {
            element_type: StorageType::Val(ValType::Ref(RefType {
                nullable: true,
                heap_type: HeapType::Concrete(node.id()),
            })),
            mutable: true,
        }
    );
}

#[test]
fn parse_tree_with_inheritance() {
    let module = parse_wat(
        r#"(module
            (rec
                (type $tree (sub (struct
                    (field $val i32)
                )))
                (type $leaf (sub $tree (struct
                    (field $val i32)
                    (field $data f64)
                )))
                (type $branch (sub $tree (struct
                    (field $val i32)
                    (field $left (ref null $tree))
                    (field $right (ref null $tree))
                )))
            )
        )"#,
    );

    let types: Vec<_> = module.types.iter().collect();
    assert_eq!(types.len(), 3);

    let tree = types[0];
    let leaf = types[1];
    let branch = types[2];

    // $tree: no supertype
    assert!(tree.supertype.is_none());
    assert!(!tree.is_final);

    // $leaf extends $tree
    assert_eq!(leaf.supertype.unwrap(), tree.id());
    let leaf_st = leaf.as_struct().unwrap();
    assert_eq!(leaf_st.fields.len(), 2);

    // $branch extends $tree, has refs to $tree
    assert_eq!(branch.supertype.unwrap(), tree.id());
    let branch_st = branch.as_struct().unwrap();
    assert_eq!(branch_st.fields.len(), 3);
    match branch_st.fields[1].element_type {
        StorageType::Val(ValType::Ref(rt)) => {
            assert!(rt.nullable);
            assert_eq!(rt.heap_type, HeapType::Concrete(tree.id()));
        }
        _ => panic!("expected ref type"),
    }

    // Verify rec group
    let rec_groups = module.types.rec_groups();
    assert_eq!(rec_groups.len(), 1);
    assert_eq!(rec_groups[0].types.len(), 3);
}
