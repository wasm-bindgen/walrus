//! walrus renumbers locals when it emits a function (grouping them by type
//! and dropping unused ones), and gc can remove globals, so the indices in
//! `DW_OP_WASM_location` must be remapped to name the same local or global.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, LineProgram, Location, LocationList,
    Sections,
};
use gimli::{constants, read, Encoding, EndianSlice, Format, LittleEndian};
use std::borrow::Cow;
use wasm_encoder::CustomSection;
use wasmparser::{Operator, Parser, Payload};

const WAT: &str = r#"
(module
  (global $unused (mut i32) (i32.const 0))
  (global $sp (mut i32) (i32.const 0))
  (func (export "f") (param i32) (result i32)
    (local i64 i32 i64 f32)
    i64.const 1
    local.set 1
    i32.const 2
    local.set 2
    i64.const 3
    local.set 3
    local.get 1
    local.get 3
    i64.add
    i32.wrap_i64
    local.get 2
    i32.add
    global.get $sp
    i32.add
    local.get 0
    i32.add))
"#;

/// The function's body range, code-section relative, after the size before
/// it, and the local indices it sets and global index it reads, in order.
fn function(wasm: &[u8]) -> ((u64, u64), Vec<u32>, u32) {
    let mut code_start = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                let range = (
                    (range.start - code_start) as u64,
                    (range.end - code_start) as u64,
                );
                let mut sets = Vec::new();
                let mut global = None;
                for op in body.get_operators_reader().unwrap() {
                    match op.unwrap() {
                        Operator::LocalSet { local_index } => sets.push(local_index),
                        Operator::GlobalGet { global_index } => global = Some(global_index),
                        _ => {}
                    }
                }
                return (range, sets, global.unwrap());
            }
            _ => {}
        }
    }
    unreachable!()
}

/// `DW_OP_WASM_location` with a kind and a 1-byte LEB128 index.
fn wasm_location(kind: u8, index: u8) -> Vec<u8> {
    vec![constants::DW_OP_WASM_location.0, kind, index]
}

/// A local's value.
fn local(index: u8) -> Expression {
    let mut e = wasm_location(0, index);
    e.push(constants::DW_OP_stack_value.0);
    Expression::raw(e)
}

fn build_dwarf(body: (u64, u64)) -> Vec<(&'static str, Vec<u8>)> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::none();
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );

    let mut subprogram = |low_pc: u64, frame_base: Vec<u8>| {
        let id = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        let entry = dwarf.unit.get_mut(id);
        entry.set(
            constants::DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(low_pc)),
        );
        entry.set(
            constants::DW_AT_high_pc,
            AttributeValue::Udata(body.1 - body.0),
        );
        entry.set(
            constants::DW_AT_frame_base,
            AttributeValue::Exprloc(Expression::raw(frame_base)),
        );
        id
    };
    // LLVM points `DW_AT_low_pc` at the body, after its size. The frame base
    // is global 1 in its fixed-width form.
    let frame_base = [
        &[constants::DW_OP_WASM_location.0, 3][..],
        &1u32.to_le_bytes(),
    ]
    .concat();
    let live = subprogram(body.0, frame_base);
    // A function the linker discarded, naming a local that is out of range.
    let dead = subprogram(0xffff_ffff, wasm_location(0, 9));

    let list = dwarf
        .unit
        .locations
        .add(LocationList(vec![Location::StartEnd {
            begin: Address::Constant(body.0),
            end: Address::Constant(body.1),
            data: local(3),
        }]));
    let base = dwarf.unit.add(root, constants::DW_TAG_base_type);
    let mut variable = |parent, name: &str, location: AttributeValue| {
        let id = dwarf.unit.add(parent, constants::DW_TAG_variable);
        let entry = dwarf.unit.get_mut(id);
        entry.set(constants::DW_AT_name, AttributeValue::String(name.into()));
        entry.set(constants::DW_AT_location, location);
    };
    for (name, index) in [("one", 1), ("two", 2), ("unused", 4), ("arg", 0)] {
        variable(live, name, AttributeValue::Exprloc(local(index)));
    }
    variable(live, "three", AttributeValue::LocationListRef(list));
    variable(
        live,
        "sp",
        AttributeValue::Exprloc(Expression::raw(wasm_location(1, 1))),
    );
    variable(dead, "dead", AttributeValue::Exprloc(local(9)));
    // A reference to another entry, as LLVM writes for DWARF 5 casts.
    let mut converted = Expression::new();
    converted.op_wasm_local(1);
    converted.op_convert(Some(base));
    converted.op(constants::DW_OP_stack_value);
    variable(live, "converted", AttributeValue::Exprloc(converted));

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    let mut out = Vec::new();
    sections
        .for_each(|id, data| {
            if !data.slice().is_empty() {
                out.push((id.name(), data.slice().to_vec()));
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    out
}

type Unit<'a> = read::Unit<EndianSlice<'a, LittleEndian>>;
/// A variable's `DW_OP_WASM_location`s, or `None` without a location.
type Locations = Option<Vec<(u8, u32)>>;

/// The `DW_OP_WASM_location` operations in an expression as (kind, index),
/// with kind 3 reported as 1 like gimli does, and `DW_OP_convert` as
/// (0xa8, tag of the entry it names).
fn wasm_locations(
    expression: read::Expression<EndianSlice<'_, LittleEndian>>,
    unit: &Unit,
) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut ops = expression.operations(unit.encoding());
    while let Some(op) = ops.next().unwrap() {
        match op {
            read::Operation::WasmLocal { index } => out.push((0, index)),
            read::Operation::WasmGlobal { index } => out.push((1, index)),
            read::Operation::WasmStack { index } => out.push((2, index)),
            read::Operation::Convert { base_type } => {
                let tag = unit.entry(base_type).unwrap().tag();
                out.push((0xa8, tag.0.into()));
            }
            _ => {}
        }
    }
    out
}

#[test]
fn wasm_locations_follow_renumbered_locals_and_globals() {
    let mut wasm = wat::parse_str(WAT).unwrap();
    let (body, old_sets, old_global) = function(&wasm);
    assert_eq!((old_sets.as_slice(), old_global), (&[1, 2, 3][..], 1));
    for (name, data) in build_dwarf(body) {
        wasm.push(wasm_encoder::SectionId::Custom as u8);
        wasm_encoder::Encode::encode(
            &CustomSection {
                name: Cow::Borrowed(name),
                data: Cow::Borrowed(&data),
            },
            &mut wasm,
        );
    }

    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(&wasm).unwrap();
    // Removes `$unused`, so `$sp` becomes global 0.
    walrus::passes::gc::run(&mut module);
    let output = module.emit_wasm();

    let (_, sets, global) = function(&output);
    // walrus emits the i32 local before the i64s.
    assert_ne!(sets, old_sets);
    assert_eq!(global, 0);
    let new_local = |old: u32| match old {
        0 => 0,
        _ => sets[old_sets.iter().position(|&s| s == old).unwrap()],
    };

    let sections: Vec<(String, Vec<u8>)> = Parser::new(0)
        .parse_all(&output)
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

    let mut frame_bases = Vec::new();
    let mut variables = Vec::new();
    let mut entries = unit.entries();
    while let Some(entry) = entries.next_dfs().unwrap() {
        if let Some(read::AttributeValue::Exprloc(e)) =
            entry.attr_value(constants::DW_AT_frame_base)
        {
            frame_bases.push(wasm_locations(e, &unit));
        }
        if entry.tag() != constants::DW_TAG_variable {
            continue;
        }
        let name = match entry.attr_value(constants::DW_AT_name) {
            Some(read::AttributeValue::String(s)) => s.to_string().unwrap().to_string(),
            other => panic!("unexpected name {other:?}"),
        };
        let locations = match entry.attr_value(constants::DW_AT_location) {
            None => None,
            Some(read::AttributeValue::Exprloc(e)) => Some(wasm_locations(e, &unit)),
            Some(value) => {
                let mut list = dwarf.attr_locations(&unit, value).unwrap().unwrap();
                let mut out = Vec::new();
                while let Some(l) = list.next().unwrap() {
                    out.extend(wasm_locations(l.data, &unit));
                }
                Some(out)
            }
        };
        variables.push((name, locations));
    }

    assert_eq!(frame_bases, [vec![(1, 0)], vec![(0, 9)]]);
    let expected: Vec<(&str, Locations)> = vec![
        ("one", Some(vec![(0, new_local(1))])),
        ("two", Some(vec![(0, new_local(2))])),
        // walrus doesn't emit unused locals.
        ("unused", None),
        ("arg", Some(vec![(0, 0)])),
        ("three", Some(vec![(0, new_local(3))])),
        ("sp", Some(vec![(1, 0)])),
        (
            "converted",
            Some(vec![
                (0, new_local(1)),
                (0xa8, constants::DW_TAG_base_type.0.into()),
            ]),
        ),
        // Left as it was.
        ("dead", Some(vec![(0, 9)])),
    ];
    let variables: Vec<_> = variables
        .iter()
        .map(|(name, locations)| (name.as_str(), locations.clone()))
        .collect();
    assert_eq!(variables, expected);
}

/// A new index can take more or fewer bytes than the old one, and branches
/// over it still land on the operation they targeted.
#[test]
fn wasm_location_index_widths() {
    // An i64 local, then 200 i32 locals. walrus emits the i32 locals first,
    // so local 0 becomes 200 and local `k` of the others becomes `k - 1`.
    let mut wat = String::from("(module (func (export \"f\") (local i64)");
    wat.push_str(&" (local i32)".repeat(200));
    wat.push_str(" i64.const 0 local.set 0");
    for i in 1..=200 {
        wat.push_str(&format!(" i32.const 0 local.set {i}"));
    }
    wat.push_str("))");
    let mut wasm = wat::parse_str(&wat).unwrap();

    let body = |wasm: &[u8]| -> (u64, u64) {
        let mut code_start = 0;
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.unwrap() {
                Payload::CodeSectionStart { range, .. } => code_start = range.start,
                Payload::CodeSectionEntry(body) => {
                    let range = body.range();
                    return (
                        (range.start - code_start) as u64,
                        (range.end - code_start) as u64,
                    );
                }
                _ => {}
            }
        }
        unreachable!()
    };
    let (start, end) = body(&wasm);

    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::none();
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );
    let subprogram = dwarf.unit.add(root, constants::DW_TAG_subprogram);
    let entry = dwarf.unit.get_mut(subprogram);
    entry.set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(start)),
    );
    entry.set(constants::DW_AT_high_pc, AttributeValue::Udata(end - start));
    let op = constants::DW_OP_WASM_location.0;
    let stack_value = constants::DW_OP_stack_value.0;
    // `DW_OP_lit0; DW_OP_bra` over the next `n` bytes.
    let bra = |n: u8| [constants::DW_OP_lit0.0, constants::DW_OP_bra.0, n, 0];
    let variables: [(&str, Vec<u8>); 4] = [
        // Local 128 becomes 127, which takes 1 byte instead of 2.
        ("shrinks", vec![op, 0, 0x80, 0x01, stack_value]),
        // Local 0 becomes 200, which needs 2.
        ("grows", vec![op, 0, 0, stack_value]),
        (
            "grows_after_branch",
            [&bra(3)[..], &[op, 0, 0, stack_value]].concat(),
        ),
        (
            "shrinks_after_branch",
            [&bra(4)[..], &[op, 0, 0x80, 0x01, stack_value]].concat(),
        ),
    ];
    for (name, expression) in &variables {
        let id = dwarf.unit.add(subprogram, constants::DW_TAG_variable);
        let entry = dwarf.unit.get_mut(id);
        entry.set(
            constants::DW_AT_name,
            AttributeValue::String((*name).into()),
        );
        entry.set(
            constants::DW_AT_location,
            AttributeValue::Exprloc(Expression::raw(expression.clone())),
        );
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

    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let output = config.parse(&wasm).unwrap().emit_wasm();
    // The i64 local is now last.
    let mut sets = Vec::new();
    for payload in Parser::new(0).parse_all(&output) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            for op in body.get_operators_reader().unwrap() {
                if let Operator::LocalSet { local_index } = op.unwrap() {
                    sets.push(local_index);
                }
            }
        }
    }
    assert_eq!((sets[0], sets[128]), (200, 127));

    let sections: Vec<(String, Vec<u8>)> = Parser::new(0)
        .parse_all(&output)
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
    let mut found = Vec::new();
    let mut entries = unit.entries();
    while let Some(entry) = entries.next_dfs().unwrap() {
        if entry.tag() != constants::DW_TAG_variable {
            continue;
        }
        let name = match entry.attr_value(constants::DW_AT_name) {
            Some(read::AttributeValue::String(s)) => s.to_string().unwrap().to_string(),
            other => panic!("unexpected name {other:?}"),
        };
        let location = match entry.attr_value(constants::DW_AT_location) {
            None => None,
            Some(read::AttributeValue::Exprloc(e)) => Some(e.0.to_vec()),
            Some(other) => panic!("unexpected location {other:?}"),
        };
        found.push((name, location));
    }
    let expected = [
        ("shrinks", Some(vec![op, 0, 0x7f, stack_value])),
        ("grows", Some(vec![op, 0, 0xc8, 0x01, stack_value])),
        (
            "grows_after_branch",
            Some([&bra(4)[..], &[op, 0, 0xc8, 0x01, stack_value]].concat()),
        ),
        (
            "shrinks_after_branch",
            Some([&bra(3)[..], &[op, 0, 0x7f, stack_value]].concat()),
        ),
    ];
    let found: Vec<_> = found.iter().map(|(n, l)| (n.as_str(), l.clone())).collect();
    assert_eq!(found, expected);
}
