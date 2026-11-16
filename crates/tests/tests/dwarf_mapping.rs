//! How walrus maps DWARF onto the code it emits, in the cases that once went
//! wrong: code walrus removes, code and locals in several functions, line
//! tables and addresses of static variables. Each test writes DWARF by
//! hand and checks that the output tells a debugger the same as the input.

use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, LineProgram, LineString, Location,
    LocationList, Range, RangeList, Sections,
};
use gimli::{constants, Encoding, Format, LineEncoding, LittleEndian};
use std::borrow::Cow;
use walrus_tests_utils::dwarf;
use wasmparser::{Parser, Payload};

/// Code-section-relative addresses of each function's body and instructions.
struct Body {
    start: u64,
    end: u64,
    instrs: Vec<u64>,
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
                    end: rel(body.range().end),
                    instrs,
                });
            }
            _ => {}
        }
    }
    out
}

fn unit(version: u16) -> DwarfUnit {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version,
        address_size: 4,
    };
    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(b"/".to_vec()),
        None,
        LineString::String(b"test.c".to_vec()),
        None,
    );
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(
        constants::DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(0)),
    );
    dwarf
}

/// `wasm` with `dwarf`'s sections.
fn with_dwarf(wasm: &[u8], mut dwarf: DwarfUnit) -> Vec<u8> {
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).unwrap();
    let mut wasm = wasm.to_vec();
    sections
        .for_each(|id, data| {
            if !data.slice().is_empty() {
                wasm.push(0);
                wasm_encoder::Encode::encode(
                    &wasm_encoder::CustomSection {
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

fn walrus(input: &[u8], gc: bool) -> Vec<u8> {
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(input).unwrap();
    if gc {
        walrus::passes::gc::run(&mut module);
    }
    module.emit_wasm()
}

fn local(index: u32) -> Expression {
    let mut expr = Expression::new();
    expr.op_wasm_local(index);
    expr.op(constants::DW_OP_stack_value);
    expr
}

/// A location list entry that starts in unreachable code, which walrus
/// removes, covers the instructions after it.
#[test]
fn location_entry_starting_in_removed_code_covers_what_follows() {
    let wasm = wat::parse_str(
        r#"(module
          (func (export "f") (param i32) (result i32)
            block
              br 0
              i32.const 1
              drop
            end
            local.get 0))"#,
    )
    .unwrap();
    let body = &bodies(&wasm)[0];
    for version in [4, 5] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let var = dwarf.unit.add(root, constants::DW_TAG_variable);
        // From the unreachable `i32.const 1` to the end.
        let list = dwarf
            .unit
            .locations
            .add(LocationList(vec![Location::StartEnd {
                begin: Address::Constant(body.instrs[2]),
                end: Address::Constant(body.end),
                data: local(0),
            }]));
        dwarf.unit.get_mut(var).set(
            constants::DW_AT_location,
            AttributeValue::LocationListRef(list),
        );
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        // `end`, `local.get 0` and `end`.
        assert_eq!(stats.locations, 3, "DWARF {version}");
    }
}

/// A line table row for a `nop`, which walrus removes, applies to the next
/// instruction, as it did when that had no row of its own.
#[test]
fn row_for_removed_nop_applies_to_the_next_instruction() {
    let wasm = wat::parse_str(r#"(module (func (export "f") nop i32.const 1 drop))"#).unwrap();
    let body = &bodies(&wasm)[0];
    let mut dwarf = unit(5);
    let file = dwarf.unit.line_program.add_file(
        LineString::String(b"test.c".to_vec()),
        dwarf.unit.line_program.default_directory(),
        None,
    );
    let program = &mut dwarf.unit.line_program;
    program.begin_sequence(Some(Address::Constant(body.start)));
    for (i, line) in [(0, 10), (2, 11)] {
        let row = program.row();
        row.address_offset = body.instrs[i] - body.start;
        row.line = line;
        row.file = file;
        program.generate_row();
    }
    program.end_sequence(body.end - body.start);
    let input = with_dwarf(&wasm, dwarf);
    let output = walrus(&input, false);
    dwarf::check_readable(&output).unwrap();
    let stats = dwarf::check_preserved(&input, &output).unwrap();
    assert_eq!(stats.with_line, stats.instructions);
}

/// An expression's local indices belong to the function its DIE is in,
/// which a subprogram can give as `DW_AT_ranges` rather than
/// `DW_AT_low_pc`.
#[test]
fn locals_of_a_subprogram_with_ranges_are_remapped() {
    // walrus drops the unused `$unused`, so `$x` goes from index 1 to 0.
    let wasm = wat::parse_str(
        r#"(module
          (func (export "f") (result i32)
            (local $unused i64) (local $x i32)
            i32.const 7
            local.set $x
            local.get $x))"#,
    )
    .unwrap();
    let body = &bodies(&wasm)[0];
    for version in [4, 5] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let sub = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        let ranges = dwarf.unit.ranges.add(RangeList(vec![Range::StartEnd {
            begin: Address::Constant(body.start),
            end: Address::Constant(body.end),
        }]));
        let mut frame_base = Expression::new();
        frame_base.op_wasm_local(1);
        let entry = dwarf.unit.get_mut(sub);
        entry.set(
            constants::DW_AT_ranges,
            AttributeValue::RangeListRef(ranges),
        );
        entry.set(
            constants::DW_AT_frame_base,
            AttributeValue::Exprloc(frame_base),
        );
        let var = dwarf.unit.add(sub, constants::DW_TAG_variable);
        dwarf
            .unit
            .get_mut(var)
            .set(constants::DW_AT_location, AttributeValue::Exprloc(local(1)));
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        // The frame base and the variable, at each of the 4 instructions.
        assert_eq!(stats.locations, 8, "DWARF {version}");
    }
}

/// `DW_OP_addr` and `DW_OP_addrx` name linear memory, such as where a
/// static variable is, not code, so walrus keeps them.
#[test]
fn static_variable_addresses_are_kept() {
    let wasm =
        wat::parse_str(r#"(module (memory 1) (func (export "f") i32.const 0 drop))"#).unwrap();
    for version in [4, 5] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let var = dwarf.unit.add(root, constants::DW_TAG_variable);
        let mut expr = Expression::new();
        // An address that's also a code address, which mustn't move.
        expr.op_addr(Address::Constant(bodies(&wasm)[0].instrs[1]));
        dwarf
            .unit
            .get_mut(var)
            .set(constants::DW_AT_location, AttributeValue::Exprloc(expr));
        let var = dwarf.unit.add(root, constants::DW_TAG_variable);
        let mut expr = Expression::new();
        expr.op_addr(Address::Constant(0x1_0400));
        dwarf
            .unit
            .get_mut(var)
            .set(constants::DW_AT_location, AttributeValue::Exprloc(expr));
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        assert_eq!(stats.locations, 2 * stats.instructions, "DWARF {version}");
    }
}

/// A line table row at the end of a function is for no instruction, and is
/// dropped rather than ending the sequence inside the next function.
#[test]
fn row_at_function_end_is_dropped() {
    let wasm = wat::parse_str(
        r#"(module
          (func (export "a") i32.const 1 drop)
          (func (export "b") i32.const 2 drop))"#,
    )
    .unwrap();
    let a = &bodies(&wasm)[0];
    let mut dwarf = unit(5);
    let file = dwarf.unit.line_program.add_file(
        LineString::String(b"test.c".to_vec()),
        dwarf.unit.line_program.default_directory(),
        None,
    );
    let program = &mut dwarf.unit.line_program;
    program.begin_sequence(Some(Address::Constant(a.start)));
    for (address, line) in [(a.instrs[0], 10), (a.end, 11)] {
        let row = program.row();
        row.address_offset = address - a.start;
        row.line = line;
        row.file = file;
        program.generate_row();
    }
    program.end_sequence(a.end + 1 - a.start);
    let input = with_dwarf(&wasm, dwarf);
    let output = walrus(&input, false);
    dwarf::check_readable(&output).unwrap();
    dwarf::check_preserved(&input, &output).unwrap();
}

/// A `DW_AT_low_pc` and `DW_AT_high_pc` whose code spans two functions,
/// which walrus may reorder, become `DW_AT_ranges` with a range in each.
#[test]
fn low_high_spanning_functions_becomes_ranges() {
    let wasm = wat::parse_str(
        r#"(module
          (func (export "a") i32.const 1 drop)
          (func (export "b") i32.const 2 drop))"#,
    )
    .unwrap();
    let (a, b) = (&bodies(&wasm)[0], &bodies(&wasm)[1]);
    for (version, size) in [(4, true), (5, true), (5, false)] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let block = dwarf.unit.add(root, constants::DW_TAG_lexical_block);
        let entry = dwarf.unit.get_mut(block);
        entry.set(
            constants::DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(a.instrs[1])),
        );
        let high = b.instrs[1];
        entry.set(
            constants::DW_AT_high_pc,
            match size {
                true => AttributeValue::Udata(high - a.instrs[1]),
                false => AttributeValue::Address(Address::Constant(high)),
            },
        );
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        // `drop` and `end` of `a`, and `i32.const 2` of `b`.
        assert_eq!(stats.in_scope, 3, "DWARF {version}, size: {size}");
    }
}

/// A line table for a VLIW target, with several operations per address,
/// is rewritten for wasm's one, so rows moved to the same address can't
/// have their operation indices go backwards.
#[test]
fn rows_with_operation_indices_moved_together() {
    let wasm = wat::parse_str(r#"(module (func (export "f") nop i32.const 1 drop))"#).unwrap();
    let body = &bodies(&wasm)[0];
    let mut dwarf = unit(4);
    let line_encoding = LineEncoding {
        maximum_operations_per_instruction: 4,
        ..LineEncoding::default()
    };
    dwarf.unit.line_program = LineProgram::new(
        dwarf.unit.encoding(),
        line_encoding,
        LineString::String(b"/".to_vec()),
        None,
        LineString::String(b"test.c".to_vec()),
        None,
    );
    let file = dwarf.unit.line_program.add_file(
        LineString::String(b"test.c".to_vec()),
        dwarf.unit.line_program.default_directory(),
        None,
    );
    let program = &mut dwarf.unit.line_program;
    program.begin_sequence(Some(Address::Constant(body.start)));
    // The `nop`'s last operation, then the `i32.const`'s first, which are
    // at the same address once the `nop` is gone.
    for (i, op_index, line) in [(0, 3, 10), (1, 0, 11)] {
        let row = program.row();
        row.address_offset = body.instrs[i] - body.start;
        row.op_index = op_index;
        row.line = line;
        row.file = file;
        program.generate_row();
    }
    program.end_sequence(body.end - body.start);
    let input = with_dwarf(&wasm, dwarf);
    let output = walrus(&input, false);
    dwarf::check_readable(&output).unwrap();
    dwarf::check_preserved(&input, &output).unwrap();
}

/// Code that spans functions, which a DIE can give as one range, uses each
/// function's local indices. An expression for a local walrus renumbers
/// differently in each becomes a location list with each function's index,
/// as do a location list's entries. A default location entry can't be
/// split, so it's dropped.
#[test]
fn locals_over_functions_numbered_differently() {
    // `a` drops its unused `i64` local, so its local 1 becomes 0; `b` keeps
    // its local 1.
    let wasm = wat::parse_str(
        r#"(module
          (func (export "a") (result i32) (local i64 i32)
            i32.const 7 local.set 1 local.get 1)
          (func (export "b") (param i32) (result i32) (local i32)
            i32.const 8 local.set 1 local.get 1))"#,
    )
    .unwrap();
    let (a, b) = (&bodies(&wasm)[0], &bodies(&wasm)[1]);
    for version in [4, 5] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let block = dwarf.unit.add(root, constants::DW_TAG_lexical_block);
        let ranges = dwarf.unit.ranges.add(RangeList(vec![Range::StartEnd {
            begin: Address::Constant(a.start),
            end: Address::Constant(b.end),
        }]));
        dwarf.unit.get_mut(block).set(
            constants::DW_AT_ranges,
            AttributeValue::RangeListRef(ranges),
        );
        let var = dwarf.unit.add(block, constants::DW_TAG_variable);
        dwarf
            .unit
            .get_mut(var)
            .set(constants::DW_AT_location, AttributeValue::Exprloc(local(1)));
        let list = dwarf
            .unit
            .locations
            .add(LocationList(vec![Location::StartEnd {
                begin: Address::Constant(a.start),
                end: Address::Constant(b.end),
                data: local(1),
            }]));
        let var = dwarf.unit.add(block, constants::DW_TAG_variable);
        dwarf.unit.get_mut(var).set(
            constants::DW_AT_location,
            AttributeValue::LocationListRef(list),
        );
        if version >= 5 {
            let list = dwarf
                .unit
                .locations
                .add(LocationList(vec![Location::DefaultLocation {
                    data: local(1),
                }]));
            let var = dwarf.unit.add(block, constants::DW_TAG_variable);
            dwarf.unit.get_mut(var).set(
                constants::DW_AT_location,
                AttributeValue::LocationListRef(list),
            );
        }
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        // The expression's and the list's locations at each of the 8
        // instructions, and none for the default location.
        assert_eq!(stats.locations, 16, "DWARF {version}");
    }
}

/// A range can start at the byte before an instruction, which is the last
/// byte of the one before it. If walrus removed that one, the range starts
/// at the instruction, rather than the byte before its new address, which
/// is in whatever walrus emitted before it.
#[test]
fn range_starting_the_byte_before_an_instruction() {
    let wasm = wat::parse_str(
        r#"(module (func (export "f") (param i32)
          local.get 0
          if
            unreachable
            i32.const 128
            drop
          end))"#,
    )
    .unwrap();
    let body = &bodies(&wasm)[0];
    for version in [4, 5] {
        let mut dwarf = unit(version);
        let root = dwarf.unit.root();
        let block = dwarf.unit.add(root, constants::DW_TAG_lexical_block);
        // From the last byte of the removed `i32.const 128` to the end.
        let ranges = dwarf.unit.ranges.add(RangeList(vec![Range::StartEnd {
            begin: Address::Constant(body.instrs[4] - 1),
            end: Address::Constant(body.end),
        }]));
        dwarf.unit.get_mut(block).set(
            constants::DW_AT_ranges,
            AttributeValue::RangeListRef(ranges),
        );
        let input = with_dwarf(&wasm, dwarf);
        let output = walrus(&input, false);
        dwarf::check_readable(&output).unwrap();
        let stats = dwarf::check_preserved(&input, &output).unwrap();
        // The two `end`s, and not `unreachable`.
        assert_eq!(stats.in_scope, 2, "DWARF {version}");
    }
}
