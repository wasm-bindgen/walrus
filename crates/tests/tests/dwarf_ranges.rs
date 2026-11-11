//! Range lists hold offsets from a base address as well as addresses: DWARF 4
//! `.debug_ranges` pairs in a unit with a non-zero base or after a base
//! address entry, and DWARF 5 `DW_RLE_offset_pair`. walrus must move the
//! ranges they describe with the code, not treat the offsets as addresses.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, LineProgram, Range, RangeList, Sections,
};
use gimli::{constants, read, Encoding, EndianSlice, Format, LittleEndian};
use std::borrow::Cow;
use std::collections::HashMap;
use wasm_encoder::{CodeSection, CustomSection, Function, FunctionSection, Module, TypeSection};
use wasmparser::{Parser, Payload};

/// `i32.const 0; drop; i32.const 0; drop; end`. Relative to the start of
/// its body, at the local declarations, the instructions start at 1, 3, 4, 6
/// and 7.
const BODY: [u8; 7] = [0x41, 0x00, 0x1a, 0x41, 0x00, 0x1a, 0x0b];

/// A module with a function whose `i32.const` immediates are padded, which
/// walrus writes shorter, so the second function, `BODY`, moves.
fn module(custom: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut padded = Function::new([]);
    for _ in 0..10 {
        padded.raw([0x41, 0x80, 0x80, 0x80, 0x80, 0x00, 0x1a]);
    }
    padded.raw([0x0b]);
    code.function(&padded);
    let mut second = Function::new([]);
    second.raw(BODY);
    code.function(&second);
    functions.function(0).function(0);
    let mut module = Module::new();
    module.section(&types).section(&functions).section(&code);
    for (name, data) in custom {
        module.section(&CustomSection {
            name: Cow::Borrowed(name),
            data: Cow::Borrowed(data),
        });
    }
    module.finish()
}

/// The code-section-relative starts of the two functions' bodies.
fn bodies(wasm: &[u8]) -> (u64, u64) {
    let mut code_start = 0;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => bodies.push(body.range().start - code_start),
            _ => {}
        }
    }
    (bodies[0] as u64, bodies[1] as u64)
}

/// DWARF with a unit whose base address is `base` and a subprogram with
/// `ranges`.
fn dwarf(version: u16, base: u64, ranges: Vec<Range>) -> Vec<(&'static str, Vec<u8>)> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::none();
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(base)),
    );
    let list = dwarf.unit.ranges.add(RangeList(ranges));
    let subprogram = dwarf.unit.add(root, constants::DW_TAG_subprogram);
    dwarf
        .unit
        .get_mut(subprogram)
        .set(constants::DW_AT_ranges, AttributeValue::RangeListRef(list));
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

/// The subprogram's ranges, as readers resolve them.
fn subprogram_ranges(wasm: &[u8]) -> Vec<(u64, u64)> {
    let mut sections = HashMap::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::CustomSection(s) = payload.unwrap() {
            sections.insert(s.name().to_string(), s.data());
        }
    }
    let dwarf = read::Dwarf::load(|id| {
        let data = sections.get(id.name()).copied().unwrap_or_default();
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })
    .unwrap();
    let unit = dwarf.unit(dwarf.units().next().unwrap().unwrap()).unwrap();
    let mut entries = unit.entries();
    while let Some((_, entry)) = entries.next_dfs().unwrap() {
        if entry.tag() == constants::DW_TAG_subprogram {
            let mut ranges = dwarf.die_ranges(&unit, entry).unwrap();
            let mut out = Vec::new();
            while let Some(range) = ranges.next().unwrap() {
                out.push((range.begin, range.end));
            }
            return out;
        }
    }
    panic!("no subprogram");
}

/// Round-trips the module with DWARF from `dwarf`, given a base and the
/// second body's start relative to it, and checks that the subprogram covers
/// the second body's first and second `i32.const; drop` after walrus shrinks
/// the first body. The base is the first body's start, then the second's.
fn check(dwarf: impl Fn(u64, u64) -> Vec<(&'static str, Vec<u8>)>) {
    let (first, start) = bodies(&module(&[]));
    for base in [first, start] {
        let wasm = module(&dwarf(base, start - base));
        let mut config = walrus::ModuleConfig::new();
        config.generate_dwarf(true);
        let emitted = config.parse(&wasm).unwrap().emit_wasm();
        let (_, new_start) = bodies(&emitted);
        assert!(new_start < start, "the second function moves");
        assert_eq!(
            subprogram_ranges(&emitted),
            [
                (new_start + 1, new_start + 3),
                (new_start + 4, new_start + 7)
            ],
            "base {base:#x}"
        );
    }
}

/// Offset pairs for the second body's instructions at `start` past the base.
fn offsets(start: u64) -> Vec<Range> {
    [(1, 3), (4, 7)]
        .map(|(begin, end)| Range::OffsetPair {
            begin: start + begin,
            end: start + end,
        })
        .to_vec()
}

fn base_address(base: u64) -> Range {
    Range::BaseAddress {
        address: Address::Constant(base),
    }
}

#[test]
fn dwarf4_offsets_from_the_unit_base() {
    check(|base, start| dwarf(4, base, offsets(start)));
}

#[test]
fn dwarf4_offsets_after_a_base_address_entry() {
    check(|base, start| {
        let mut ranges = vec![base_address(base)];
        ranges.extend(offsets(start));
        dwarf(4, 0, ranges)
    });
}

#[test]
fn dwarf5_offset_pairs_from_the_unit_base() {
    check(|base, start| dwarf(5, base, offsets(start)));
}

#[test]
fn dwarf5_offset_pairs_after_a_base_address_entry() {
    check(|base, start| {
        let mut ranges = vec![base_address(base)];
        ranges.extend(offsets(start));
        dwarf(5, 0, ranges)
    });
}

#[test]
fn dwarf5_addresses_in_a_unit_with_a_base() {
    // Start/end pairs are addresses, whatever the base.
    check(|base, start| {
        let ranges = [(1, 3), (4, 7)].map(|(begin, end)| Range::StartEnd {
            begin: Address::Constant(base + start + begin),
            end: Address::Constant(base + start + end),
        });
        dwarf(5, base, ranges.to_vec())
    });
}
