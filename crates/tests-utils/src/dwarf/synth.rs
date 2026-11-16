//! Synthesizes DWARF for any module, laid out on its instructions.
//!
//! The choices come from an `arbitrary::Unstructured`, so a fuzzer can
//! explore them. With no data every choice takes its default, which gives
//! every instruction a line, every function a subprogram with a frame base,
//! every block a lexical block, and every local a variable with an
//! expression and another with a location list.

use super::{name_functions, parse_code, Func};
use anyhow::Result;
use arbitrary::Unstructured;
use gimli::write::{
    Address, AttributeValue, Dwarf, EndianVec, Expression, FileId, LineProgram, LineString,
    Location, LocationList, Range, RangeList, Sections, Unit, UnitEntryId,
};
use gimli::{constants, Encoding, Format, LineEncoding, LittleEndian};
use std::borrow::Cow;
use wasmparser::{Parser, Payload};

/// A choice among `n` options, 0 by default.
fn choose(u: &mut Unstructured, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    u.int_in_range(0..=n - 1).unwrap_or(0)
}

/// True with probability `1 / n`, false by default.
fn one_in(u: &mut Unstructured, n: usize) -> bool {
    // Not `Unstructured::ratio`, which is true when out of data.
    choose(u, n) == n - 1
}

/// How a list of ranges or locations writes its entries.
#[derive(Clone, Copy)]
enum ListForm {
    StartEnd,
    StartLength,
    /// Offsets from the unit's base address.
    UnitOffsets,
    /// A base address entry, then offsets from it.
    BaseOffsets,
}

struct Lists {
    version: u16,
    /// The unit's nonzero `DW_AT_low_pc`, which offsets are relative to.
    unit_base: Option<u64>,
}

impl Lists {
    fn form(&self, u: &mut Unstructured) -> ListForm {
        // DWARF 4 can't write an address pair relative to a base address.
        if self.version <= 4 && self.unit_base.is_some() {
            return match choose(u, 2) {
                0 => ListForm::UnitOffsets,
                _ => ListForm::BaseOffsets,
            };
        }
        match choose(u, 4) {
            1 => ListForm::StartLength,
            2 if self.unit_base.is_some() => ListForm::UnitOffsets,
            3 => ListForm::BaseOffsets,
            _ => ListForm::StartEnd,
        }
    }

    fn ranges(&self, u: &mut Unstructured, ranges: &[(u64, u64)]) -> RangeList {
        let form = self.form(u);
        let mut out = Vec::new();
        let base = ranges.iter().map(|r| r.0).min().unwrap_or(0);
        if let ListForm::BaseOffsets = form {
            out.push(Range::BaseAddress {
                address: Address::Constant(base),
            });
        }
        for &(begin, end) in ranges {
            // A tombstone of 0 can't be an offset from a nonzero base.
            if matches!(form, ListForm::UnitOffsets) && begin < self.unit_base.unwrap() {
                continue;
            }
            out.push(match form {
                ListForm::StartEnd => Range::StartEnd {
                    begin: Address::Constant(begin),
                    end: Address::Constant(end),
                },
                ListForm::StartLength => Range::StartLength {
                    begin: Address::Constant(begin),
                    length: end - begin,
                },
                ListForm::UnitOffsets => {
                    let base = self.unit_base.unwrap();
                    Range::OffsetPair {
                        begin: begin - base,
                        end: end - base,
                    }
                }
                ListForm::BaseOffsets => Range::OffsetPair {
                    begin: begin - base,
                    end: end - base,
                },
            });
        }
        RangeList(out)
    }

    fn locations(
        &self,
        u: &mut Unstructured,
        entries: Vec<(u64, u64, Expression)>,
        default: Option<Expression>,
    ) -> LocationList {
        let form = self.form(u);
        let mut out = Vec::new();
        let base = entries.iter().map(|r| r.0).min().unwrap_or(0);
        if let ListForm::BaseOffsets = form {
            out.push(Location::BaseAddress {
                address: Address::Constant(base),
            });
        }
        for (begin, end, data) in entries {
            if matches!(form, ListForm::UnitOffsets) && begin < self.unit_base.unwrap() {
                continue;
            }
            out.push(match form {
                ListForm::StartEnd => Location::StartEnd {
                    begin: Address::Constant(begin),
                    end: Address::Constant(end),
                    data,
                },
                ListForm::StartLength => Location::StartLength {
                    begin: Address::Constant(begin),
                    length: end - begin,
                    data,
                },
                ListForm::UnitOffsets => {
                    let base = self.unit_base.unwrap();
                    Location::OffsetPair {
                        begin: begin - base,
                        end: end - base,
                        data,
                    }
                }
                ListForm::BaseOffsets => Location::OffsetPair {
                    begin: begin - base,
                    end: end - base,
                    data,
                },
            });
        }
        if let Some(data) = default {
            out.push(Location::DefaultLocation { data });
        }
        LocationList(out)
    }
}

/// Sets an entry's ranges: as `DW_AT_low_pc` and `DW_AT_high_pc` when
/// there's one range, if chosen, and otherwise as `DW_AT_ranges`.
fn set_ranges(
    u: &mut Unstructured,
    unit: &mut Unit,
    id: UnitEntryId,
    lists: &Lists,
    ranges: &[(u64, u64)],
) {
    if let [(begin, end)] = *ranges {
        let high = match choose(u, 4) {
            0 => Some(AttributeValue::Udata(end - begin)),
            1 => Some(AttributeValue::Data4((end - begin) as u32)),
            2 => Some(AttributeValue::Address(Address::Constant(end))),
            _ => None,
        };
        if let Some(high) = high {
            let entry = unit.get_mut(id);
            entry.set(
                constants::DW_AT_low_pc,
                AttributeValue::Address(Address::Constant(begin)),
            );
            entry.set(constants::DW_AT_high_pc, high);
            return;
        }
    }
    let list = lists.ranges(u, ranges);
    let list = unit.ranges.add(list);
    unit.get_mut(id)
        .set(constants::DW_AT_ranges, AttributeValue::RangeListRef(list));
}

/// An expression for a location in `func`.
fn expression(u: &mut Unstructured, func: &Func, local: u32, globals: u32) -> Expression {
    let mut expr = Expression::new();
    match choose(u, 6) {
        0 => {
            expr.op_wasm_local(local);
            expr.op(constants::DW_OP_stack_value);
        }
        // Any local, which walrus may have removed.
        1 if func.locals > 0 => {
            expr.op_wasm_local(choose(u, func.locals as usize) as u32);
            expr.op(constants::DW_OP_stack_value);
        }
        2 if globals > 0 => {
            expr.op_wasm_global(choose(u, globals as usize) as u32);
            expr.op(constants::DW_OP_stack_value);
        }
        3 => {
            expr.op_wasm_stack(choose(u, 4) as u32);
            expr.op(constants::DW_OP_stack_value);
        }
        // A branch over the local, whose index may change size.
        4 => {
            expr.op_constu(choose(u, 2) as u64);
            let bra = expr.op_bra();
            expr.op_wasm_local(local);
            let skip = expr.op_skip();
            expr.op_constu(7);
            expr.op(constants::DW_OP_stack_value);
            expr.set_target(bra, 4);
            expr.set_target(skip, 5);
        }
        _ => {
            expr.op_constu(u.arbitrary::<u8>().unwrap_or(0) as u64);
            expr.op(constants::DW_OP_stack_value);
        }
    }
    expr
}

/// A range of `func`'s instructions, ending at an instruction or the end of
/// the body.
fn instr_range(u: &mut Unstructured, func: &Func) -> (u64, u64) {
    let n = func.raw.len();
    let a = choose(u, n);
    let b = a + 1 + choose(u, n - a);
    let end = func.raw.get(b).map_or(func.body.1, |i| i.address);
    (func.raw[a].address, end)
}

/// `wasm`, with its functions named as [`name_functions`] names them and
/// DWARF that `u` chooses, replacing any it had.
pub fn synthesize(wasm: &[u8], u: &mut Unstructured) -> Result<Vec<u8>> {
    let wasm = name_functions(wasm)?;
    let code = parse_code(&wasm)?;
    let funcs: Vec<&Func> = code.funcs.iter().filter(|f| !f.raw.is_empty()).collect();

    let version = if u.arbitrary::<bool>().unwrap_or(false) {
        4
    } else {
        5
    };
    let encoding = Encoding {
        format: Format::Dwarf32,
        version,
        address_size: 4,
    };
    let mut dwarf = Dwarf::new();
    let mut next_line = 1;
    let units = 1 + choose(u, funcs.len().min(3));
    let per_unit = funcs.len().div_ceil(units).max(1);
    for (u_index, chunk) in funcs.chunks(per_unit).enumerate() {
        let name = format!("unit{u_index}.wat");
        let program = LineProgram::new(
            encoding,
            LineEncoding::default(),
            LineString::String(b"/work".to_vec()),
            None,
            LineString::String(name.clone().into_bytes()),
            None,
        );
        let mut unit = Unit::new(encoding, program);
        let dir = unit.line_program.default_directory();
        let files: Vec<FileId> = [name.clone(), format!("other{u_index}.wat")]
            .into_iter()
            .map(|f| {
                unit.line_program
                    .add_file(LineString::String(f.into_bytes()), dir, None)
            })
            .collect();
        let root = unit.root();
        unit.get_mut(root).set(
            constants::DW_AT_name,
            AttributeValue::String(name.into_bytes()),
        );
        unit.get_mut(root).set(
            constants::DW_AT_producer,
            AttributeValue::String(b"walrus-tests-utils".to_vec()),
        );

        let bodies: Vec<(u64, u64)> = chunk.iter().map(|f| f.body).collect();
        let unit_base = match choose(u, 3) {
            0 => {
                unit.get_mut(root).set(
                    constants::DW_AT_low_pc,
                    AttributeValue::Address(Address::Constant(0)),
                );
                None
            }
            1 => {
                let base = bodies[0].0;
                unit.get_mut(root).set(
                    constants::DW_AT_low_pc,
                    AttributeValue::Address(Address::Constant(base)),
                );
                Some(base)
            }
            _ => None,
        };
        let lists = Lists { version, unit_base };
        if unit_base.is_some() || one_in(u, 2) {
            let list = lists.ranges(u, &bodies);
            let list = unit.ranges.add(list);
            unit.get_mut(root)
                .set(constants::DW_AT_ranges, AttributeValue::RangeListRef(list));
        }

        // Line sequences, each over one or more functions.
        let mut i = 0;
        while i < chunk.len() {
            let n = 1 + if one_in(u, 3) {
                choose(u, chunk.len() - i)
            } else {
                0
            };
            let seq = &chunk[i..i + n];
            i += n;
            let start = seq[0].body.0;
            // A sequence for discarded code, which no instruction is in.
            if one_in(u, 8) {
                let dead = [0, 0xFFFF_FFFE][choose(u, 2)];
                unit.line_program
                    .begin_sequence(Some(Address::Constant(dead)));
                unit.line_program.row().line = next_line;
                unit.line_program.generate_row();
                unit.line_program.end_sequence(1);
            }
            unit.line_program
                .begin_sequence(Some(Address::Constant(start)));
            for func in seq {
                for instr in &func.raw {
                    if one_in(u, 5) {
                        continue;
                    }
                    let row = unit.line_program.row();
                    row.address_offset = instr.address - start;
                    row.line = next_line;
                    row.column = choose(u, 3) as u64;
                    row.file = files[choose(u, files.len())];
                    next_line += 1;
                    unit.line_program.generate_row();
                }
            }
            unit.line_program
                .end_sequence(seq[seq.len() - 1].body.1 - start);
        }

        for func in chunk {
            let sub = unit.add(root, constants::DW_TAG_subprogram);
            let name = func.name.clone().unwrap_or_default();
            unit.get_mut(sub)
                .set(constants::DW_AT_name, AttributeValue::String(name.into()));
            if one_in(u, 10) {
                // Discarded by the linker.
                let dead = [0, 0xFFFF_FFFE][choose(u, 2)];
                set_ranges(u, &mut unit, sub, &lists, &[(dead, dead + 1)]);
            } else {
                set_ranges(u, &mut unit, sub, &lists, &[func.body]);
            }
            if func.locals > 0 && !one_in(u, 4) {
                let local = func.locals - 1 - choose(u, func.locals as usize) as u32;
                let mut expr = Expression::new();
                expr.op_wasm_local(local);
                unit.get_mut(sub)
                    .set(constants::DW_AT_frame_base, AttributeValue::Exprloc(expr));
            }

            // A lexical block for each block, loop, if and try.
            let mut open = vec![sub];
            let mut starts = Vec::new();
            for (k, instr) in func.raw.iter().enumerate() {
                match instr.kind.as_str() {
                    "Block" | "Loop" | "If" | "Try" | "TryTable" => {
                        let parent = *open.last().unwrap();
                        let block = if one_in(u, 4) {
                            None
                        } else {
                            Some(unit.add(parent, constants::DW_TAG_lexical_block))
                        };
                        open.push(block.unwrap_or(parent));
                        starts.push((block, instr.address));
                    }
                    "End" | "Delegate" if !starts.is_empty() => {
                        let (block, start) = starts.pop().unwrap();
                        open.pop();
                        let end = func.raw.get(k + 1).map_or(func.body.1, |i| i.address);
                        if let Some(block) = block {
                            set_ranges(u, &mut unit, block, &lists, &[(start, end)]);
                        }
                    }
                    _ => {}
                }
            }

            for local in 0..func.locals {
                let tag = if local < func.params {
                    constants::DW_TAG_formal_parameter
                } else {
                    constants::DW_TAG_variable
                };
                let var = unit.add(sub, tag);
                unit.get_mut(var).set(
                    constants::DW_AT_name,
                    AttributeValue::String(format!("l{local}").into_bytes()),
                );
                let expr = expression(u, func, local, code.globals);
                unit.get_mut(var)
                    .set(constants::DW_AT_location, AttributeValue::Exprloc(expr));

                // The same local's location over some instructions.
                let var = unit.add(sub, constants::DW_TAG_variable);
                unit.get_mut(var).set(
                    constants::DW_AT_name,
                    AttributeValue::String(format!("l{local}_list").into_bytes()),
                );
                let mut entries = Vec::new();
                if u.is_empty() {
                    let half = &func.raw[func.raw.len() / 2];
                    let expr = expression(u, func, local, code.globals);
                    entries.push((half.address, func.body.1, expr));
                } else {
                    for _ in 0..1 + choose(u, 3) {
                        let (begin, end) = if one_in(u, 10) {
                            let dead = [0, 0xFFFF_FFFE][choose(u, 2)];
                            (dead, dead + 1)
                        } else {
                            instr_range(u, func)
                        };
                        let expr = expression(u, func, local, code.globals);
                        entries.push((begin, end, expr));
                    }
                }
                let default = (version >= 5 && one_in(u, 6))
                    .then(|| expression(u, func, local, code.globals));
                let list = lists.locations(u, entries, default);
                let list = unit.locations.add(list);
                unit.get_mut(var).set(
                    constants::DW_AT_location,
                    AttributeValue::LocationListRef(list),
                );
            }
        }
        dwarf.units.add(unit);
    }

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections)?;
    let mut debug = Vec::new();
    sections.for_each(|id, data| {
        if !data.slice().is_empty() {
            debug.push((id.name(), data.slice().to_vec()));
        }
        Ok::<_, gimli::write::Error>(())
    })?;

    let mut module = wasm_encoder::Module::new();
    for payload in Parser::new(0).parse_all(&wasm) {
        let payload = payload?;
        if let Payload::CustomSection(section) = &payload {
            if !section.name().starts_with(".debug") {
                module.section(&wasm_encoder::CustomSection {
                    name: Cow::Borrowed(section.name()),
                    data: Cow::Borrowed(section.data()),
                });
            }
            continue;
        }
        if let Some((id, range)) = payload.as_section() {
            module.section(&wasm_encoder::RawSection {
                id,
                data: &wasm[range],
            });
        }
    }
    for (name, data) in &debug {
        module.section(&wasm_encoder::CustomSection {
            name: Cow::Borrowed(name),
            data: Cow::Borrowed(data),
        });
    }
    Ok(module.finish())
}
