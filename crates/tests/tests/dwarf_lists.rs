//! walrus reorders functions and changes instruction sizes, so it must move
//! each code address in DWARF to its instruction's new address. These tests
//! write DWARF the way LLVM does (a base address and offsets from it in
//! range and location lists, one line sequence covering several functions,
//! file index 0 in DWARF 5) and check every converted address against the
//! instruction it named.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, LineProgram, LineString, Location,
    LocationList, Range, RangeList, Sections,
};
use gimli::{constants, read, Encoding, EndianSlice, Format, LineEncoding, LittleEndian};
use std::borrow::Cow;
use wasm_encoder::{CodeSection, CustomSection, Function, FunctionSection, Module, TypeSection};
use wasmparser::{Parser, Payload};

/// Two functions. walrus emits the larger one first, so it swaps them, and
/// it shortens the second one's padded immediates.
fn code() -> Module {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    // `i32.const 0; drop; end`
    let mut small = Function::new([]);
    small.raw([0x41, 0x00, 0x1a, 0x0b]);
    // `(i32.const 0; drop) x 20; end`, with each immediate padded to five
    // bytes as linkers write relocatable ones.
    let mut large = Function::new([]);
    let mut body = [0x41, 0x80, 0x80, 0x80, 0x80, 0x00, 0x1a].repeat(20);
    body.push(0x0b);
    large.raw(body);
    for f in [&small, &large] {
        code.function(f);
        functions.function(0);
    }
    let mut module = Module::new();
    module.section(&types).section(&functions).section(&code);
    module
}

/// Code-section-relative addresses of each function's body start, its
/// instructions and its end.
struct Body {
    start: u64,
    instrs: Vec<u64>,
    end: u64,
}

fn bodies(wasm: &[u8]) -> Vec<Body> {
    let mut code_start = 0;
    let mut out = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                let rel = |a: usize| (a - code_start) as u64;
                let mut ops = body.get_operators_reader().unwrap();
                let mut instrs = Vec::new();
                while !ops.eof() {
                    instrs.push(rel(ops.read_with_offset().unwrap().1));
                }
                out.push(Body {
                    start: rel(body.range().start),
                    instrs,
                    end: rel(body.range().end),
                });
            }
            _ => {}
        }
    }
    out
}

/// What the DWARF points at: (function, instruction) pairs.
type At = (usize, usize);
const RANGE: (At, At) = ((1, 2), (1, 9));
const LOCATION: (At, At) = ((1, 4), (1, 13));
/// Line rows: (function, instruction, line).
const ROWS: [(usize, usize, u64); 4] = [(0, 0, 10), (0, 1, 11), (1, 0, 20), (1, 6, 26)];

fn dwarf(version: u16, input: &[Body]) -> Vec<(&'static str, Vec<u8>)> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version,
        address_size: 4,
    };
    let addr = |(f, i): At| input[f].instrs[i];
    let mut dwarf = DwarfUnit::new(encoding);
    let source = LineString::String(b"lists.c".to_vec());
    let mut program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(b"/src".to_vec()),
        None,
        source.clone(),
        None,
    );
    // In DWARF 5 this is file index 0, as clang uses.
    let file = program.add_file(source, program.default_directory(), None);
    // One sequence over both functions, as for a unit without function
    // sections.
    program.begin_sequence(Some(Address::Constant(input[0].start)));
    for (f, i, line) in ROWS {
        let row = program.row();
        row.file = file;
        row.line = line;
        row.address_offset = addr((f, i)) - input[0].start;
        program.generate_row();
    }
    program.end_sequence(input[1].end - input[0].start);
    dwarf.unit.line_program = program;

    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );
    dwarf
        .unit
        .get_mut(root)
        .set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);
    // Base address at the function start, then offsets from it.
    let base = input[1].start;
    let ranges = dwarf.unit.ranges.add(RangeList(vec![
        Range::BaseAddress {
            address: Address::Constant(base),
        },
        Range::OffsetPair {
            begin: addr(RANGE.0) - base,
            end: addr(RANGE.1) - base,
        },
    ]));
    let block = dwarf.unit.add(root, constants::DW_TAG_lexical_block);
    dwarf.unit.get_mut(block).set(
        constants::DW_AT_ranges,
        AttributeValue::RangeListRef(ranges),
    );
    // A constant, since neither function has locals.
    let mut expr = Expression::new();
    expr.op_constu(1);
    expr.op(constants::DW_OP_stack_value);
    let locations = dwarf.unit.locations.add(LocationList(vec![
        Location::BaseAddress {
            address: Address::Constant(base),
        },
        Location::OffsetPair {
            begin: addr(LOCATION.0) - base,
            end: addr(LOCATION.1) - base,
            data: expr,
        },
    ]));
    let var = dwarf.unit.add(block, constants::DW_TAG_variable);
    dwarf.unit.get_mut(var).set(
        constants::DW_AT_location,
        AttributeValue::LocationListRef(locations),
    );

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

fn custom_sections(wasm: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::CustomSection(s) = payload.unwrap() {
            out.push((s.name().to_string(), s.data().to_vec()));
        }
    }
    out
}

fn check(version: u16) {
    let input = bodies(&code().finish());
    let mut module = code();
    for (name, data) in dwarf(version, &input) {
        module.section(&CustomSection {
            name: Cow::Borrowed(name),
            data: Cow::Borrowed(&data),
        });
    }
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let output = config.parse(&module.finish()).unwrap().emit_wasm();

    let new = bodies(&output);
    // Walrus emitted the large function first.
    assert!(new[0].instrs.len() > new[1].instrs.len());
    let new_addr = |(f, i): At| new[1 - f].instrs[i];

    let sections = custom_sections(&output);
    let dwarf = read::Dwarf::load(|id| {
        let data = sections
            .iter()
            .find(|(name, _)| name == id.name())
            .map_or(&[][..], |(_, data)| data.as_slice());
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })
    .unwrap();
    let unit = dwarf.unit(dwarf.units().next().unwrap().unwrap()).unwrap();

    let mut ranges = Vec::new();
    let mut locations = Vec::new();
    let mut entries = unit.entries();
    while let Some(entry) = entries.next_dfs().unwrap() {
        if let Some(value) = entry.attr_value(constants::DW_AT_ranges) {
            let offset = dwarf.attr_ranges_offset(&unit, value).unwrap().unwrap();
            let mut it = dwarf.ranges(&unit, offset).unwrap();
            while let Some(r) = it.next().unwrap() {
                ranges.push((r.begin, r.end));
            }
        }
        if let Some(value) = entry.attr_value(constants::DW_AT_location) {
            let mut it = dwarf.attr_locations(&unit, value).unwrap().unwrap();
            while let Some(l) = it.next().unwrap() {
                locations.push((l.range.begin, l.range.end));
            }
        }
    }
    assert_eq!(ranges, [(new_addr(RANGE.0), new_addr(RANGE.1))]);
    assert_eq!(locations, [(new_addr(LOCATION.0), new_addr(LOCATION.1))]);

    let program = unit.line_program.clone().unwrap();
    let mut rows = program.rows();
    let mut lines = Vec::new();
    let mut last = None;
    while let Some((header, row)) = rows.next_row().unwrap() {
        if row.end_sequence() {
            last = None;
            continue;
        }
        // Rows never go backwards within a sequence.
        assert!(last.is_none_or(|a| a <= row.address()));
        last = Some(row.address());
        let file = row.file(header).unwrap();
        let name = dwarf.attr_string(&unit, file.path_name()).unwrap();
        assert_eq!(&*name, b"lists.c");
        lines.push((row.address(), row.line().unwrap().get()));
    }
    lines.sort_unstable();
    let mut expected: Vec<_> = ROWS
        .iter()
        .map(|&(f, i, line)| (new_addr((f, i)), line))
        .collect();
    expected.sort_unstable();
    assert_eq!(lines, expected);
}

#[test]
fn dwarf4_lists_and_lines_follow_moved_code() {
    check(4);
}

#[test]
fn dwarf5_lists_and_lines_follow_moved_code() {
    check(5);
}

/// A DWARF 5 location list's `DW_LLE_default_location` is kept: it applies
/// wherever no other entry does, so it has no addresses to move.
#[test]
fn dwarf5_default_location_is_kept() {
    let input = bodies(&code().finish());
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );
    // `DW_OP_lit<n>; DW_OP_stack_value`.
    let constant = |n: u8| {
        let mut expr = Expression::new();
        expr.op(constants::DwOp(constants::DW_OP_lit0.0 + n));
        expr.op(constants::DW_OP_stack_value);
        expr
    };
    let locations = dwarf.unit.locations.add(LocationList(vec![
        Location::StartEnd {
            begin: Address::Constant(input[1].instrs[4]),
            end: Address::Constant(input[1].instrs[13]),
            data: constant(0),
        },
        Location::DefaultLocation { data: constant(1) },
    ]));
    let var = dwarf.unit.add(root, constants::DW_TAG_variable);
    dwarf.unit.get_mut(var).set(
        constants::DW_AT_location,
        AttributeValue::LocationListRef(locations),
    );
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    let mut module = code();
    sections
        .for_each(|id, data| {
            if !data.slice().is_empty() {
                module.section(&CustomSection {
                    name: Cow::Borrowed(id.name()),
                    data: Cow::Borrowed(data.slice()),
                });
            }
            Ok::<_, ()>(())
        })
        .unwrap();

    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let output = config.parse(&module.finish()).unwrap().emit_wasm();
    let new = bodies(&output);
    let sections = custom_sections(&output);
    let dwarf = read::Dwarf::load(|id| {
        let data = sections
            .iter()
            .find(|(name, _)| name == id.name())
            .map_or(&[][..], |(_, data)| data.as_slice());
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })
    .unwrap();
    let unit = dwarf.unit(dwarf.units().next().unwrap().unwrap()).unwrap();
    let mut entries = unit.entries();
    let mut found = Vec::new();
    while let Some(entry) = entries.next_dfs().unwrap() {
        let Some(value) = entry.attr_value(constants::DW_AT_location) else {
            continue;
        };
        let offset = dwarf.attr_locations_offset(&unit, value).unwrap().unwrap();
        let mut raw = dwarf.raw_locations(&unit, offset).unwrap();
        while let Some(entry) = raw.next().unwrap() {
            found.push(match entry {
                read::RawLocListEntry::StartEnd { begin, end, data } => {
                    (Some((begin, end)), data.0.to_vec())
                }
                read::RawLocListEntry::DefaultLocation { data } => (None, data.0.to_vec()),
                other => panic!("unexpected entry {other:?}"),
            });
        }
    }
    // The large function is emitted first.
    assert_eq!(
        found,
        [
            (
                Some((new[0].instrs[4], new[0].instrs[13])),
                vec![0x30, 0x9f]
            ),
            (None, vec![0x31, 0x9f]),
        ]
    );
}
