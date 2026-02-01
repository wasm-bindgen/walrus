#![allow(clippy::approx_constant)]
//! Tests for const expression mutation API

use walrus::{ConstExpr, ConstOp, Module, ModuleConfig, RefType, ValType};

#[test]
fn create_global_with_extended_const_expr_i32_add() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I32Const(5),
        ConstOp::I32Const(3),
        ConstOp::I32Add,
    ]);

    let global_id = module.globals.add_local(ValType::I32, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();

    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2, "Round-trip should be deterministic");
}

#[test]
fn create_global_with_extended_const_expr_i32_sub() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I32Const(15),
        ConstOp::I32Const(7),
        ConstOp::I32Sub,
    ]);

    let global_id = module.globals.add_local(ValType::I32, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_i32_mul() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I32Const(4),
        ConstOp::I32Const(6),
        ConstOp::I32Mul,
    ]);

    let global_id = module.globals.add_local(ValType::I32, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_i64_add() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I64Const(100),
        ConstOp::I64Const(50),
        ConstOp::I64Add,
    ]);

    let global_id = module.globals.add_local(ValType::I64, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_i64_sub() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I64Const(200),
        ConstOp::I64Const(75),
        ConstOp::I64Sub,
    ]);

    let global_id = module.globals.add_local(ValType::I64, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_i64_mul() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I64Const(8),
        ConstOp::I64Const(9),
        ConstOp::I64Mul,
    ]);

    let global_id = module.globals.add_local(ValType::I64, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_complex() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![
        ConstOp::I32Const(10),
        ConstOp::I32Const(5),
        ConstOp::I32Add,
        ConstOp::I32Const(2),
        ConstOp::I32Mul,
    ]);

    let global_id = module.globals.add_local(ValType::I32, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_global_get() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let base_global = module.globals.add_local(
        ValType::I32,
        false,
        false,
        ConstExpr::Value(walrus::ir::Value::I32(42)),
    );

    let init = ConstExpr::Extended(vec![
        ConstOp::GlobalGet(base_global),
        ConstOp::I32Const(8),
        ConstOp::I32Add,
    ]);

    let derived_global = module.globals.add_local(ValType::I32, false, false, init);
    module.exports.add("base", base_global);
    module.exports.add("derived", derived_global);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_f32() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![ConstOp::F32Const(3.14)]);

    let global_id = module.globals.add_local(ValType::F32, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_f64() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![ConstOp::F64Const(2.71828)]);

    let global_id = module.globals.add_local(ValType::F64, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_v128() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![ConstOp::V128Const(0x0102030405060708090a0b0c0d0e0f10)]);

    let global_id = module.globals.add_local(ValType::V128, false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_ref_null() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let init = ConstExpr::Extended(vec![ConstOp::RefNull(RefType::FUNCREF)]);

    let global_id = module
        .globals
        .add_local(ValType::Ref(RefType::FUNCREF), false, false, init);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_global_with_extended_const_expr_ref_func() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let builder = walrus::FunctionBuilder::new(&mut module.types, &[], &[]);
    let func_id = builder.finish(vec![], &mut module.funcs);

    let init = ConstExpr::Extended(vec![ConstOp::RefFunc(func_id)]);

    let global_id = module
        .globals
        .add_local(ValType::Ref(RefType::FUNCREF), false, false, init);
    module.exports.add("f", func_id);
    module.exports.add("g", global_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}
