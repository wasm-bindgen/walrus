//! walrus rewrites the code addresses in DWARF to match the code it emits.
//! These tests check them against the emitted code.

use gimli::write::{Address, AttributeValue, DwarfUnit, EndianVec, LineProgram, Sections};
use gimli::{constants, read, Encoding, EndianSlice, Format, LittleEndian};
use std::borrow::Cow;
use std::collections::HashMap;
use wasm_encoder::{CodeSection, CustomSection, Function, FunctionSection, Module, TypeSection};
use wasmparser::{Parser, Payload};

/// A module whose first function has `locals` locals and the instructions
/// `body`, followed by `extra_functions` empty functions and the `custom`
/// sections.
fn module(locals: u32, body: &[u8], extra_functions: u32, custom: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut first = Function::new((locals > 0).then_some((locals, wasm_encoder::ValType::I32)));
    first.raw(body.iter().copied());
    code.function(&first);
    functions.function(0);
    for _ in 0..extra_functions {
        let mut empty = Function::new([]);
        empty.raw([0x0b]); // end
        code.function(&empty);
        functions.function(0);
    }
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

/// The code-section-relative start and the size of the first function body.
fn first_body(wasm: &[u8]) -> (u64, usize) {
    let mut code_start = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.unwrap() {
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                return ((body.range().start - code_start) as u64, body.range().len())
            }
            _ => {}
        }
    }
    panic!("no function body");
}

/// DWARF sections with one subprogram whose `low_pc` is `low_pc`.
fn subprogram_at(low_pc: u64) -> Vec<(&'static str, Vec<u8>)> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::none();
    let root = dwarf.unit.root();
    let subprogram = dwarf.unit.add(root, constants::DW_TAG_subprogram);
    dwarf.unit.get_mut(subprogram).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(low_pc)),
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

fn subprogram_low_pc(wasm: &[u8]) -> u64 {
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
    while let Some(entry) = entries.next_dfs().unwrap() {
        if entry.tag() == constants::DW_TAG_subprogram {
            if let Some(read::AttributeValue::Addr(pc)) = entry.attr_value(constants::DW_AT_low_pc)
            {
                return pc;
            }
        }
    }
    panic!("no subprogram with a low_pc");
}

/// Round-trips a module whose first function has `locals` locals and `body`
/// through walrus, with a subprogram at that function's start, after
/// `transform`, and checks that the subprogram still starts there.
fn check_function_start_after(
    locals: u32,
    body: &[u8],
    extra_functions: u32,
    transform: impl FnOnce(&mut walrus::Module),
) -> (usize, usize) {
    let (start, size) = first_body(&module(locals, body, extra_functions, &[]));
    let wasm = module(locals, body, extra_functions, &subprogram_at(start));
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut walrus_module = config.parse(&wasm).unwrap();
    transform(&mut walrus_module);
    let emitted = walrus_module.emit_wasm();
    let (new_start, new_size) = first_body(&emitted);
    assert_eq!(subprogram_low_pc(&emitted), new_start);
    (size, new_size)
}

fn check_function_start(body: &[u8], extra_functions: u32) -> (usize, usize) {
    check_function_start_after(1, body, extra_functions, |_| {})
}

#[test]
fn addresses_follow_a_function_count_of_any_length() {
    // `i32.const 0; drop; end`, which walrus emits unchanged.
    let body = [0x41, 0x00, 0x1a, 0x0b];
    // The function count takes one, two and three bytes.
    for extra_functions in [0, 199, 16_999] {
        check_function_start(&body, extra_functions);
    }
}

#[test]
fn function_start_follows_a_body_size_that_gets_shorter() {
    // `i32.const 0` with its immediate padded to five bytes, as linkers write
    // relocatable ones, then `drop`; walrus writes the immediate in one byte.
    let mut body = [0x41, 0x80, 0x80, 0x80, 0x80, 0x00, 0x1a].repeat(20);
    body.push(0x0b); // end
    for extra_functions in [0, 199, 16_999] {
        let (size, new_size) = check_function_start(&body, extra_functions);
        // The body's size took two bytes and now takes one.
        assert!(size >= 128 && new_size < 128, "{size} -> {new_size}");
    }
}

#[test]
fn function_start_follows_locals_added_to_a_function_without_any() {
    // `i32.const 0; drop; end`, in a function without locals, whose start is
    // then the byte before its first instruction.
    let body = [0x41, 0x00, 0x1a, 0x0b];
    check_function_start_after(0, &body, 0, |module| {
        let local = module.locals.add(walrus::ValType::I32);
        let (id, _) = module.funcs.iter_local().next().unwrap();
        let function = module.funcs.get_mut(id).kind.unwrap_local_mut();
        function
            .builder_mut()
            .func_body()
            .i32_const(1)
            .local_set(local);
    });
}
