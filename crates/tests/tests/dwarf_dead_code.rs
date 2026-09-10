//! DWARF addresses inside code that walrus removes must not be written as
//! `-1`: in DWARF 4 `.debug_loc`/`.debug_ranges` with 4-byte addresses, a
//! `begin` of `-1` is the base address selection marker, which desyncs every
//! reader of the rest of the section.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, LineProgram, Location, LocationList,
    Sections,
};
use gimli::{
    constants, read, Encoding, EndianSlice, Format, LittleEndian, RawLocListEntry, SectionId,
};
use wasmparser::{Parser, Payload};

const WAT: &str = r#"
(module
  (func $live (export "live") (result i32) i32.const 1 i32.const 2 i32.add)
  (func $dead (result i32) i32.const 3 i32.const 4 i32.add))
"#;

/// Code-section-relative address ranges of each function body, in order.
fn function_ranges(wasm: &[u8]) -> Vec<(u64, u64)> {
    let mut code_start = 0;
    let mut ranges = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                ranges.push((
                    (range.start - code_start) as u64,
                    (range.end - code_start) as u64,
                ));
            }
            _ => {}
        }
    }
    ranges
}

fn debug_sections(wasm: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::CustomSection(s) = payload.unwrap() {
            if s.name().starts_with(".debug_") {
                out.push((s.name().to_string(), s.data().to_vec()));
            }
        }
    }
    out
}

fn build_dwarf(live: (u64, u64), dead: (u64, u64)) -> Sections<EndianVec<LittleEndian>> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::none();
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );

    let mut variable = |name: &str, (begin, end): (u64, u64)| {
        let list = dwarf
            .unit
            .locations
            .add(LocationList(vec![Location::StartEnd {
                begin: Address::Constant(begin),
                end: Address::Constant(end),
                data: Expression::new(),
            }]));
        let var = dwarf.unit.add(root, constants::DW_TAG_variable);
        let var = dwarf.unit.get_mut(var);
        var.set(constants::DW_AT_name, AttributeValue::String(name.into()));
        var.set(
            constants::DW_AT_location,
            AttributeValue::LocationListRef(list),
        );
    };
    // Straddles removed code: begin lands in `dead`, end in `live`.
    variable("straddle", (dead.0 + 1, live.1));
    // Entirely inside live code, after the tombstoned entry in section order.
    variable("live", (live.0 + 1, live.1));

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    sections
}

#[test]
fn removed_code_tombstone_is_not_base_address_marker() {
    let mut wasm = wat::parse_str(WAT).unwrap();
    let ranges = function_ranges(&wasm);
    assert_eq!(ranges.len(), 2);
    let (live, dead) = (ranges[0], ranges[1]);

    let sections = build_dwarf(live, dead);
    sections
        .for_each(|id: SectionId, data| -> Result<(), ()> {
            if !data.slice().is_empty() {
                let section = wasm_encoder::CustomSection {
                    name: id.name().into(),
                    data: data.slice().into(),
                };
                wasm.push(wasm_encoder::SectionId::Custom as u8);
                wasm_encoder::Encode::encode(&section, &mut wasm);
            }
            Ok(())
        })
        .unwrap();

    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(&wasm).unwrap();
    // Removes `$dead`, so its addresses no longer map anywhere.
    walrus::passes::gc::run(&mut module);
    let output = module.emit_wasm();

    let out_sections = debug_sections(&output);
    let load = |id: SectionId| -> Result<EndianSlice<'_, LittleEndian>, ()> {
        let data = out_sections
            .iter()
            .find(|(name, _)| name == id.name())
            .map(|(_, data)| data.as_slice())
            .unwrap_or(&[]);
        Ok(EndianSlice::new(data, LittleEndian))
    };
    let dwarf = read::Dwarf::load(load).unwrap();

    let mut lists = 0;
    let mut units = dwarf.units();
    while let Some(header) = units.next().unwrap() {
        let unit = dwarf.unit(header).unwrap();
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs().unwrap() {
            let offset = match entry.attr_value(constants::DW_AT_location).unwrap() {
                Some(read::AttributeValue::LocationListsRef(offset)) => offset,
                _ => continue,
            };
            lists += 1;
            // The whole list (and everything after it in the section) must
            // parse, and nothing may have turned into a base address entry.
            let mut raw = dwarf.raw_locations(&unit, offset).unwrap();
            let mut n = 0;
            while let Some(entry) = raw.next().unwrap() {
                assert!(
                    !matches!(entry, RawLocListEntry::BaseAddress { .. }),
                    "tombstone written as base address selection: {entry:?}"
                );
                n += 1;
            }
            assert_eq!(n, 1);
        }
    }
    assert_eq!(lists, 2);
}
