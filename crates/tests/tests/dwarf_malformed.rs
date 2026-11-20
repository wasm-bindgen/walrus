//! DWARF that can't be read is dropped with a warning, instead of making
//! `emit_wasm` panic.

use std::borrow::Cow;
use wasm_encoder::{CodeSection, CustomSection, Function, FunctionSection, Module, TypeSection};
use wasmparser::{Parser, Payload};

fn module(custom: &[(&str, &[u8])]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut code = CodeSection::new();
    let mut f = Function::new([]);
    f.raw([0x0b]); // end
    code.function(&f);
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

fn emit(wasm: &[u8]) -> Vec<u8> {
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    config.parse(wasm).unwrap().emit_wasm()
}

fn debug_sections(wasm: &[u8]) -> Vec<String> {
    Parser::new(0)
        .parse_all(wasm)
        .filter_map(|p| match p.unwrap() {
            Payload::CustomSection(s) if s.name().starts_with(".debug") => {
                Some(s.name().to_string())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn truncated_unit_is_dropped() {
    // A unit whose length runs past the end of the section.
    let output = emit(&module(&[(".debug_info", &[0xff, 0xff, 0xff, 0x7f, 1, 2])]));
    wasmparser::validate(&output).unwrap();
    assert_eq!(debug_sections(&output), Vec::<String>::new());
}

#[test]
fn missing_abbreviations_are_dropped() {
    // A DWARF 4 unit header, then a DIE whose abbreviation code isn't in the
    // (missing) `.debug_abbrev`.
    let info = [8, 0, 0, 0, 4, 0, 0, 0, 0, 0, 4, 1];
    let output = emit(&module(&[(".debug_info", &info[..])]));
    wasmparser::validate(&output).unwrap();
    assert_eq!(debug_sections(&output), Vec::<String>::new());
}

#[test]
fn empty_defined_file_name_is_dropped() {
    // A compile unit with only a `DW_AT_stmt_list`.
    let abbrev = [1, 0x11, 0, 0x10, 0x17, 0, 0, 0];
    let info = [12, 0, 0, 0, 4, 0, 0, 0, 0, 0, 4, 1, 0, 0, 0, 0];
    // A DWARF 4 line program with a file "a", then a `DW_LNE_define_file`
    // whose name is empty, which a header's file list can't hold.
    #[rustfmt::skip]
    let line = [
        41, 0, 0, 0, // unit_length
        4, 0, // version
        25, 0, 0, 0, // header_length
        1, 1, 1, 0xfb, 14, 13, // instruction lengths, is_stmt, line base and range, opcode_base
        0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1, // standard_opcode_lengths
        0, // include_directories
        b'a', 0, 0, 0, 0, 0, // file_names
        0, 5, 3, 0, 0, 0, 0, // DW_LNE_define_file ""
        0, 1, 1, // DW_LNE_end_sequence
    ];
    let output = emit(&module(&[
        (".debug_abbrev", &abbrev[..]),
        (".debug_info", &info[..]),
        (".debug_line", &line[..]),
    ]));
    wasmparser::validate(&output).unwrap();
    assert_eq!(debug_sections(&output), Vec::<String>::new());
}

#[test]
fn advance_line_by_i64_min_is_dropped() {
    let abbrev = [1, 0x11, 0, 0x10, 0x17, 0, 0, 0];
    let info = [12, 0, 0, 0, 4, 0, 0, 0, 0, 0, 4, 1, 0, 0, 0, 0];
    // A DWARF 4 line program with a `DW_LNS_advance_line` of `i64::MIN`,
    // which gimli negates.
    #[rustfmt::skip]
    let line = [
        46, 0, 0, 0, // unit_length
        4, 0, // version
        25, 0, 0, 0, // header_length
        1, 1, 1, 0xfb, 14, 13, // instruction lengths, is_stmt, line base and range, opcode_base
        0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1, // standard_opcode_lengths
        0, // include_directories
        b'a', 0, 0, 0, 0, 0, // file_names
        3, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f, // DW_LNS_advance_line
        1, // DW_LNS_copy
        0, 1, 1, // DW_LNE_end_sequence
    ];
    let output = emit(&module(&[
        (".debug_abbrev", &abbrev[..]),
        (".debug_info", &info[..]),
        (".debug_line", &line[..]),
    ]));
    wasmparser::validate(&output).unwrap();
    assert_eq!(debug_sections(&output), Vec::<String>::new());
}
