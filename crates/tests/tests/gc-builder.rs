//! Tests for GC instruction builder methods and programmatic module construction.

use walrus::ir::*;
use walrus::{
    AbstractHeapType, CompositeType, ConstExpr, FieldType, FunctionBuilder, HeapType, Module,
    ModuleConfig, RefType, StorageType, StructType, ValType,
};

fn round_trip(module: &mut Module) -> Module {
    let wasm = module.emit_wasm();
    Module::from_buffer(&wasm).expect("should parse emitted wasm")
}

#[test]
fn test_ref_i31_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::I32],
        &[ValType::Ref(RefType::I31REF)],
    );

    let arg = module.locals.add(ValType::I32);
    builder.func_body().local_get(arg).ref_i31();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("make_i31", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_i31_get_s_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::I31REF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::I31REF));
    builder.func_body().local_get(arg).i31_get_s();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("get_s", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_i31_get_u_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::I31REF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::I31REF));
    builder.func_body().local_get(arg).i31_get_u();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("get_u", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_i31_roundtrip_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // Create: i32.const 42 -> ref.i31 -> i31.get_u -> returns 42
    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[ValType::I32]);

    builder.func_body().i32_const(42).ref_i31().i31_get_u();

    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("roundtrip", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_test_nullable_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_test(true, HeapType::Abstract(AbstractHeapType::I31));

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("test_nullable", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_test_non_nullable_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_test(false, HeapType::Abstract(AbstractHeapType::I31));

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("test_non_nullable", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_cast_nullable_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::Ref(RefType::I31REF)],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_cast(true, HeapType::Abstract(AbstractHeapType::I31));

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("cast_nullable", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_cast_non_nullable_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // Non-nullable cast returns (ref i31), not (ref null i31)
    let non_null_i31 = RefType {
        nullable: false,
        heap_type: HeapType::Abstract(AbstractHeapType::I31),
    };

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::Ref(non_null_i31)],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder
        .func_body()
        .local_get(arg)
        .ref_cast(false, HeapType::Abstract(AbstractHeapType::I31));

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("cast_non_nullable", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_any_convert_extern_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::EXTERNREF)],
        &[ValType::Ref(RefType::ANYREF)],
    );

    let arg = module.locals.add(ValType::Ref(RefType::EXTERNREF));
    builder.func_body().local_get(arg).any_convert_extern();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("internalize", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_extern_convert_any_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::Ref(RefType::EXTERNREF)],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));
    builder.func_body().local_get(arg).extern_convert_any();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("externalize", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_null_any_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder =
        FunctionBuilder::new(&mut module.types, &[], &[ValType::Ref(RefType::ANYREF)]);

    builder.func_body().ref_null(RefType::ANYREF);

    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("null_any", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_ref_null_i31_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder =
        FunctionBuilder::new(&mut module.types, &[], &[ValType::Ref(RefType::I31REF)]);

    builder.func_body().ref_null(RefType::I31REF);

    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("null_i31", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_br_on_cast_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));

    // Build:
    // (block $is_i31 (result i31ref)
    //   local.get 0
    //   br_on_cast $is_i31 (ref null any) (ref null i31)
    //   drop
    //   i32.const -1
    //   return
    // )
    // i31.get_u
    let block_id = {
        let block = builder.dangling_instr_seq(ValType::Ref(RefType::I31REF));
        block.id()
    };

    builder
        .instr_seq(block_id)
        .local_get(arg)
        .br_on_cast(
            block_id,
            true,
            HeapType::Abstract(AbstractHeapType::Any),
            true,
            HeapType::Abstract(AbstractHeapType::I31),
        )
        .drop()
        .i32_const(-1)
        .return_();

    builder
        .func_body()
        .instr(Block { seq: block_id })
        .i31_get_u();

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("br_on_cast_i31", func_id);

    let _ = round_trip(&mut module);
}

#[test]
fn test_br_on_cast_fail_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let mut builder = FunctionBuilder::new(
        &mut module.types,
        &[ValType::Ref(RefType::ANYREF)],
        &[ValType::I32],
    );

    let arg = module.locals.add(ValType::Ref(RefType::ANYREF));

    // Build:
    // (block $not_i31 (result anyref)
    //   local.get 0
    //   br_on_cast_fail $not_i31 (ref null any) (ref null i31)
    //   i31.get_u
    //   return
    // )
    // drop
    // i32.const -1
    let block_id = {
        let block = builder.dangling_instr_seq(ValType::Ref(RefType::ANYREF));
        block.id()
    };

    builder
        .instr_seq(block_id)
        .local_get(arg)
        .br_on_cast_fail(
            block_id,
            true,
            HeapType::Abstract(AbstractHeapType::Any),
            true,
            HeapType::Abstract(AbstractHeapType::I31),
        )
        .i31_get_u()
        .return_();

    builder
        .func_body()
        .instr(Block { seq: block_id })
        .drop()
        .i32_const(-1);

    let func_id = builder.finish(vec![arg], &mut module.funcs);
    module.exports.add("br_on_cast_fail_i31", func_id);

    let _ = round_trip(&mut module);
}

/// Build a slab allocator module programmatically.
/// This demonstrates the full GC instruction set working together.
#[test]
fn test_slab_allocator_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // Table holding both externrefs (allocated) and i31refs (free list pointers)
    let table_id = module
        .tables
        .add_local(false, 128, Some(128), RefType::ANYREF);

    // Global: head of free list (index into table)
    let free_head = module.globals.add_local(
        ValType::I32,
        true,  // mutable
        false, // shared
        ConstExpr::Value(Value::I32(0)),
    );

    // $init - Initialize free list: each slot points to next
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
        let i = module.locals.add(ValType::I32);

        // Loop body
        let loop_id = {
            let l = builder.dangling_instr_seq(None);
            l.id()
        };

        builder
            .instr_seq(loop_id)
            // table.set $slab i (ref.i31 (i + 1))
            .local_get(i)
            .local_get(i)
            .i32_const(1)
            .binop(BinaryOp::I32Add)
            .ref_i31()
            .table_set(table_id)
            // i = i + 1
            .local_get(i)
            .i32_const(1)
            .binop(BinaryOp::I32Add)
            .local_set(i)
            // br_if $loop (i < 127)
            .local_get(i)
            .i32_const(127)
            .binop(BinaryOp::I32LtU)
            .br_if(loop_id);

        builder
            .func_body()
            .i32_const(0)
            .local_set(i)
            .instr(Loop { seq: loop_id })
            // Last slot points to sentinel (128 = end of list)
            .i32_const(127)
            .i32_const(128)
            .ref_i31()
            .table_set(table_id);

        let func_id = builder.finish(vec![], &mut module.funcs);
        module.exports.add("init", func_id);
    }

    // $alloc - Allocate: pop from free list, return index
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[], &[ValType::I32]);
        let idx = module.locals.add(ValType::I32);
        let next = module.locals.add(ValType::I32);

        builder
            .func_body()
            // idx = free_head
            .global_get(free_head)
            .local_set(idx)
            // next = i31.get_u(ref.cast i31ref (table.get $slab idx))
            .local_get(idx)
            .table_get(table_id)
            .ref_cast(true, HeapType::Abstract(AbstractHeapType::I31))
            .i31_get_u()
            .local_set(next)
            // free_head = next
            .local_get(next)
            .global_set(free_head)
            // return idx
            .local_get(idx);

        let func_id = builder.finish(vec![], &mut module.funcs);
        module.exports.add("alloc", func_id);
    }

    // $dealloc - Deallocate: push onto free list
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[ValType::I32], &[]);
        let idx = module.locals.add(ValType::I32);

        builder
            .func_body()
            // slab[idx] = i31(free_head)
            .local_get(idx)
            .global_get(free_head)
            .ref_i31()
            .table_set(table_id)
            // free_head = idx
            .local_get(idx)
            .global_set(free_head);

        let func_id = builder.finish(vec![idx], &mut module.funcs);
        module.exports.add("dealloc", func_id);
    }

    // $store - Store externref at allocated slot
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32, ValType::Ref(RefType::EXTERNREF)],
            &[],
        );
        let idx = module.locals.add(ValType::I32);
        let val = module.locals.add(ValType::Ref(RefType::EXTERNREF));

        builder
            .func_body()
            .local_get(idx)
            .local_get(val)
            .any_convert_extern()
            .table_set(table_id);

        let func_id = builder.finish(vec![idx, val], &mut module.funcs);
        module.exports.add("store", func_id);
    }

    // $load - Load externref from slot
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32],
            &[ValType::Ref(RefType::EXTERNREF)],
        );
        let idx = module.locals.add(ValType::I32);

        builder
            .func_body()
            .local_get(idx)
            .table_get(table_id)
            .extern_convert_any();

        let func_id = builder.finish(vec![idx], &mut module.funcs);
        module.exports.add("load", func_id);
    }

    let _ = round_trip(&mut module);
}

// ---------------------------------------------------------------------------
// Phase 9: Builder API tests for struct/array/composite/rec_group
// ---------------------------------------------------------------------------

/// Build a struct type via `add_struct()`, then use `struct_new`, `struct_get`,
/// and `struct_set` instructions. Round-trip through wasm binary.
#[test]
fn test_struct_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // (type $point (struct (field (mut i32)) (field (mut i32))))
    let point_ty = module.types.add_struct(vec![
        FieldType {
            element_type: StorageType::Val(ValType::I32),
            mutable: true,
        },
        FieldType {
            element_type: StorageType::Val(ValType::I32),
            mutable: true,
        },
    ]);

    let point_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(point_ty),
    });

    // $make_point: (i32, i32) -> (ref $point)
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32, ValType::I32],
            &[point_ref],
        );
        let x = module.locals.add(ValType::I32);
        let y = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(x)
            .local_get(y)
            .struct_new(point_ty);
        let fid = builder.finish(vec![x, y], &mut module.funcs);
        module.exports.add("make_point", fid);
    }

    // $get_x: (ref $point) -> i32
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[point_ref], &[ValType::I32]);
        let p = module.locals.add(point_ref);
        builder.func_body().local_get(p).struct_get(point_ty, 0);
        let fid = builder.finish(vec![p], &mut module.funcs);
        module.exports.add("get_x", fid);
    }

    // $set_y: (ref $point, i32) -> ()
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[point_ref, ValType::I32], &[]);
        let p = module.locals.add(point_ref);
        let v = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(p)
            .local_get(v)
            .struct_set(point_ty, 1);
        let fid = builder.finish(vec![p, v], &mut module.funcs);
        module.exports.add("set_y", fid);
    }

    let _ = round_trip(&mut module);
}

/// Build an array type via `add_array()`, then use `array_new`, `array_get`,
/// `array_set`, and `array_len` instructions. Round-trip through wasm binary.
#[test]
fn test_array_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // (type $i32_array (array (mut i32)))
    let arr_ty = module.types.add_array(FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: true,
    });

    let arr_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(arr_ty),
    });

    // $new_array: (i32 init_val, i32 len) -> (ref $i32_array)
    {
        let mut builder =
            FunctionBuilder::new(&mut module.types, &[ValType::I32, ValType::I32], &[arr_ref]);
        let init = module.locals.add(ValType::I32);
        let len = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(init)
            .local_get(len)
            .array_new(arr_ty);
        let fid = builder.finish(vec![init, len], &mut module.funcs);
        module.exports.add("new_array", fid);
    }

    // $get_elem: (ref $i32_array, i32 idx) -> i32
    {
        let mut builder =
            FunctionBuilder::new(&mut module.types, &[arr_ref, ValType::I32], &[ValType::I32]);
        let a = module.locals.add(arr_ref);
        let idx = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(a)
            .local_get(idx)
            .array_get(arr_ty);
        let fid = builder.finish(vec![a, idx], &mut module.funcs);
        module.exports.add("get_elem", fid);
    }

    // $set_elem: (ref $i32_array, i32 idx, i32 val) -> ()
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[arr_ref, ValType::I32, ValType::I32],
            &[],
        );
        let a = module.locals.add(arr_ref);
        let idx = module.locals.add(ValType::I32);
        let val = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(a)
            .local_get(idx)
            .local_get(val)
            .array_set(arr_ty);
        let fid = builder.finish(vec![a, idx, val], &mut module.funcs);
        module.exports.add("set_elem", fid);
    }

    // $array_length: (ref $i32_array) -> i32
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[arr_ref], &[ValType::I32]);
        let a = module.locals.add(arr_ref);
        builder.func_body().local_get(a).array_len();
        let fid = builder.finish(vec![a], &mut module.funcs);
        module.exports.add("array_length", fid);
    }

    let _ = round_trip(&mut module);
}

/// Build a non-final type with a subtype via `add_composite()`. Round-trip.
#[test]
fn test_add_composite_with_subtype() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // (type $base (sub (struct (field i32))))
    let base_ty = module.types.add_composite(
        CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            }]
            .into_boxed_slice(),
        }),
        false, // not final — open for subtyping
        None,
    );

    // (type $derived (sub final $base (struct (field i32) (field f64))))
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
        true, // final
        Some(base_ty),
    );

    let derived_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(derived_ty),
    });

    // $make_derived: (i32, f64) -> (ref $derived)
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32, ValType::F64],
            &[derived_ref],
        );
        let a = module.locals.add(ValType::I32);
        let b = module.locals.add(ValType::F64);
        builder
            .func_body()
            .local_get(a)
            .local_get(b)
            .struct_new(derived_ty);
        let fid = builder.finish(vec![a, b], &mut module.funcs);
        module.exports.add("make_derived", fid);
    }

    // $get_base_field: (ref $base) -> i32  (using base type to access field 0)
    {
        let base_ref = ValType::Ref(RefType {
            nullable: true,
            heap_type: HeapType::Concrete(base_ty),
        });
        let mut builder = FunctionBuilder::new(&mut module.types, &[base_ref], &[ValType::I32]);
        let p = module.locals.add(base_ref);
        builder.func_body().local_get(p).struct_get(base_ty, 0);
        let fid = builder.finish(vec![p], &mut module.funcs);
        module.exports.add("get_base_field", fid);
    }

    let _ = round_trip(&mut module);
}

/// Build a recursive group with `add_rec_group()` — two mutually-recursive
/// struct types (A has ref to B, B has ref to A). Round-trip.
#[test]
fn test_rec_group_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // (rec
    //   (type $a (struct (field (ref null $b))))
    //   (type $b (struct (field (ref null $a))))
    // )
    let ids = module.types.add_rec_group(2, |type_ids| {
        let a_id = type_ids[0];
        let b_id = type_ids[1];

        let a_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(b_id),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });

        let b_def = CompositeType::Struct(StructType {
            fields: vec![FieldType {
                element_type: StorageType::Val(ValType::Ref(RefType {
                    nullable: true,
                    heap_type: HeapType::Concrete(a_id),
                })),
                mutable: false,
            }]
            .into_boxed_slice(),
        });

        vec![(a_def, true, None), (b_def, true, None)]
    });

    let a_ty = ids[0];
    let b_ty = ids[1];

    let a_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(a_ty),
    });
    let b_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(b_ty),
    });

    // $make_a: (ref null $b) -> (ref $a)
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[b_ref], &[a_ref]);
        let b = module.locals.add(b_ref);
        builder.func_body().local_get(b).struct_new(a_ty);
        let fid = builder.finish(vec![b], &mut module.funcs);
        module.exports.add("make_a", fid);
    }

    // $make_b: (ref null $a) -> (ref $b)
    {
        let mut builder = FunctionBuilder::new(&mut module.types, &[a_ref], &[b_ref]);
        let a = module.locals.add(a_ref);
        builder.func_body().local_get(a).struct_new(b_ty);
        let fid = builder.finish(vec![a], &mut module.funcs);
        module.exports.add("make_b", fid);
    }

    let _ = round_trip(&mut module);
}

/// Verify that `add_struct()` deduplicates: two identical calls return the
/// same `TypeId`.
#[test]
fn test_struct_type_dedup() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    let fields = vec![
        FieldType {
            element_type: StorageType::Val(ValType::I32),
            mutable: true,
        },
        FieldType {
            element_type: StorageType::Val(ValType::F64),
            mutable: false,
        },
    ];

    let ty1 = module.types.add_struct(fields.clone());
    let ty2 = module.types.add_struct(fields);

    assert_eq!(
        ty1, ty2,
        "add_struct with identical fields should return the same TypeId"
    );

    // Use the type so it survives GC, then round-trip.
    let ty_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(ty1),
    });
    let mut builder =
        FunctionBuilder::new(&mut module.types, &[ValType::I32, ValType::F64], &[ty_ref]);
    let a = module.locals.add(ValType::I32);
    let b = module.locals.add(ValType::F64);
    builder
        .func_body()
        .local_get(a)
        .local_get(b)
        .struct_new(ty1);
    let fid = builder.finish(vec![a, b], &mut module.funcs);
    module.exports.add("make", fid);

    let _ = round_trip(&mut module);
}

/// Build an array via `add_array()` and use `array_new_fixed` to create it
/// from stack values. Round-trip.
#[test]
fn test_array_new_fixed_builder() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config);

    // (type $i32_arr (array (mut i32)))
    let arr_ty = module.types.add_array(FieldType {
        element_type: StorageType::Val(ValType::I32),
        mutable: true,
    });

    let arr_ref = ValType::Ref(RefType {
        nullable: true,
        heap_type: HeapType::Concrete(arr_ty),
    });

    // $make_triple: (i32, i32, i32) -> (ref $i32_arr)
    {
        let mut builder = FunctionBuilder::new(
            &mut module.types,
            &[ValType::I32, ValType::I32, ValType::I32],
            &[arr_ref],
        );
        let a = module.locals.add(ValType::I32);
        let b = module.locals.add(ValType::I32);
        let c = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(a)
            .local_get(b)
            .local_get(c)
            .array_new_fixed(arr_ty, 3);
        let fid = builder.finish(vec![a, b, c], &mut module.funcs);
        module.exports.add("make_triple", fid);
    }

    // $get_elem: (ref $i32_arr, i32) -> i32
    {
        let mut builder =
            FunctionBuilder::new(&mut module.types, &[arr_ref, ValType::I32], &[ValType::I32]);
        let a = module.locals.add(arr_ref);
        let idx = module.locals.add(ValType::I32);
        builder
            .func_body()
            .local_get(a)
            .local_get(idx)
            .array_get(arr_ty);
        let fid = builder.finish(vec![a, idx], &mut module.funcs);
        module.exports.add("get_elem", fid);
    }

    let _ = round_trip(&mut module);
}
