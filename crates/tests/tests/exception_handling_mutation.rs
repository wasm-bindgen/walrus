//! Tests for exception handling mutation API

use walrus::{FunctionBuilder, Module, ModuleConfig, ValType};

#[test]
fn create_tag_and_export() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type = module.types.add(&[ValType::I32, ValType::I32], &[]);

    let tag_id = module.tags.add(tag_type);

    module.exports.add("my_exception", tag_id);

    let wasm = module.emit_wasm();

    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2, "Round-trip should be deterministic");
}

#[test]
fn create_multiple_tags() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type1 = module.types.add(&[ValType::I32], &[]);
    let tag_type2 = module.types.add(&[ValType::I64, ValType::I32], &[]);
    let tag_type3 = module.types.add(&[], &[]);

    let tag_id1 = module.tags.add(tag_type1);
    let tag_id2 = module.tags.add(tag_type2);
    let tag_id3 = module.tags.add(tag_type3);

    module.exports.add("error1", tag_id1);
    module.exports.add("error2", tag_id2);
    module.exports.add("error3", tag_id3);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_tag_with_function_using_throw() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type = module.types.add(&[ValType::I32], &[]);
    let tag_id = module.tags.add(tag_type);

    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let mut body = builder.func_body();

    body.i32_const(42);

    body.throw(tag_id);

    let func_id = builder.finish(vec![], &mut module.funcs);

    module.exports.add("my_tag", tag_id);
    module.exports.add("throw_func", func_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_tag_with_multiple_params() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type = module.types.add(
        &[ValType::I32, ValType::I64, ValType::F32, ValType::F64],
        &[],
    );
    let tag_id = module.tags.add(tag_type);

    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let mut body = builder.func_body();

    body.i32_const(1);
    body.i64_const(2);
    body.f32_const(3.0);
    body.f64_const(4.0);
    body.throw(tag_id);

    let func_id = builder.finish(vec![], &mut module.funcs);

    module.exports.add("multi_param_tag", tag_id);
    module.exports.add("throw_multi", func_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn create_tag_with_no_params() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type = module.types.add(&[], &[]);
    let tag_id = module.tags.add(tag_type);

    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let mut body = builder.func_body();
    body.throw(tag_id);
    let func_id = builder.finish(vec![], &mut module.funcs);

    module.exports.add("empty_tag", tag_id);
    module.exports.add("throw_empty", func_id);

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).unwrap();
    let wasm2 = module2.emit_wasm();

    assert_eq!(wasm, wasm2);
}

#[test]
fn parse_module_with_tags() {
    let wat = r#"
        (module
          (tag $e (param i32))
          (func (export "throw")
            i32.const 42
            throw $e
          )
        )
    "#;

    let wasm = wat::parse_str(wat).unwrap();
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = config.parse(&wasm).unwrap();

    assert_eq!(module.tags.iter().count(), 1);

    let wasm2 = module.emit_wasm();
    let mut module2 = config.parse(&wasm2).unwrap();
    let wasm3 = module2.emit_wasm();

    assert_eq!(wasm2, wasm3);
}

#[test]
fn mutate_tag_type() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type1 = module.types.add(&[ValType::I32], &[]);
    let tag_id = module.tags.add(tag_type1);

    let tag_type2 = module.types.add(&[ValType::I64], &[]);
    let tag = module.tags.get_mut(tag_id);
    tag.ty = tag_type2;

    module.exports.add("my_tag", tag_id);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).unwrap();

    let parsed_tag = module2.tags.iter().next().unwrap();
    let parsed_type = module2.types.get(parsed_tag.ty);
    assert_eq!(parsed_type.params(), &[ValType::I64]);
}

#[test]
fn parse_module_with_try_table() {
    let wat = r#"
        (module
          (tag $e (param i32))
          (func $throw
            i32.const 42
            throw $e
          )
          (func (export "catch") (result i32)
            (block $handler (result i32)
              (try_table (catch $e $handler)
                (call $throw)
              )
              (return (i32.const 0))
            )
          )
        )
    "#;

    let wasm = wat::parse_str(wat).unwrap();
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = config.parse(&wasm).unwrap();

    assert_eq!(module.tags.iter().count(), 1);

    let wasm2 = module.emit_wasm();
    let mut module2 = config.parse(&wasm2).unwrap();
    let wasm3 = module2.emit_wasm();

    assert_eq!(wasm2, wasm3);
}

#[test]
fn parse_module_with_multiple_catch_clauses() {
    let wat = r#"
        (module
          (tag $e1 (param i32))
          (tag $e2 (param i64))
          (func $throw1
            i32.const 1
            throw $e1
          )
          (func $throw2
            i64.const 2
            throw $e2
          )
          (func (export "multi_catch1") (result i32)
            (block $handler (result i32)
              (try_table (catch $e1 $handler)
                (call $throw1)
              )
              (return (i32.const 0))
            )
          )
          (func (export "multi_catch2") (result i64)
            (block $handler (result i64)
              (try_table (catch $e2 $handler)
                (call $throw2)
              )
              (return (i64.const 0))
            )
          )
        )
    "#;

    let wasm = wat::parse_str(wat).unwrap();
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = config.parse(&wasm).unwrap();

    assert_eq!(module.tags.iter().count(), 2);

    let wasm2 = module.emit_wasm();
    let mut module2 = config.parse(&wasm2).unwrap();
    let wasm3 = module2.emit_wasm();

    assert_eq!(wasm2, wasm3);
}

#[test]
fn delete_tag() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let tag_type1 = module.types.add(&[ValType::I32], &[]);
    let tag_id1 = module.tags.add(tag_type1);

    let tag_type2 = module.types.add(&[ValType::I64], &[]);
    let tag_id2 = module.tags.add(tag_type2);

    module.exports.add("tag1", tag_id1);
    module.exports.add("tag2", tag_id2);

    assert_eq!(module.tags.iter().count(), 2);

    module.tags.delete(tag_id1);
    let export_id = module
        .exports
        .iter()
        .find(|e| e.name == "tag1")
        .unwrap()
        .id();
    module.exports.delete(export_id);

    assert_eq!(module.tags.iter().count(), 1);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).unwrap();

    assert_eq!(module2.tags.iter().count(), 1);
}

#[test]
fn imported_tag() {
    let wat = r#"
        (module
          (import "env" "exception" (tag $e (param i32)))
          (func (export "throw_imported")
            i32.const 99
            throw $e
          )
        )
    "#;

    let wasm = wat::parse_str(wat).unwrap();
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = config.parse(&wasm).unwrap();

    assert_eq!(module.tags.iter().count(), 1);

    let tag = module.tags.iter().next().unwrap();
    assert!(matches!(tag.kind, walrus::TagKind::Import(_)));

    let wasm2 = module.emit_wasm();
    let mut module2 = config.parse(&wasm2).unwrap();
    let wasm3 = module2.emit_wasm();

    assert_eq!(wasm2, wasm3);
}
