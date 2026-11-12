//! `visit_instr_seq_id` is called for the instruction sequences an instruction
//! owns (block, loop and `if` arms, `try_table` and `try` bodies, legacy
//! `catch` handlers), once each, and not for branch targets, which refer back
//! to a sequence that was already visited.

use std::collections::HashMap;
use walrus::ir::{dfs_in_order, dfs_pre_order_mut, InstrSeq, InstrSeqId, Visitor, VisitorMut};
use walrus::{LocalFunction, Module};

const WAT: &str = r#"
(module
  (type $base (sub (struct (field i32))))
  (type $child (sub final $base (struct (field i32))))
  (tag $e)

  (func (export "br") (param i32)
    block $b
      loop $l
        local.get 0
        br_if $b
        local.get 0
        br_table $b $l $b
      end
    end
    local.get 0
    if
      br 0
    else
      nop
    end)

  (func (export "br_on_null") (param anyref)
    block $b
      local.get 0
      br_on_null $b
      drop
    end)

  (func (export "br_on_non_null") (param anyref)
    block $b (result (ref any))
      local.get 0
      br_on_non_null $b
      unreachable
    end
    drop)

  (func (export "br_on_cast") (param (ref null $base)) (result i32)
    block $yes (result (ref null $child))
      local.get 0
      br_on_cast $yes (ref null $base) (ref null $child)
      drop
      i32.const 0
      return
    end
    drop
    i32.const 1)

  (func (export "br_on_cast_fail") (param (ref null $base)) (result i32)
    block $no (result (ref null $base))
      local.get 0
      br_on_cast_fail $no (ref null $base) (ref null $child)
      drop
      i32.const 1
      return
    end
    drop
    i32.const 0)

  (func (export "try_table")
    block $h
      try_table (catch $e $h) (catch_all $h)
        throw $e
      end
    end)

  (func (export "legacy_try")
    try
      throw $e
    catch $e
      nop
    catch_all
      nop
    end
    block
      try
        throw $e
      delegate 0
    end))
"#;

#[derive(Default)]
struct Seqs {
    started: Vec<InstrSeqId>,
    visited: Vec<InstrSeqId>,
}

impl<'instr> Visitor<'instr> for Seqs {
    fn start_instr_seq(&mut self, seq: &'instr InstrSeq) {
        self.started.push(seq.id());
    }
    fn visit_instr_seq_id(&mut self, id: &InstrSeqId) {
        self.visited.push(*id);
    }
}

impl VisitorMut for Seqs {
    fn start_instr_seq_mut(&mut self, seq: &mut InstrSeq) {
        self.started.push(seq.id());
    }
    fn visit_instr_seq_id_mut(&mut self, id: &mut InstrSeqId) {
        self.visited.push(*id);
    }
}

/// Every sequence but the entry block is visited exactly once.
fn check(name: &str, f: &LocalFunction, seqs: Seqs) {
    let mut expected: HashMap<InstrSeqId, usize> = HashMap::new();
    for id in seqs.started.iter().filter(|&&id| id != f.entry_block()) {
        *expected.entry(*id).or_default() += 1;
    }
    let mut visited: HashMap<InstrSeqId, usize> = HashMap::new();
    for id in &seqs.visited {
        *visited.entry(*id).or_default() += 1;
    }
    assert_eq!(visited, expected, "{name}: visit_instr_seq_id calls");
}

fn functions(module: &Module) -> Vec<(String, walrus::FunctionId)> {
    module
        .exports
        .iter()
        .filter_map(|e| match e.item {
            walrus::ExportItem::Function(f) => Some((e.name.clone(), f)),
            _ => None,
        })
        .collect()
}

#[test]
fn dfs_in_order_visits_owned_sequences_once() {
    let module = Module::from_buffer(&wat::parse_str(WAT).unwrap()).unwrap();
    for (name, id) in functions(&module) {
        let f = module.funcs.get(id).kind.unwrap_local();
        let mut seqs = Seqs::default();
        dfs_in_order(&mut seqs, f, f.entry_block());
        check(&name, f, seqs);
    }
}

#[test]
fn dfs_pre_order_mut_visits_owned_sequences_once() {
    let mut module = Module::from_buffer(&wat::parse_str(WAT).unwrap()).unwrap();
    for (name, id) in functions(&module) {
        let f = module.funcs.get_mut(id).kind.unwrap_local_mut();
        let entry = f.entry_block();
        let mut seqs = Seqs::default();
        dfs_pre_order_mut(&mut seqs, f, entry);
        check(&name, f, seqs);
    }
}

/// A `VisitorMut` that only handles one kind of field sees each such field
/// once, as a `Visitor` does.
#[test]
fn visitor_mut_visits_each_field_once() {
    struct AddOne;
    impl VisitorMut for AddOne {
        fn visit_value_mut(&mut self, value: &mut walrus::ir::Value) {
            if let walrus::ir::Value::I32(x) = value {
                *x += 1;
            }
        }
    }
    let wat = r#"(module (func (export "f") (result i32) i32.const 41))"#;
    let mut module = Module::from_buffer(&wat::parse_str(wat).unwrap()).unwrap();
    let id = functions(&module)[0].1;
    let f = module.funcs.get_mut(id).kind.unwrap_local_mut();
    let entry = f.entry_block();
    dfs_pre_order_mut(&mut AddOne, f, entry);
    match &f.block(entry).instrs[0].0 {
        walrus::ir::Instr::Const(c) => assert!(matches!(c.value, walrus::ir::Value::I32(42))),
        other => panic!("unexpected {other:?}"),
    }
}
