//! Emission only considers instruction sequences reachable from a function's
//! entry block. Sequences that a pass created but never linked in ("dangling"
//! sequences) must not affect function sizes (and so the largest-first
//! function order), the `DataCount` section, or synthesized `ref.func`
//! declarations. In particular, a function's size is not the total length of
//! the sequences in its arena.

use walrus::{FunctionKind, Module};

#[test]
fn dangling_instr_seqs_are_ignored() {
    let wasm = wat::parse_str(
        r#"(module
          (memory 1)
          (data (i32.const 0) "x")
          (func $small i32.const 0 drop)
          (func $big i32.const 0 drop i32.const 0 drop i32.const 0 drop)
          (func $target))"#,
    )
    .unwrap();
    let mut module = Module::from_buffer(&wasm).unwrap();
    let data = module.data.iter().next().unwrap().id();
    let target = module.funcs.by_name("target").unwrap();
    let small = module.funcs.by_name("small").unwrap();
    let FunctionKind::Local(local) = &mut module.funcs.get_mut(small).kind else {
        panic!()
    };
    let mut seq = local.builder_mut().dangling_instr_seq(None);
    for _ in 0..10 {
        seq.i32_const(0).drop();
    }
    seq.data_drop(data).ref_func(target).drop();

    let out = module.emit_wasm();

    for payload in wasmparser::Parser::new(0).parse_all(&out) {
        match payload.unwrap() {
            wasmparser::Payload::DataCountSection { .. } => {
                panic!("data.drop in a dangling sequence shouldn't need DataCount")
            }
            wasmparser::Payload::ElementSection(_) => {
                panic!("ref.func in a dangling sequence shouldn't need a declaration")
            }
            _ => {}
        }
    }

    // Functions are emitted largest first, by reachable instructions.
    let out = Module::from_buffer(&out).unwrap();
    let order: Vec<_> = out
        .funcs
        .iter()
        .map(|f| f.name.as_deref().unwrap())
        .collect();
    assert_eq!(order, ["big", "small", "target"]);
}
