//! The implicit block types of legacy `try` handlers and function entry
//! blocks are registered after function bodies are parsed. These tests pin
//! the resulting `TypeId` order and block types, which must match registering
//! them function by function before parsing: the entry type, then each
//! multi-value handler type in operator order.

use walrus::ir::{Instr, InstrSeqType, LegacyCatch};
use walrus::{FunctionKind, Module, ValType};

const WAT: &str = r#"(module
  (type $mv (func (result i32 i64)))
  (type $open (sub (func (param f32) (result f64 f64))))
  (tag $a (param i32))
  (tag $b (param f32 f64))
  (tag $c (param f32))
  (func $f0 (result i64 i32)
    try (result i64 i32)
      i64.const 0
      i32.const 0
    catch $a
      drop
      i64.const 1
      i32.const 1
    catch_all
      i64.const 2
      i32.const 2
    end)
  (func $f1 (result i32)
    try (result i32 i64)
      i32.const 0
      i64.const 0
    catch_all
      i32.const 1
      i64.const 1
    end
    drop)
  (func $f2
    try
    catch $b
      drop
      drop
    end
    try (result f64 f64)
      f64.const 0
      f64.const 0
    catch $c
      drop
      f64.const 1
      f64.const 1
    end
    drop
    drop))"#;

fn parse() -> Module {
    let wasm = wat::parse_str(WAT).unwrap();
    Module::from_buffer(&wasm).unwrap()
}

fn sig(module: &Module, ty: InstrSeqType) -> (Vec<ValType>, Vec<ValType>) {
    match ty {
        InstrSeqType::Simple(r) => (vec![], r.into_iter().collect()),
        InstrSeqType::MultiValue(id) => {
            let (p, r) = module.types.params_results(id);
            (p.to_vec(), r.to_vec())
        }
    }
}

#[test]
fn type_arena_order() {
    use ValType::*;
    let module = parse();
    let types: Vec<_> = module
        .types
        .iter()
        .map(|t| (t.params().to_vec(), t.results().to_vec()))
        .collect();
    assert_eq!(
        types,
        vec![
            // The type section.
            (vec![], vec![I32, I64]),
            (vec![F32], vec![F64, F64]),
            (vec![I32], vec![]),
            (vec![F32, F64], vec![]),
            (vec![F32], vec![]),
            (vec![], vec![I64, I32]),
            (vec![], vec![I32]),
            (vec![], vec![]),
            (vec![], vec![F64, F64]),
            // $f0: entry type, then `catch $a`. `catch_all` is `[] -> [i64 i32]`,
            // which is already the type of $f0.
            (vec![], vec![I64, I32]),
            (vec![I32], vec![I64, I32]),
            // $f1: entry type. `catch_all` is `[] -> [i32 i64]`, i.e. $mv.
            (vec![], vec![I32]),
            // $f2: entry type. `catch $b` is `[f32 f64] -> []`, i.e. $b's type.
            // `catch $c` is `[f32] -> [f64 f64]`. $open has that signature
            // but isn't final, so it isn't deduplicated against and a new
            // type is added.
            (vec![], vec![]),
            (vec![F32], vec![F64, F64]),
        ]
    );
}

#[test]
fn handler_block_types() {
    let module = parse();
    let mv = module.types.iter().next().unwrap().id();
    let open = module.types.iter().nth(1).unwrap().id();

    let mut handlers = vec![];
    for name in ["f0", "f1", "f2"] {
        let f = module.funcs.by_name(name).unwrap();
        let FunctionKind::Local(local) = &module.funcs.get(f).kind else {
            panic!()
        };
        // The entry block's type is the function's entry type.
        let entry = local.block(local.entry_block());
        let InstrSeqType::MultiValue(entry_ty) = entry.ty else {
            panic!("entry block of {name} should have a MultiValue type")
        };
        assert_eq!(
            module.types.results(entry_ty),
            module.types.results(local.ty())
        );
        for (instr, _) in entry.instrs.iter() {
            if let Instr::Try(t) = instr {
                for c in t.catches.iter() {
                    if let LegacyCatch::Catch { handler, .. } | LegacyCatch::CatchAll { handler } =
                        c
                    {
                        handlers.push(local.block(*handler).ty);
                    }
                }
            }
        }
    }
    assert_eq!(handlers.len(), 5);
    // Every handler block has the `[tag params] -> [try results]` signature.
    use ValType::*;
    let sigs: Vec<_> = handlers.iter().map(|t| sig(&module, *t)).collect();
    assert_eq!(
        sigs,
        vec![
            (vec![I32], vec![I64, I32]),
            (vec![], vec![I64, I32]),
            (vec![], vec![I32, I64]),
            (vec![F32, F64], vec![]),
            (vec![F32], vec![F64, F64]),
        ]
    );
    // Handler types resolve to the first matching type, like any other
    // multi-value block: $mv itself, and $open rather than the type added for
    // the handler.
    assert_eq!(handlers[2], InstrSeqType::MultiValue(mv));
    assert_eq!(handlers[4], InstrSeqType::MultiValue(open));
}

#[test]
fn round_trip() {
    let mut module = parse();
    let wasm = module.emit_wasm();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .unwrap();
    Module::from_buffer(&wasm).unwrap();
}

/// A legacy `try` whose block type names a struct type is invalid. This used
/// to panic in `ModuleTypes::results` before the body was validated.
#[test]
fn struct_typed_try_is_an_error() {
    let wasm = wat::parse_str(
        r#"(module
          (type $s (struct (field i32)))
          (func
            try (type $s)
            end))"#,
    )
    .unwrap();
    assert!(Module::from_buffer(&wasm).is_err());
}
