//! `emit_wasm_with_source_map` builds a source map, with Scopes, from the
//! DWARF of the emitted module. These tests write a module with DWARF the
//! way rustc writes it, for a made-up crate `krate`. The map is compared as
//! text with `source_map/krate.txt`, which `WALRUS_BLESS=1` rewrites.

use gimli::constants::*;
use gimli::write::{
    Address, AttributeValue as Attr, DwarfUnit, EndianVec, FileId, LineProgram, LineString, Range,
    RangeList, Sections, UnitEntryId,
};
use gimli::{DwAt, DwTag, Encoding, Format, LineEncoding, LittleEndian};
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

/// Writes the debugging information entries of one unit.
struct Entries<'a> {
    dwarf: &'a mut DwarfUnit,
    code: &'a Code,
    files: [FileId; 2],
    /// The source of the functions, whose namespace is `krate` in `lib.rs`.
    file: usize,
    krate: UnitEntryId,
}

impl Entries<'_> {
    fn add(&mut self, parent: UnitEntryId, tag: DwTag, attrs: Vec<(DwAt, Attr)>) -> UnitEntryId {
        let id = self.dwarf.unit.add(parent, tag);
        for (name, value) in attrs {
            self.dwarf.unit.get_mut(id).set(name, value);
        }
        id
    }

    /// A function declared on `line`, with the body of module function `f`,
    /// or only inlined. A linkage name is v0, legacy, or empty for none.
    fn function(
        &mut self,
        f: Option<usize>,
        line: u64,
        name: &str,
        linkage_name: &str,
    ) -> UnitEntryId {
        let mut attrs = vec![
            (DW_AT_name, Attr::String(name.into())),
            (
                DW_AT_decl_file,
                Attr::FileIndex(Some(self.files[self.file])),
            ),
            (DW_AT_decl_line, Attr::Udata(line)),
        ];
        if !linkage_name.is_empty() {
            attrs.push((DW_AT_linkage_name, Attr::String(linkage_name.into())));
        }
        if let Some(f) = f {
            let &(start, _, end) = &self.code.bodies[f];
            attrs.push((DW_AT_low_pc, Attr::Address(Address::Constant(start))));
            attrs.push((DW_AT_high_pc, Attr::Udata(end - start)));
        } else {
            attrs.push((DW_AT_inline, Attr::Inline(DW_INL_inlined)));
        }
        let parent = match self.file {
            LIB => self.krate,
            _ => self.dwarf.unit.root(),
        };
        self.add(parent, DW_TAG_subprogram, attrs)
    }

    /// A call of `origin`, inlined into module function `f` from instruction
    /// to instruction in each of `pieces`: with `DW_AT_ranges` for more than
    /// one.
    fn inlined(
        &mut self,
        parent: UnitEntryId,
        origin: UnitEntryId,
        (file, line, column): Position,
        f: usize,
        pieces: &[(usize, usize)],
    ) -> UnitEntryId {
        let code = self.code;
        let at = |i: usize| code.bodies[f].1[i];
        let mut attrs = vec![
            (DW_AT_abstract_origin, Attr::UnitRef(origin)),
            (DW_AT_call_file, Attr::FileIndex(Some(self.files[file]))),
            (DW_AT_call_line, Attr::Udata(line)),
            (DW_AT_call_column, Attr::Udata(column)),
        ];
        if let [(start, end)] = *pieces {
            attrs.push((DW_AT_low_pc, Attr::Address(Address::Constant(at(start)))));
            attrs.push((DW_AT_high_pc, Attr::Udata(at(end) - at(start))));
        } else {
            let list = pieces.iter().map(|&(start, end)| Range::StartEnd {
                begin: Address::Constant(at(start)),
                end: Address::Constant(at(end)),
            });
            let list = self.dwarf.unit.ranges.add(RangeList(list.collect()));
            attrs.push((DW_AT_ranges, Attr::RangeListRef(list)));
        }
        self.add(parent, DW_TAG_inlined_subroutine, attrs)
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
    let krate = dwarf.unit.add(root, DW_TAG_namespace);
    let name = Attr::String("krate".into());
    dwarf.unit.get_mut(krate).set(DW_AT_name, name);
    let mut e = Entries {
        dwarf: &mut dwarf,
        code,
        files,
        file: OPS,
        krate,
    };

    // `core`'s `call_once` of a closure, which is only inlined, and the
    // vtable shim of another closure's.
    let call_once = e.function(
        None,
        250,
        "call_once<krate::once::{closure_env#0}, ()>",
        "_RNvYNCNvCsjC8j7CnY1Nh_5krate4once0INtNtNtCs8hZryZq3oeI_4core3ops8function6FnOnceuE9call_onceB6_",
    );
    e.function(
        Some(SHIM),
        250,
        "call_once<krate::ONCE::{closure_env#0}, ()>",
        "_RNSNvYNCNvCsjC8j7CnY1Nh_5krate4ONCE0INtNtNtCs8hZryZq3oeI_4core3ops8function6FnOnceuE9call_once6vtableB8_",
    );
    e.file = LIB;

    // Functions that are only inlined.
    let square = e.function(None, 10, "square", "_RNvCsjC8j7CnY1Nh_5krate6square");
    let sum = e.function(None, 20, "sum", "_RNvCsjC8j7CnY1Nh_5krate3sum");
    let apply = e.function(
        None,
        70,
        "apply<krate::outer::{closure_env#0}>",
        "_RINvCsjC8j7CnY1Nh_5krate5applyNCNvB2_5outer0EB2_",
    );
    let closure = e.function(
        None,
        61,
        "{closure#0}",
        "_RNCNvCsjC8j7CnY1Nh_5krate5outer0B3_",
    );

    // A legacy linkage name. `sum` is inlined in two pieces, and inlines
    // `square` twice.
    let run = e.function(Some(RUN), 40, "run", "_ZN5krate3run17h0123456789abcdefE");
    let call = e.inlined(run, sum, (LIB, 42, 9), RUN, &[(4, 12), (16, 20)]);
    e.inlined(call, square, (LIB, 21, 5), RUN, &[(6, 8)]);
    e.inlined(call, square, (LIB, 21, 17), RUN, &[(10, 12)]);
    e.inlined(run, call_once, (LIB, 44, 9), RUN, &[(24, 26)]);

    // `#[no_mangle]`, so only its name in the namespace. It inlines `apply`
    // and the closure, which has no row on its declaration line, 61.
    let outer = e.function(Some(OUTER), 60, "outer", "");
    let call = e.inlined(outer, apply, (LIB, 61, 13), OUTER, &[(2, 8)]);
    e.inlined(call, closure, (LIB, 71, 5), OUTER, &[(4, 6)]);

    // Two copies of a generic function, whose v0 names encode `u32` as `m`
    // and `u64` as `y`.
    for (f, ty, v0) in [(ID_U32, "u32", 'm'), (ID_U64, "u64", 'y')] {
        let linkage_name = format!("_RINvCsjC8j7CnY1Nh_5krate2id{v0}EB2_");
        e.function(Some(f), 50, &format!("id<{ty}>"), &linkage_name);
    }

    let dead = e.function(Some(DEAD), 80, "dead", "_RNvCsjC8j7CnY1Nh_5krate4dead");
    e.inlined(dead, square, (LIB, 81, 5), DEAD, &[(2, 4)]);

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
    // `dead` has a scope until gc removes it.
    let (_, all) = emit(parse(|c| c.generate_source_map(true)), false);
    assert!(all["names"].to_string().contains("\"krate::dead\""));
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

#[test]
fn a_map_without_scopes_has_the_same_mappings() {
    let (wasm, map) = emit(parse(|c| c.generate_source_map(true)), true);
    let (plain_wasm, plain) = emit(
        parse(|c| c.generate_source_map(true).source_map_scopes(false)),
        true,
    );
    assert_eq!(plain_wasm, wasm);
    let expected =
        json!({"version": 3, "sources": map["sources"], "names": [], "mappings": map["mappings"]});
    assert_eq!(plain, expected);
}

/// The map as text: its sources, its mappings, each source's scopes, and
/// each segment of ranges. Nesting is indented, offsets are `Code::label`s,
/// and positions are 1-based, like DWARF's.
fn print(code: &Code, map: &Value) -> String {
    let sources = map["sources"].as_array().unwrap();
    let name = |i: i64| map["names"][i as usize].as_str().unwrap();
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

    // Each source's scope names, in pre-order, for the ranges.
    let mut defined: Vec<Vec<&str>> = Vec::new();
    for (source, scopes) in map["scopes"].as_array().unwrap().iter().enumerate() {
        writeln!(out, "scopes {}", file(source as u64)).unwrap();
        let (mut lines, mut open, mut position) = (Vec::new(), Vec::new(), (0, 0));
        let (mut name_index, mut kind) = (0, 0);
        defined.push(Vec::new());
        for item in items(scopes.as_str().unwrap()) {
            // After B's flags, or C's tag: a line delta, then a column,
            // relative on the same line.
            let i = if item[0] == 1 { 2 } else { 1 };
            let (line, column) = (item[i], item[i + 1]);
            let base = if line == 0 { position.1 } else { 0 };
            position = (position.0 + line, base + column);
            let here = pos(position.0, position.1);
            match item[..] {
                // B: flags, line, column, name (flag 1), kind; frame: flag 4.
                [1, flags, _, _, ref rest @ ..] => {
                    let mut rest = rest.iter().map(|&v| signed(v));
                    let scope = if flags & 1 != 0 {
                        name_index += rest.next().unwrap();
                        name(name_index)
                    } else {
                        ""
                    };
                    kind += rest.next().unwrap();
                    let frame = if flags & 4 != 0 { " frame" } else { "" };
                    let indent = "  ".repeat(open.len() + 1);
                    lines.push(format!("{indent}{here}-"));
                    open.push((lines.len() - 1, format!("{} {scope}{frame}", name(kind))));
                    defined[source].push(scope);
                }
                // C: line, column.
                [2, _, _] => {
                    let (line, tail) = open.pop().unwrap();
                    lines[line] += &format!("{here} {tail}");
                }
                _ => panic!("unexpected scope item {item:?}"),
            }
        }
        out.extend(lines.iter().map(|l| format!("{}\n", l.trim_end())));
    }

    out.push_str("ranges\n");
    for segment in map["ranges"].as_array().unwrap() {
        let (mut column, mut source, mut scope, mut open) = (0, 0, 0, Vec::new());
        let mut lines: Vec<String> = Vec::new();
        for item in items(segment.as_str().unwrap()) {
            match item[..] {
                // E: flags, column, definition (flag 2); frame: flag 4.
                [4, flags, delta, ref definition @ ..] => {
                    assert_eq!(flags & 1, 0, "Wasm has one line");
                    column += delta;
                    let mut name = "";
                    if let [source_delta, index] = *definition {
                        source += signed(source_delta);
                        scope = if source_delta == 0 {
                            scope + signed(index)
                        } else {
                            index as i64
                        };
                        name = defined[source as usize][scope as usize];
                    }
                    let frame = if flags & 4 != 0 { " frame" } else { "" };
                    let indent = "  ".repeat(open.len() + 1);
                    lines.push(format!("{indent}{}-", code.label(column)));
                    open.push((lines.len() - 1, format!("{name}{frame}")));
                }
                // I: the call site's source, line and column.
                [8, source, line, column] => {
                    open.last_mut().unwrap().1 += &format!(" @ {}", at(source, line, column));
                }
                // F: column.
                [5, delta] => {
                    column += delta;
                    let (line, tail) = open.pop().unwrap();
                    lines[line] += &format!("{} {tail}", code.label(column));
                }
                _ => panic!("unexpected range item {item:?}"),
            }
        }
        out.extend(lines.iter().map(|l| format!("{l}\n")));
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
