//! `emit_wasm_with_source_map` builds a source map from the DWARF of the
//! emitted module. These tests write a module with a line table the way
//! rustc writes it, for a made-up crate `krate`. The map is compared as text
//! with `source_map/krate.txt`, which `WALRUS_BLESS=1` rewrites.

use gimli::constants::*;
use gimli::write::{
    Address, AttributeValue as Attr, DwarfUnit, EndianVec, LineProgram, LineString, Sections,
};
use gimli::{Encoding, Format, LineEncoding, LittleEndian};
use serde_json::{json, Value};
use std::fmt::Write;
use std::path::Path;
use walrus::{Module, ModuleConfig};
use wasmparser::{Parser, Payload};

/// The module's functions and their sizes in `i32.const 0; drop` pairs,
/// largest first, so that walrus keeps their order. All but `dead` are
/// exported, so gc removes it.
const FUNCTIONS: [(&str, usize); 6] = [
    ("run", 20),
    ("outer", 14),
    ("id_u32", 10),
    ("id_u64", 9),
    ("shim", 8),
    ("dead", 6),
];
const RUN: usize = 0;
const OUTER: usize = 1;
const ID_U32: usize = 2;
const ID_U64: usize = 3;
const SHIM: usize = 4;
const DEAD: usize = 5;

/// The sources: `src/lib.rs`, and `function.rs` of `core`.
const LIB: usize = 0;
const OPS: usize = 1;

/// A source, and a 1-based line and column.
type Position = (usize, u64, u64);

/// The line table rows: a function, the index of one of its instructions,
/// and the position from there on, or none for line 0.
const ROWS: &[(usize, usize, Option<Position>)] = &[
    (RUN, 0, Some((LIB, 41, 5))),
    (RUN, 4, Some((LIB, 21, 28))),
    (RUN, 6, Some((LIB, 11, 5))),
    (RUN, 8, Some((LIB, 21, 20))),
    (RUN, 10, Some((LIB, 11, 5))),
    (RUN, 12, Some((LIB, 43, 5))),
    (RUN, 16, Some((LIB, 21, 24))),
    (RUN, 20, None),
    (RUN, 22, Some((LIB, 44, 5))),
    (RUN, 24, Some((OPS, 250, 5))),
    (RUN, 26, Some((LIB, 45, 1))),
    (RUN, 38, None),
    (OUTER, 0, Some((LIB, 61, 5))),
    (OUTER, 2, Some((LIB, 71, 5))),
    (OUTER, 4, Some((LIB, 62, 9))),
    (OUTER, 6, Some((LIB, 71, 12))),
    (OUTER, 8, Some((LIB, 63, 5))),
    (ID_U32, 0, Some((LIB, 51, 5))),
    (ID_U32, 2, Some((LIB, 51, 9))),
    (ID_U64, 0, Some((LIB, 51, 5))),
    (ID_U64, 2, Some((LIB, 51, 12))),
    (SHIM, 0, Some((OPS, 250, 5))),
    (DEAD, 0, Some((LIB, 81, 5))),
    (DEAD, 2, Some((LIB, 11, 5))),
    (DEAD, 4, Some((LIB, 82, 1))),
];

/// The start of a module's code section, and each function body's start,
/// instructions and end, relative to it as DWARF addresses are.
struct Code {
    section: u64,
    bodies: Vec<(u64, Vec<u64>, u64)>,
}

impl Code {
    fn of(wasm: &[u8]) -> Code {
        let mut code = Code {
            section: 0,
            bodies: Vec::new(),
        };
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.unwrap() {
                Payload::CodeSectionStart { range, .. } => code.section = range.start as u64,
                Payload::CodeSectionEntry(body) => {
                    let at = |offset: usize| offset as u64 - code.section;
                    let mut ops = body.get_operators_reader().unwrap();
                    let mut instrs = Vec::new();
                    while !ops.eof() {
                        instrs.push(at(ops.read_with_offset().unwrap().1));
                    }
                    let range = body.range();
                    code.bodies.push((at(range.start), instrs, at(range.end)));
                }
                _ => {}
            }
        }
        code
    }

    /// A module offset as `f` and `f.end` for the start and end of function
    /// `f`'s body, and `f.i` for its instruction `i`.
    fn label(&self, offset: u64) -> String {
        let offset = offset - self.section;
        for ((start, instrs, end), (name, _)) in self.bodies.iter().zip(FUNCTIONS) {
            if offset == *start {
                return name.to_string();
            } else if offset == *end {
                return format!("{name}.end");
            } else if let Some(i) = instrs.iter().position(|&o| o == offset) {
                return format!("{name}.{i}");
            }
        }
        panic!("no instruction at {offset}")
    }
}

/// The DWARF of the module whose code is `code`.
fn dwarf(code: &Code) -> Sections<EndianVec<LittleEndian>> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    let string = |s: &str| LineString::String(s.into());
    let (work, lib_rs) = (string("/work"), string("src/lib.rs"));
    let mut program = LineProgram::new(encoding, LineEncoding::default(), work, None, lib_rs, None);
    // A directory relative to the compilation directory, an absolute one,
    // and a file that nothing maps to, which the map leaves out.
    let src = program.add_directory(string("src"));
    let ops = program.add_directory(string("/rustc/x/library/core/src/ops"));
    let files = [
        program.add_file(string("lib.rs"), src, None),
        program.add_file(string("function.rs"), ops, None),
    ];
    program.add_file(string("unused.rs"), src, None);
    // One sequence per function, ending at the end of its body.
    for (f, (start, instrs, end)) in code.bodies.iter().enumerate() {
        program.begin_sequence(Some(Address::Constant(*start)));
        for &(_, i, position) in ROWS.iter().filter(|row| row.0 == f) {
            let (file, line, column) = position.unwrap_or((LIB, 0, 0));
            let row = program.row();
            row.address_offset = instrs[i] - start;
            (row.file, row.line, row.column) = (files[file], line, column);
            program.generate_row();
        }
        program.end_sequence(end - start);
    }
    dwarf.unit.line_program = program;

    let root = dwarf.unit.root();
    let unit = dwarf.unit.get_mut(root);
    unit.set(DW_AT_name, Attr::String("src/lib.rs".into()));
    unit.set(DW_AT_comp_dir, Attr::String("/work".into()));
    unit.set(DW_AT_low_pc, Attr::Address(Address::Constant(0)));
    unit.set(DW_AT_stmt_list, Attr::LineProgramRef);

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    sections
}

/// The module, with its DWARF.
fn parse(config: impl FnOnce(&mut ModuleConfig) -> &mut ModuleConfig) -> Module {
    let funcs = FUNCTIONS.map(|(name, pairs)| {
        let export = (name != "dead").then(|| format!(" (export \"{name}\")"));
        let body = " i32.const 0 drop".repeat(pairs);
        format!("(func ${name}{}{body})", export.unwrap_or_default())
    });
    let mut wasm = wat::parse_str(format!("(module {})", funcs.join(" "))).unwrap();
    let sections = dwarf(&Code::of(&wasm));
    sections
        .for_each(|id, data| {
            if !data.slice().is_empty() {
                let (name, data) = (id.name().into(), data.slice().into());
                let section = wasm_encoder::CustomSection { name, data };
                wasm.push(wasm_encoder::SectionId::Custom as u8);
                wasm_encoder::Encode::encode(&section, &mut wasm);
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    config(&mut ModuleConfig::new()).parse(&wasm).unwrap()
}

fn emit(mut module: Module, gc: bool) -> (Vec<u8>, Value) {
    if gc {
        walrus::passes::gc::run(&mut module);
    }
    let (wasm, map) = module.emit_wasm_with_source_map("x.map");
    (wasm, serde_json::from_str(&map.unwrap()).unwrap())
}

#[test]
fn map_follows_the_dwarf() {
    let (wasm, map) = emit(parse(|c| c.generate_source_map(true)), true);
    let text = print(&Code::of(&wasm), &map);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/source_map/krate.txt");
    if std::env::var("WALRUS_BLESS").is_ok() {
        std::fs::write(&path, &text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(&path).unwrap());
    assert_eq!(map["version"], 3);
    assert_eq!(map["names"], json!([]));
}

#[test]
fn source_mapping_url_replaces_the_one_in_the_module() {
    let mut module = parse(|c| c.generate_source_map(true));
    module.customs.add(walrus::RawCustomSection {
        name: "sourceMappingURL".to_string(),
        data: b"\x07old.map".to_vec(),
    });
    let (wasm, _) = module.emit_wasm_with_source_map("new.map");
    let urls: Vec<_> = Parser::new(0)
        .parse_all(&wasm)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CustomSection(s) if s.name() == "sourceMappingURL" => Some(s.data().to_vec()),
            _ => None,
        })
        .collect();
    assert_eq!(urls, [b"\x07new.map"]);
}

#[test]
fn without_generate_source_map_there_is_no_map() {
    let (wasm, map) = parse(|c| c).emit_wasm_with_source_map("x.map");
    assert_eq!((wasm, map), (parse(|c| c).emit_wasm(), None));
}

#[test]
fn code_is_the_same_with_a_source_map() {
    let plain = parse(|c| c).emit_wasm();
    assert_eq!(parse(|c| c.generate_source_map(true)).emit_wasm(), plain);
    // Only the sourceMappingURL section is new, also next to DWARF, and the
    // map is the same with DWARF.
    const URL_SECTION: &[u8] = b"\0\x17\x10sourceMappingURL\x05x.map";
    let (wasm, map) = emit(parse(|c| c.generate_source_map(true)), false);
    assert_eq!(wasm, [&plain[..], URL_SECTION].concat());
    let dwarf = parse(|c| c.generate_dwarf(true)).emit_wasm();
    let both = emit(
        parse(|c| c.generate_dwarf(true).generate_source_map(true)),
        false,
    );
    assert_eq!(both, ([&dwarf[..], URL_SECTION].concat(), map));
}

/// The map as text: its sources and its mappings. Offsets are `Code::label`s,
/// and positions are 1-based, like DWARF's.
fn print(code: &Code, map: &Value) -> String {
    let sources = map["sources"].as_array().unwrap();
    let file = |source: u64| {
        let path = sources[source as usize].as_str().unwrap();
        path.rsplit('/').next().unwrap()
    };
    let pos = |line: u64, column: u64| format!("{}:{}", line + 1, column + 1);
    let at = |source, line, column| format!("{}:{}", file(source), pos(line, column));
    let mut out = format!("sources {}\nmappings\n", map["sources"]);
    let mut fields = [0i64; 4];
    for item in items(map["mappings"].as_str().unwrap()) {
        for (field, &value) in fields.iter_mut().zip(&item) {
            *field += signed(value);
        }
        let [offset, source, line, column] = fields.map(|f| f as u64);
        let location = (item.len() == 4).then(|| at(source, line, column));
        let location = location.unwrap_or_else(|| "-".into());
        writeln!(out, "  {} {location}", code.label(offset)).unwrap();
    }
    out
}

/// The unsigned VLQs of each comma-separated item of `encoded`.
fn items(encoded: &str) -> Vec<Vec<u64>> {
    const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    for item in encoded.split(',').filter(|i| !i.is_empty()) {
        let (mut values, mut value, mut shift) = (Vec::new(), 0, 0);
        for c in item.bytes() {
            let digit = BASE64.iter().position(|&b| b == c).unwrap() as u64;
            value |= (digit & 31) << shift;
            shift += 5;
            if digit & 32 == 0 {
                values.push(value);
                (value, shift) = (0, 0);
            }
        }
        out.push(values);
    }
    out
}

fn signed(value: u64) -> i64 {
    (value >> 1) as i64 * if value & 1 == 1 { -1 } else { 1 }
}
