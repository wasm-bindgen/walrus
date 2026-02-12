//! Tests for GC instruction builder methods and programmatic module construction.

use walrus::ir::*;
use walrus::{
    AbstractHeapType, ConstExpr, FunctionBuilder, HeapType, Module, ModuleConfig, RefType, ValType,
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
