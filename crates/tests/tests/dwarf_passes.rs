//! Passes can insert, copy and remove instructions and replace locals, so
//! walrus derives every converted code address from where each input
//! instruction was emitted. These tests make such changes, then check lines,
//! ranges, location lists, local indices and static addresses against the
//! emitted code.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, LineProgram, LineString, Location,
    LocationList, Sections,
};
use gimli::{constants, read, Encoding, EndianSlice, Format, LineEncoding, LittleEndian};
use std::borrow::Cow;
use std::collections::BTreeMap;
use walrus::ir::{Instr, LocalGet, LocalSet};
use walrus::{FunctionId, Module};
use wasm_encoder::CustomSection;
use wasmparser::{ExternalKind, Operator, Parser, Payload};

const WAT: &str = r#"
(module
  (memory 1)
  (func $f (export "f") (param i32) (result i32)
    (local i32 i32)
    local.get 0
    local.set 1
    local.get 1
    i32.const 1
    i32.add
    local.set 2
    local.get 2)
  (func $g (export "g") (result i32)
    i32.const 7))
"#;

/// The line of input instruction `i` of `f`, and of `g`'s first instruction.
fn line(i: usize) -> u64 {
    10 + i as u64
}
const G_LINE: u64 = 50;
/// The instructions of `f` a lexical block covers, and those of `y`'s
/// location list entry, as `start..end` indices.
const BLOCK: (usize, usize) = (3, 5);
const Y: (usize, usize) = (5, 7);
/// A static variable's linear memory address.
const STATIC: u64 = 0x1000;

/// A function body, code-section relative: the start after its size, each
/// instruction's address and local index operand, and the end.
#[derive(Debug, Default)]
struct Body {
    start: u64,
    instrs: Vec<(u64, Option<u32>)>,
    end: u64,
}

impl Body {
    fn addr(&self, i: usize) -> u64 {
        self.instrs[i].0
    }
}

/// The bodies of the exported functions, by export name.
fn bodies(wasm: &[u8]) -> BTreeMap<String, Body> {
    let mut code_start = 0;
    let mut exports = Vec::new();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::ExportSection(s) => {
                for e in s {
                    let e = e.unwrap();
                    if e.kind == ExternalKind::Func {
                        exports.push((e.name.to_string(), e.index as usize));
                    }
                }
            }
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                let rel = |a: usize| (a - code_start) as u64;
                let mut ops = body.get_operators_reader().unwrap();
                let mut instrs = Vec::new();
                while !ops.eof() {
                    let (op, offset) = ops.read_with_offset().unwrap();
                    let local = match op {
                        Operator::LocalGet { local_index }
                        | Operator::LocalSet { local_index }
                        | Operator::LocalTee { local_index } => Some(local_index),
                        _ => None,
                    };
                    instrs.push((rel(offset), local));
                }
                bodies.push(Body {
                    start: rel(body.range().start),
                    instrs,
                    end: rel(body.range().end),
                });
            }
            _ => {}
        }
    }
    let mut bodies: Vec<_> = bodies.into_iter().map(Some).collect();
    exports
        .into_iter()
        .map(|(name, index)| (name, bodies[index].take().unwrap()))
        .collect()
}

fn wasm_local(index: u64) -> Expression {
    let mut e = Expression::new();
    e.op_wasm_local(index as u32);
    e.op(constants::DW_OP_stack_value);
    e
}

/// The module with DWARF describing it.
fn input() -> Vec<u8> {
    let mut wasm = wat::parse_str(WAT).unwrap();
    let bodies = bodies(&wasm);
    let (f, g) = (&bodies["f"], &bodies["g"]);

    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    let source = LineString::String(b"passes.c".to_vec());
    let mut program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(b"/src".to_vec()),
        None,
        source.clone(),
        None,
    );
    let file = program.add_file(source, program.default_directory(), None);
    for (body, lines) in [
        (f, (0..f.instrs.len()).map(line).collect::<Vec<_>>()),
        (g, vec![G_LINE]),
    ] {
        program.begin_sequence(Some(Address::Constant(body.start)));
        for (i, &line) in lines.iter().enumerate() {
            let row = program.row();
            row.file = file;
            row.line = line;
            row.address_offset = body.addr(i) - body.start;
            program.generate_row();
        }
        program.end_sequence(body.end - body.start);
    }
    dwarf.unit.line_program = program;

    let root = dwarf.unit.root();
    let unit = &mut dwarf.unit;
    unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );
    unit.get_mut(root)
        .set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

    let subprogram = unit.add(root, constants::DW_TAG_subprogram);
    let e = unit.get_mut(subprogram);
    e.set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(f.start)),
    );
    e.set(
        constants::DW_AT_high_pc,
        AttributeValue::Udata(f.end - f.start),
    );

    let block = unit.add(subprogram, constants::DW_TAG_lexical_block);
    let e = unit.get_mut(block);
    e.set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(f.addr(BLOCK.0))),
    );
    e.set(
        constants::DW_AT_high_pc,
        AttributeValue::Udata(f.addr(BLOCK.1) - f.addr(BLOCK.0)),
    );

    let list = unit.locations.add(LocationList(vec![Location::StartEnd {
        begin: Address::Constant(f.addr(Y.0)),
        end: Address::Constant(f.addr(Y.1)),
        data: wasm_local(2),
    }]));
    let mut addr = Expression::new();
    addr.op_addr(Address::Constant(STATIC));
    for (name, location) in [
        ("x", AttributeValue::Exprloc(wasm_local(1))),
        ("y", AttributeValue::LocationListRef(list)),
        ("s", AttributeValue::Exprloc(addr)),
    ] {
        let variable = unit.add(subprogram, constants::DW_TAG_variable);
        let e = unit.get_mut(variable);
        e.set(constants::DW_AT_name, AttributeValue::String(name.into()));
        e.set(constants::DW_AT_location, location);
    }

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    sections
        .for_each(|id, data| {
            if !data.slice().is_empty() {
                wasm.push(wasm_encoder::SectionId::Custom as u8);
                wasm_encoder::Encode::encode(
                    &CustomSection {
                        name: Cow::Borrowed(id.name()),
                        data: Cow::Borrowed(data.slice()),
                    },
                    &mut wasm,
                );
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    wasm
}

/// What the output DWARF says.
#[derive(Debug, Default)]
struct Output {
    /// (address, line) of every row, sorted.
    lines: Vec<(u64, u64)>,
    /// The ranges of the subprogram and the lexical block.
    subprogram: Vec<(u64, u64)>,
    block: Vec<(u64, u64)>,
    /// The local index in `x`, `y`'s location list entries as (begin, end,
    /// local index), and `s`'s address.
    x: Option<u64>,
    y: Vec<(u64, u64, u64)>,
    s: Option<u64>,
}

fn local_of(expression: read::Expression<EndianSlice<'_, LittleEndian>>, unit: &Unit) -> u64 {
    let mut ops = expression.operations(unit.encoding());
    while let Some(op) = ops.next().unwrap() {
        if let read::Operation::WasmLocal { index } = op {
            return index.into();
        }
    }
    panic!("no DW_OP_WASM_location")
}

type Unit<'a> = read::Unit<EndianSlice<'a, LittleEndian>>;

fn output(wasm: &[u8]) -> Output {
    let sections: Vec<(String, Vec<u8>)> = Parser::new(0)
        .parse_all(wasm)
        .filter_map(|p| match p.unwrap() {
            Payload::CustomSection(s) => Some((s.name().to_string(), s.data().to_vec())),
            _ => None,
        })
        .collect();
    let dwarf = read::Dwarf::load(|id| {
        let data = sections
            .iter()
            .find(|(name, _)| name == id.name())
            .map_or(&[][..], |(_, data)| data.as_slice());
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })
    .unwrap();
    let unit = dwarf.unit(dwarf.units().next().unwrap().unwrap()).unwrap();
    let mut out = Output::default();

    let mut rows = unit.line_program.clone().unwrap().rows();
    while let Some((_, row)) = rows.next_row().unwrap() {
        if !row.end_sequence() {
            out.lines.push((row.address(), row.line().unwrap().get()));
        }
    }
    out.lines.sort_unstable();

    let mut entries = unit.entries();
    while let Some(entry) = entries.next_dfs().unwrap() {
        let ranges = || {
            let mut ranges = Vec::new();
            let mut it = dwarf.die_ranges(&unit, entry).unwrap();
            while let Some(r) = it.next().unwrap() {
                ranges.push((r.begin, r.end));
            }
            ranges.sort_unstable();
            ranges
        };
        match entry.tag() {
            constants::DW_TAG_subprogram => out.subprogram = ranges(),
            constants::DW_TAG_lexical_block => out.block = ranges(),
            constants::DW_TAG_variable => {
                let name = dwarf
                    .attr_string(&unit, entry.attr_value(constants::DW_AT_name).unwrap())
                    .unwrap();
                let Some(location) = entry.attr_value(constants::DW_AT_location) else {
                    continue;
                };
                match &*name {
                    b"x" => {
                        let read::AttributeValue::Exprloc(e) = location else {
                            panic!("{location:?}")
                        };
                        out.x = Some(local_of(e, &unit));
                    }
                    b"s" => {
                        let read::AttributeValue::Exprloc(e) = location else {
                            panic!("{location:?}")
                        };
                        let mut ops = e.operations(unit.encoding());
                        if let Some(read::Operation::Address { address }) = ops.next().unwrap() {
                            out.s = Some(address);
                        }
                    }
                    b"y" => {
                        let mut list = dwarf.attr_locations(&unit, location).unwrap().unwrap();
                        while let Some(l) = list.next().unwrap() {
                            out.y
                                .push((l.range.begin, l.range.end, local_of(l.data, &unit)));
                        }
                        out.y.sort_unstable();
                    }
                    _ => unreachable!(),
                }
            }
            _ => {}
        }
    }
    out
}

fn function(module: &Module, name: &str) -> FunctionId {
    module.exports.get_func(name).unwrap()
}

/// Parse with DWARF, run `pass`, emit, and return the input and output
/// bodies and the output DWARF.
fn run(pass: impl FnOnce(&mut Module)) -> (BTreeMap<String, Body>, BTreeMap<String, Body>, Output) {
    let wasm = input();
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(&wasm).unwrap();
    pass(&mut module);
    let output_wasm = module.emit_wasm();
    (bodies(&wasm), bodies(&output_wasm), output(&output_wasm))
}

/// The local index that output instruction `i` of `body` uses.
fn local_at(body: &Body, i: usize) -> u64 {
    body.instrs[i].1.unwrap().into()
}

#[test]
fn round_trip() {
    let (input, out, dwarf) = run(|_| {});
    let (f, g) = (&out["f"], &out["g"]);
    assert_eq!(f.instrs.len(), input["f"].instrs.len());

    let mut lines: Vec<_> = (0..f.instrs.len()).map(|i| (f.addr(i), line(i))).collect();
    lines.push((g.addr(0), G_LINE));
    lines.sort_unstable();
    assert_eq!(dwarf.lines, lines);
    assert_eq!(dwarf.subprogram, [(f.start, f.end)]);
    assert_eq!(dwarf.block, [(f.addr(BLOCK.0), f.addr(BLOCK.1))]);
    // Instruction 1 is `local.set 1`, and instruction 5 `local.set 2`.
    assert_eq!(dwarf.x, Some(local_at(f, 1)));
    assert_eq!(dwarf.y, [(f.addr(Y.0), f.addr(Y.1), local_at(f, 5))]);
    // A linear memory address, not a code address.
    assert_eq!(dwarf.s, Some(STATIC));
}

#[test]
fn inserted_code() {
    // Two instructions with no input location at the start of `f`.
    let (_, out, dwarf) = run(|module| {
        let id = function(module, "f");
        let func = module.funcs.get_mut(id).kind.unwrap_local_mut();
        let entry = func.entry_block();
        let instrs = &mut func.block_mut(entry).instrs;
        let constant = Instr::Const(walrus::ir::Const {
            value: walrus::ir::Value::I32(0),
        });
        instrs.insert(0, (constant, Default::default()));
        instrs.insert(1, (Instr::Drop(walrus::ir::Drop {}), Default::default()));
    });
    let (f, g) = (&out["f"], &out["g"]);
    // Input instruction `i` is now output instruction `i + 2`.
    let at = |i: usize| f.addr(i + 2);

    let mut lines: Vec<_> = (0..f.instrs.len() - 2).map(|i| (at(i), line(i))).collect();
    lines.push((g.addr(0), G_LINE));
    lines.sort_unstable();
    assert_eq!(dwarf.lines, lines);
    assert_eq!(dwarf.subprogram, [(f.start, f.end)]);
    assert_eq!(dwarf.block, [(at(BLOCK.0), at(BLOCK.1))]);
    assert_eq!(dwarf.y, [(at(Y.0), at(Y.1), local_at(f, Y.0 + 2))]);
}

#[test]
fn copied_code() {
    // Copy `i32.const 1; i32.add`, with their locations, to the end of `g`,
    // which then returns 8.
    let (_, out, dwarf) = run(|module| {
        let (f, g) = (function(module, "f"), function(module, "g"));
        let f = module.funcs.get(f).kind.unwrap_local();
        let copy: Vec<_> = f.block(f.entry_block()).instrs[BLOCK.0..BLOCK.1].to_vec();
        let g = module.funcs.get_mut(g).kind.unwrap_local_mut();
        let entry = g.entry_block();
        g.block_mut(entry).instrs.extend(copy);
    });
    let (f, g) = (&out["f"], &out["g"]);
    assert_eq!(g.instrs.len(), 4);

    let mut lines: Vec<_> = (0..f.instrs.len()).map(|i| (f.addr(i), line(i))).collect();
    lines.push((g.addr(0), G_LINE));
    lines.extend((BLOCK.0..BLOCK.1).map(|i| (g.addr(1 + i - BLOCK.0), line(i))));
    // `g`'s own `end` follows the copy.
    lines.push((g.addr(3), G_LINE));
    lines.sort_unstable();
    assert_eq!(dwarf.lines, lines);
    // The block is in both functions now, so its pair of addresses became a
    // range list.
    let mut block = vec![(f.addr(BLOCK.0), f.addr(BLOCK.1)), (g.addr(1), g.addr(3))];
    block.sort_unstable();
    assert_eq!(dwarf.block, block);
}

#[test]
fn coalesced_locals() {
    // Use local 1 instead of local 2, which is then no longer emitted.
    let (_, out, dwarf) = run(|module| {
        let id = function(module, "f");
        let func = module.funcs.get_mut(id).kind.unwrap_local_mut();
        let entry = func.entry_block();
        let instrs = &mut func.block_mut(entry).instrs;
        let (Instr::LocalSet(LocalSet { local: one }), Instr::LocalSet(LocalSet { local: two })) =
            (&instrs[1].0, &instrs[5].0)
        else {
            unreachable!()
        };
        let (one, two) = (*one, *two);
        for (instr, _) in instrs.iter_mut() {
            match instr {
                Instr::LocalGet(LocalGet { local }) | Instr::LocalSet(LocalSet { local })
                    if *local == two =>
                {
                    *local = one;
                }
                _ => {}
            }
        }
    });
    let f = &out["f"];
    assert_eq!(local_at(f, 5), local_at(f, 1));
    assert_eq!(dwarf.x, Some(local_at(f, 1)));
    assert_eq!(dwarf.y, [(f.addr(Y.0), f.addr(Y.1), local_at(f, 1))]);
}
