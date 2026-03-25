//! Tests for `ModuleTypes::delete`, `try_delete`, and `delete_entire_group`.
//!
//! These APIs were added in PR #304 alongside the GC extension support.
//! None of them had any test coverage prior to this file.

use walrus::{
    CompositeType, FieldType, FunctionBuilder, HeapType, Module, ModuleConfig, RefType,
    StorageType, StructType, ValType,
};

fn make_module() -> Module {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    Module::with_config(config)
}

// ---------------------------------------------------------------------------
// delete()
// ---------------------------------------------------------------------------

/// Deleting a standalone type removes it from the arena and from its
/// singleton rec group (so the group becomes empty and is skipped at emit).
#[test]
fn test_delete_singleton_type() {
    let mut module = make_module();

    let ty = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    // Must be present before deletion.
    assert!(module.types.rec_group_for_type(ty).is_some());

    module.types.delete(ty);

    // After deletion the group has no members.
    // rec_group_for_type() returns None because the type is no longer indexed.
    assert!(
        module.types.rec_group_for_type(ty).is_none(),
        "deleted type should have no rec group entry"
    );

    // The type should not appear in rec_groups either.
    for rg in module.types.rec_groups() {
        assert!(
            !rg.types.contains(&ty),
            "deleted type should not appear in any rec group"
        );
    }
}

/// Deleting one type from a multi-type rec group via `delete()` removes just
/// that type, leaving the group with the remaining member.
#[test]
fn test_delete_one_member_of_rec_group() {
    let mut module = make_module();

    let ids = module.types.add_rec_group(2, |ids| {
        let _a = ids[0];
        let b = ids[1];
        // $a references $b
        let a_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(b),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        // $b is a simple struct (no back-ref to $a)
        let b_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        vec![(a_def, true, None), (b_def, true, None)]
    });

    let a_id = ids[0];
    let b_id = ids[1];

    // Both should be in the same rec group initially.
    let rg_before = module.types.rec_group_for_type(a_id).unwrap();
    assert_eq!(rg_before.types.len(), 2);

    // Delete $a (caller's responsibility: no external refs to $a).
    module.types.delete(a_id);

    // $b should still be in the group; $a should be gone.
    assert!(module.types.rec_group_for_type(a_id).is_none());
    let rg_after = module.types.rec_group_for_type(b_id).unwrap();
    assert_eq!(rg_after.types.len(), 1);
    assert_eq!(rg_after.types[0], b_id);
}

// ---------------------------------------------------------------------------
// try_delete()
// ---------------------------------------------------------------------------

/// `try_delete` on a type that is referenced by a sibling in the same rec
/// group should return `false` and leave the type intact.
#[test]
fn test_try_delete_blocked_by_sibling_ref() {
    let mut module = make_module();

    // (rec
    //   (type $a (struct (field (ref null $b))))
    //   (type $b (struct (field i32)))
    // )
    // $a references $b — so try_delete($b) should fail.
    let ids = module.types.add_rec_group(2, |ids| {
        let b = ids[1];
        let a_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(b),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        let b_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        vec![(a_def, true, None), (b_def, true, None)]
    });

    let a_id = ids[0];
    let b_id = ids[1];

    // $a references $b, so deleting $b should be blocked.
    let deleted = module.types.try_delete(b_id);
    assert!(
        !deleted,
        "try_delete should return false when a sibling references the type"
    );

    // $b should still exist.
    assert!(module.types.rec_group_for_type(b_id).is_some());
    let rg = module.types.rec_group_for_type(a_id).unwrap();
    assert_eq!(rg.types.len(), 2);
}

/// `try_delete` on a self-referencing type (linked list node) should succeed,
/// because self-references don't prevent deletion.
#[test]
fn test_try_delete_self_referencing_type() {
    let mut module = make_module();

    // (rec
    //   (type $node (struct (field (ref null $node)) (field i32)))
    // )
    let ids = module.types.add_rec_group(1, |ids| {
        let node = ids[0];
        let def = CompositeType::Struct(StructType {
            fields: vec![
                FieldType {
                    element_type: StorageType::Val(ValType::Ref(RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(node),
                    })),
                    mutable: false,
                },
                FieldType {
                    element_type: StorageType::Val(ValType::I32),
                    mutable: false,
                },
            ]
            .into_boxed_slice(),
        });
        vec![(def, false, None)]
    });

    let node_id = ids[0];

    // Self-references are allowed; deletion should succeed.
    let deleted = module.types.try_delete(node_id);
    assert!(
        deleted,
        "try_delete should succeed for a self-referencing type"
    );
    assert!(module.types.rec_group_for_type(node_id).is_none());
}

/// `try_delete` on a standalone type (no rec group, no siblings) always succeeds.
#[test]
fn test_try_delete_standalone_type() {
    let mut module = make_module();

    let ty = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    let deleted = module.types.try_delete(ty);
    assert!(deleted);
    assert!(module.types.rec_group_for_type(ty).is_none());
}

/// `try_delete` on a type with no siblings (singleton rec group) always succeeds.
#[test]
fn test_try_delete_singleton_rec_group() {
    let mut module = make_module();

    // Explicit singleton rec group.
    let ids = module.types.add_rec_group(1, |_ids| {
        let def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        vec![(def, true, None)]
    });

    let ty = ids[0];
    let deleted = module.types.try_delete(ty);
    assert!(deleted);
}

// ---------------------------------------------------------------------------
// delete_entire_group()
// ---------------------------------------------------------------------------

/// `delete_entire_group` removes every type in a multi-type rec group.
#[test]
fn test_delete_entire_group_multi_type() {
    let mut module = make_module();

    // (rec
    //   (type $a (struct (field (ref null $b))))
    //   (type $b (struct (field (ref null $a))))
    // )
    let ids = module.types.add_rec_group(2, |ids| {
        let a = ids[0];
        let b = ids[1];
        let a_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(b),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        let b_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(a),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        vec![(a_def, true, None), (b_def, true, None)]
    });

    let a_id = ids[0];
    let b_id = ids[1];

    // Delete the entire group via $a.
    module.types.delete_entire_group(a_id);

    // Both types should be gone.
    assert!(module.types.rec_group_for_type(a_id).is_none());
    assert!(module.types.rec_group_for_type(b_id).is_none());

    // The group's types vec should be empty.
    for rg in module.types.rec_groups() {
        assert!(!rg.types.contains(&a_id));
        assert!(!rg.types.contains(&b_id));
    }
}

/// `delete_entire_group` on a singleton type (no group) just deletes it.
#[test]
fn test_delete_entire_group_singleton() {
    let mut module = make_module();

    let ty = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    module.types.delete_entire_group(ty);
    assert!(module.types.rec_group_for_type(ty).is_none());
}

/// After `delete_entire_group`, the module round-trips cleanly without
/// emitting the deleted types.
#[test]
fn test_delete_entire_group_does_not_appear_in_output() {
    let mut module = make_module();

    // Add a type to be deleted.
    let doomed_ids = module.types.add_rec_group(2, |ids| {
        let a = ids[0];
        let b = ids[1];
        let a_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(b),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        let b_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(a),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });
        vec![(a_def, true, None), (b_def, true, None)]
    });

    // Also add a "survivor" type that we'll keep.
    let survivor = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    // Export a function that uses the survivor so the GC pass keeps it.
    let survivor_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(survivor),
    });
    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[survivor_ref]);
    builder.func_body().struct_new_default(survivor);
    let fid = builder.finish(vec![], &mut module.funcs);
    module.exports.add("make", fid);

    // Delete the doomed group before emitting.
    module.types.delete_entire_group(doomed_ids[0]);

    // Emit and parse back.
    let wasm = module.emit_wasm();
    let parsed = Module::from_buffer(&wasm).expect("should parse after group deletion");

    // Only the survivor struct type (and the function type) should remain.
    // Doomed types must be absent.
    let types: Vec<_> = parsed.types.iter().collect();
    for ty in &types {
        if let Some(st) = ty.as_struct() {
            // If a struct has a ref field pointing to another struct, it's the
            // doomed linked pair — they should NOT appear.
            for field in st.fields.iter() {
                if let StorageType::Val(ValType::Ref(rt)) = field.element_type {
                    if let HeapType::Concrete(_) = rt.heap_type {
                        // The survivor's fields are plain i32 — no concrete refs.
                        panic!("found unexpected concrete ref in emitted type: {:?}", ty);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GC pass: ref.test/ref.cast heap types keep referenced types alive
// ---------------------------------------------------------------------------

/// A struct type only referenced in a `ref.test` instruction must survive
/// the GC pass. This exercises the `visit_heap_type` auto-generation added
/// in the `address feedback` commit.
#[test]
fn test_ref_test_concrete_keeps_type_alive() {
    let mut module = make_module();

    // (type $s (struct (field i32)))
    let s_ty = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    // (func (export "test") (param anyref) (result i32)
    //   local.get 0
    //   ref.test (ref null $s)
    // )
    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::I32],
    );
    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_test(true, HeapType::Concrete(s_ty));
    let fid = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("test", fid);

    // Run GC pass (via emit_wasm, which calls gc internally on non-raw emit).
    // But walrus::passes::gc is called when running Module::emit_wasm via
    // walrus's default pipeline. We trigger it explicitly.
    walrus::passes::gc::run(&mut module);

    // $s must still be alive.
    assert!(
        module.types.iter().any(|t| t.id() == s_ty),
        "struct type only referenced in ref.test must survive GC"
    );

    // Verify the module round-trips cleanly.
    let wasm = module.emit_wasm();
    let _ = Module::from_buffer(&wasm).expect("round-trip after GC should succeed");
}

/// A struct type only referenced in a `ref.cast` instruction must survive GC.
#[test]
fn test_ref_cast_concrete_keeps_type_alive() {
    let mut module = make_module();

    let s_ty = module.types.add_struct(vec![FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: false,
    }]);

    let s_ref = ValType::Ref(RefType {
        nullable: false,
        heap_type: HeapType::Concrete(s_ty),
    });

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[s_ref],
    );
    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_cast(false, HeapType::Concrete(s_ty));
    let fid = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("cast", fid);

    walrus::passes::gc::run(&mut module);

    assert!(
        module.types.iter().any(|t| t.id() == s_ty),
        "struct type only referenced in ref.cast must survive GC"
    );
}

/// Types referenced only in `br_on_cast` (from and to heap types) must survive GC.
#[test]
fn test_br_on_cast_heap_types_keep_types_alive() {
    let mut module = make_module();

    // Base (non-final) and derived (final, extends base)
    let base_ty = module.types.add_composite(
        CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            }]
            .into_boxed_slice(),
        }),
        false, // non-final
        None,
    );
    let derived_ty = module.types.add_composite(
        CompositeType::Struct(StructType {
            fields: vec![
                FieldType {
                    element_type: StorageType::Val(ValType::I32),
                    mutable: false,
                },
                FieldType {
                    element_type: StorageType::Val(ValType::F64),
                    mutable: false,
                },
            ]
            .into_boxed_slice(),
        }),
        true,
        Some(base_ty),
    );

    let base_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(base_ty),
    });
    let derived_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(derived_ty),
    });

    // (func (export "cast") (param (ref null $base)) (result i32)
    //   (block $is_derived (result (ref null $derived))
    //     local.get 0
    //     br_on_cast $is_derived (ref null $base) (ref null $derived)
    //     drop
    //     i32.const 0
    //     return
    //   )
    //   drop
    //   i32.const 1
    // )
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[base_ref], &[ValType::I32]);
        let arg = module.locals.add(base_ref);
        let block_seq_id = {
            let b = builder.dangling_instr_seq(derived_ref);
            b.id()
        };

        builder
            .instr_seq(block_seq_id)
            .local_get(arg)
            .br_on_cast(
                block_seq_id,
                true,
                HeapType::Concrete(base_ty),
                true,
                HeapType::Concrete(derived_ty),
            )
            .drop()
            .i32_const(0)
            .return_();

        use walrus::ir::Block;
        builder
            .func_body()
            .instr(Block { seq: block_seq_id })
            .drop()
            .i32_const(1);

        let fid = builder.finish(vec![arg], &mut module.funcs);
        module.exports.add("cast", fid);
    }

    walrus::passes::gc::run(&mut module);

    assert!(
        module.types.iter().any(|t| t.id() == base_ty),
        "base type only used in br_on_cast must survive GC"
    );
    assert!(
        module.types.iter().any(|t| t.id() == derived_ty),
        "derived type only used in br_on_cast must survive GC"
    );
}

// ---------------------------------------------------------------------------
// call_ref / return_call_ref round-trip
// ---------------------------------------------------------------------------

/// Build a module using `call_ref` and verify it round-trips correctly.
#[test]
fn test_call_ref_round_trip() {
    let mut module = make_module();

    // (type $fn_ty (func (param i32) (result i32)))
    let fn_ty = module.types.add(&[ValType::I32], &[ValType::I32]);

    // (func $double (param i32) (result i32) local.get 0 local.get 0 i32.add)
    let double_fid = {
        let mut b = FunctionBuilder::new(&mut module.types, &[ValType::I32], &[ValType::I32]);
        let x = module.locals.add(ValType::I32);
        use walrus::ir::BinaryOp;
        b.func_body()
            .local_get(x)
            .local_get(x)
            .binop(BinaryOp::I32Add);
        b.finish(vec![x], &mut module.funcs)
    };

    // (func $apply (param (ref null $fn_ty) i32) (result i32)
    //   local.get 1
    //   local.get 0
    //   call_ref $fn_ty
    // )
    let func_ref_ty = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(fn_ty),
    });
    let apply_fid = {
        let mut b = FunctionBuilder::new(
            &mut module.types,
            &[func_ref_ty, ValType::I32],
            &[ValType::I32],
        );
        let f = module.locals.add(func_ref_ty);
        let n = module.locals.add(ValType::I32);
        b.func_body().local_get(n).local_get(f).call_ref(fn_ty);
        b.finish(vec![f, n], &mut module.funcs)
    };

    module.exports.add("double", double_fid);
    module.exports.add("apply", apply_fid);

    let wasm = module.emit_wasm();
    let parsed = Module::from_buffer(&wasm).expect("call_ref round-trip should succeed");

    // Verify we have two exported functions.
    let export_names: Vec<_> = parsed.exports.iter().map(|e| e.name.as_str()).collect();
    assert!(export_names.contains(&"double"));
    assert!(export_names.contains(&"apply"));
}
