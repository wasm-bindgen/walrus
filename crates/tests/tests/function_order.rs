//! Tests for `ModuleConfig::function_order`, which decides the order defined
//! functions are emitted, and so numbered, in.
//!
//! The default order is not guaranteed; `default_order` checks what it is
//! today and should be updated, not preserved, if it changes.

use std::cmp::Reverse;
use std::sync::{Arc, Mutex};

use walrus::ir::Instr;
use walrus::{FunctionId, Module, ModuleConfig};

/// Sizes (in instructions): `big` 9, `caller` 7, `small` 1, `in_table` 0.
/// Uses: `small` 4 (three `call`s and a `return_call`), `in_table` 2 (listed
/// twice in an element segment), `big` 1 (an export), `caller` 0.
const WAT: &str = r#"
(module
  (table 2 funcref)
  (func $big (export "big") (result i32)
    i32.const 1 i32.const 2 i32.add i32.const 3 i32.add
    i32.const 4 i32.add i32.const 5 i32.add)
  (func $small (result i32) i32.const 0)
  (func $caller (result i32)
    call $small call $small i32.add call $small i32.add drop
    return_call $small)
  (func $in_table)
  (elem (i32.const 0) func $in_table $in_table))
"#;

/// Parses `WAT`, emits it with `config`, and returns the names of the
/// emitted module's defined functions in index order.
fn emitted_order(config: &ModuleConfig) -> Vec<String> {
    let wasm = wat::parse_str(WAT).unwrap();
    let mut module = config.parse(&wasm).unwrap();
    let emitted = module.emit_wasm();
    let module = Module::from_buffer(&emitted).unwrap();
    check_calls(&module);
    module
        .funcs
        .iter_local()
        .map(|(id, _)| name(&module, id))
        .collect()
}

fn name(module: &Module, id: FunctionId) -> String {
    module.funcs.get(id).name.clone().unwrap()
}

/// The renumbered calls in `caller` still reach `small`.
fn check_calls(module: &Module) {
    let caller = module.funcs.by_name("caller").unwrap();
    let local = module.funcs.get(caller).kind.unwrap_local();
    let targets: Vec<_> = local
        .block(local.entry_block())
        .instrs
        .iter()
        .filter_map(|(instr, _)| match instr {
            Instr::Call(c) => Some(c.func),
            Instr::ReturnCall(c) => Some(c.func),
            _ => None,
        })
        .map(|f| name(module, f))
        .collect();
    assert_eq!(targets, ["small"; 4]);
}

#[test]
fn default_order() {
    // Largest first.
    assert_eq!(
        emitted_order(&ModuleConfig::new()),
        ["big", "caller", "small", "in_table"]
    );
}

#[test]
fn most_used_first() {
    let mut config = ModuleConfig::new();
    config.function_order(|_, functions| functions.sort_by_key(|f| Reverse(f.uses())));
    assert_eq!(
        emitted_order(&config),
        ["small", "in_table", "big", "caller"]
    );
}

#[test]
fn closure_sees_default_order_sizes_and_uses() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut config = ModuleConfig::new();
    let record = seen.clone();
    config.function_order(move |module, functions| {
        *record.lock().unwrap() = functions
            .iter()
            .map(|f| (name(module, f.id()), f.size(), f.uses()))
            .collect();
    });
    let order = emitted_order(&config);

    let expected = [
        ("big", 9, 1),
        ("caller", 7, 0),
        ("small", 1, 4),
        ("in_table", 0, 2),
    ]
    .map(|(n, size, uses)| (n.to_string(), size, uses));
    assert_eq!(*seen.lock().unwrap(), expected);
    // Leaving the slice as it is keeps the default order.
    assert_eq!(order, ["big", "caller", "small", "in_table"]);
}

#[test]
fn closure_can_read_the_module() {
    let mut config = ModuleConfig::new();
    config.function_order(|module, functions| {
        functions.sort_by_key(|f| module.funcs.get(f.id()).name.clone());
    });
    assert_eq!(
        emitted_order(&config),
        ["big", "caller", "in_table", "small"]
    );
}

#[test]
fn clone_keeps_the_closure() {
    let mut config = ModuleConfig::new();
    config.function_order(|_, functions| functions.reverse());
    assert_eq!(
        emitted_order(&config.clone()),
        ["in_table", "small", "caller", "big"]
    );
}

#[test]
fn uses_count_every_index_outside_code() {
    let wat = r#"
    (module
      (table 2 funcref)
      (global funcref (ref.func $in_global))
      (elem (i32.const 0) funcref (ref.func $in_elem_expr) (ref.func $in_elem_expr))
      (elem declare func $by_ref_func)
      (start $start)
      (func $start)
      (func $in_global)
      (func $in_elem_expr)
      (func $by_ref_func)
      (func $user
        ref.func $by_ref_func drop
        ref.func $by_ref_func drop))
    "#;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut config = ModuleConfig::new();
    let record = seen.clone();
    config.function_order(move |module, functions| {
        let mut uses: Vec<_> = functions
            .iter()
            .map(|f| (name(module, f.id()), f.uses()))
            .collect();
        uses.sort();
        *record.lock().unwrap() = uses;
    });
    let mut module = config.parse(&wat::parse_str(wat).unwrap()).unwrap();
    module.emit_wasm();
    let expected = [
        // Two `ref.func`s in code and its `declare` segment.
        ("by_ref_func", 3),
        // Listed twice in an element segment's expressions.
        ("in_elem_expr", 2),
        // The global initializer, and the `declare` segment walrus adds for
        // functions referenced from one.
        ("in_global", 2),
        ("start", 1),
        ("user", 0),
    ]
    .map(|(n, uses)| (n.to_string(), uses));
    assert_eq!(*seen.lock().unwrap(), expected);
}
