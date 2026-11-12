//! Line table rows follow the instructions they describe, including
//! instructions that end blocks, and nothing else.

use gimli::write::{
    Address, AttributeValue, Dwarf, EndianVec, LineProgram, LineString, Sections, Unit,
};
use gimli::{constants, read, Encoding, EndianSlice, Format, LineEncoding, LittleEndian};
use std::borrow::Cow;
use wasm_encoder::CustomSection;
use wasmparser::{Parser, Payload};

/// Code-section-relative addresses of the first function's body start, each
/// of its instructions, and its end.
struct Body {
    start: u64,
    instrs: Vec<u64>,
    end: u64,
}

fn body(wasm: &[u8]) -> Body {
    let mut code_start = 0;
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
                return Body {
                    start: rel(body.range().start),
                    instrs,
                    end: rel(body.range().end),
                };
            }
            _ => {}
        }
    }
    unreachable!()
}

/// A unit's line sequence: its first row's address, (address, line) rows,
/// and the end.
type Sequence = (u64, Vec<(u64, u64)>, u64);

/// Adds a unit for each sequence, parses with DWARF, emits, and returns the
/// input and output bodies and each output unit's sequences.
fn run(wat: &str, sequences: impl Fn(&Body) -> Vec<Sequence>) -> (Body, Body, Vec<Vec<Sequence>>) {
    let mut wasm = wat::parse_str(wat).unwrap();
    let input = body(&wasm);
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size: 4,
    };
    let mut dwarf = Dwarf::new();
    for (start, rows, end) in sequences(&input) {
        let source = LineString::String(b"lines.c".to_vec());
        let mut program = LineProgram::new(
            encoding,
            LineEncoding::default(),
            LineString::String(b"/src".to_vec()),
            None,
            source.clone(),
            None,
        );
        let file = program.add_file(source, program.default_directory(), None);
        program.begin_sequence(Some(Address::Constant(start)));
        for (address, line) in rows {
            let row = program.row();
            row.file = file;
            row.line = line;
            row.address_offset = address - start;
            program.generate_row();
        }
        program.end_sequence(end - start);
        let id = dwarf.units.add(Unit::new(encoding, program));
        let unit = dwarf.units.get_mut(id);
        let root = unit.root();
        unit.get_mut(root).set(
            constants::DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(0)),
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
    config.generate_dwarf(true).legacy_exceptions(true);
    let output = config.parse(&wasm).unwrap().emit_wasm();

    let sections: Vec<(String, Vec<u8>)> = Parser::new(0)
        .parse_all(&output)
        .filter_map(|p| match p.unwrap() {
            Payload::CustomSection(s) => Some((s.name().to_string(), s.data().to_vec())),
            _ => None,
        })
        .collect();
    let read = read::Dwarf::load(|id| {
        let data = sections
            .iter()
            .find(|(name, _)| name == id.name())
            .map_or(&[][..], |(_, data)| data.as_slice());
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })
    .unwrap();
    let mut units = Vec::new();
    let mut headers = read.units();
    while let Some(header) = headers.next().unwrap() {
        let unit = read.unit(header).unwrap();
        let mut sequences = Vec::new();
        let mut rows = unit.line_program.clone().unwrap().rows();
        let mut current: Option<(u64, Vec<(u64, u64)>)> = None;
        while let Some((_, row)) = rows.next_row().unwrap() {
            if row.end_sequence() {
                let (start, rows) = current.take().unwrap();
                sequences.push((start, rows, row.address()));
            } else {
                let current = current.get_or_insert_with(|| (row.address(), Vec::new()));
                current.1.push((row.address(), row.line().unwrap().get()));
            }
        }
        units.push(sequences);
    }
    (input, body(&output), units)
}

/// One sequence over the whole function, with a row on every instruction.
fn every_instruction(body: &Body) -> Vec<Sequence> {
    let rows = body
        .instrs
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, 10 + i as u64))
        .collect();
    vec![(body.start, rows, body.end)]
}

#[test]
fn unreachable_code() {
    let (input, out, units) = run(
        r#"(module
          (func (export "f") (result i32)
            i32.const 1
            return
            i32.const 2
            drop
            i32.const 3))"#,
        every_instruction,
    );
    // walrus drops the code after `return`.
    assert_eq!((input.instrs.len(), out.instrs.len()), (6, 3));
    let rows = vec![
        (out.instrs[0], 10),
        (out.instrs[1], 11),
        (out.instrs[2], 15),
    ];
    assert_eq!(units, [[(out.instrs[0], rows, out.end)]]);
}

#[test]
fn delegate() {
    let (input, out, units) = run(
        r#"(module
          (func (export "f") (result i32)
            try (result i32)
              i32.const 1
            delegate 0
            i32.const 2
            i32.add))"#,
        every_instruction,
    );
    assert_eq!(out.instrs.len(), input.instrs.len());
    let rows = out
        .instrs
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, 10 + i as u64))
        .collect();
    assert_eq!(units, [[(out.instrs[0], rows, out.end)]]);
}

#[test]
fn overlapping_sequences() {
    // A second unit's sequence covers only the first instruction, as when a
    // linker leaves a discarded function's rows at the address of another.
    let (_, out, units) = run(
        r#"(module
          (func (export "f") (result i32)
            i32.const 1
            i32.const 2
            i32.add))"#,
        |body| {
            let mut sequences = every_instruction(body);
            sequences.push((body.instrs[0], vec![(body.instrs[0], 99)], body.instrs[1]));
            sequences
        },
    );
    let rows = out
        .instrs
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, 10 + i as u64))
        .collect();
    assert_eq!(units[0], [(out.instrs[0], rows, out.end)]);
    // It still ends after the first instruction.
    assert_eq!(
        units[1],
        [(out.instrs[0], vec![(out.instrs[0], 99)], out.instrs[1])]
    );
}
