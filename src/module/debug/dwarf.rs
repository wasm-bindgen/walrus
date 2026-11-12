//! Convert the input module's DWARF to match the emitted code.
//!
//! walrus re-encodes every instruction, reorders functions, drops
//! unreachable code, and passes may move, copy or remove instructions, so
//! code addresses don't move by a constant. Everything is therefore derived
//! from where each input instruction was emitted (its `InstrLocId`):
//!
//! - A line table row is generated wherever the input row covering the
//!   emitted instruction changes.
//! - A range or location list entry covers the emitted instructions whose
//!   input instructions it covered, merged into runs.
//! - A single code address (such as `DW_AT_low_pc` without a size, or
//!   `DW_AT_call_return_pc`) moves to the first instruction at or after it
//!   that is still emitted.
//!
//! `gimli::write::Dwarf::from` can't do this: it only passes each address
//! through a callback, and passes offsets from a base address through it as
//! if they were addresses. So this drives `gimli::write::Dwarf::convert`.
//!
//! Addresses in expressions (`DW_OP_addr`) are linear memory addresses of
//! static data in Wasm, so they're kept. walrus also renumbers locals (it
//! groups them by type and drops unused ones) and may remove globals, so the
//! indices in `DW_OP_WASM_location` are remapped.

use std::collections::HashMap;

use crate::{FunctionId, InstrLocId};
use gimli::write::{self, Address, ConvertLineSequenceEnd, ConvertResult};
use gimli::{constants, read, AttributeValue, EndianRcSlice, LittleEndian, Reader};

/// An owned reader, so that an expression with remapped indices can be
/// converted by gimli like any other.
pub(crate) type R = EndianRcSlice<LittleEndian>;

/// Tombstone for addresses in code that no longer exists.
///
/// `-2` rather than `-1`: in DWARF 4 `.debug_loc` and `.debug_ranges` with
/// 4-byte addresses, a `begin` of `-1` is the base address selection marker
/// (which carries no expression length), so a `-1` tombstone desyncs every
/// reader of the rest of the section. lld makes the same choice.
pub(crate) static DEAD_CODE: u64 = 0xFFFF_FFFE;

/// Addresses linkers write for code they discarded.
fn is_tombstone(address: u64) -> bool {
    address == 0 || address >= DEAD_CODE
}

/// A function in the input module, by code-section-relative addresses.
pub(crate) struct InputFunction<'a> {
    pub id: FunctionId,
    /// The body, after its size. LLVM's `DW_AT_low_pc` is its start.
    pub start: u64,
    pub end: u64,
    /// Each instruction's address and location, sorted by address.
    pub instrs: &'a [(usize, InstrLocId)],
}

/// A function in the output module.
pub(crate) struct OutputFunction<'a> {
    pub id: FunctionId,
    /// The body, after its size, code-section-relative.
    pub start: u64,
    pub end: u64,
    /// Each emitted instruction's location and absolute offset, in order.
    pub instrs: &'a [(InstrLocId, usize)],
    /// Subtracted from `instrs` offsets to make them code-section-relative.
    pub base: usize,
}

impl OutputFunction<'_> {
    /// Position 0 is the body start, position `i + 1` is instruction `i`,
    /// and the position after the last instruction is the body end.
    fn address(&self, position: usize) -> u64 {
        match position {
            0 => self.start,
            p if p <= self.instrs.len() => (self.instrs[p - 1].1 - self.base) as u64,
            _ => self.end,
        }
    }
}

/// Where each input instruction was emitted.
pub(crate) struct CodeMap<'a> {
    /// Sorted by start.
    inputs: Vec<InputFunction<'a>>,
    /// Sorted by start.
    outputs: Vec<OutputFunction<'a>>,
    output_of: HashMap<FunctionId, usize>,
    input_of: HashMap<FunctionId, usize>,
    /// (location, output function, position), sorted.
    emitted: Vec<(InstrLocId, u32, u32)>,
    /// The input address of each location, sorted.
    input_addresses: Vec<(InstrLocId, u64)>,
    /// For each output function, the number of positions before each
    /// position that have an input address.
    known_before: Vec<Vec<u32>>,
    /// For each input instruction, the index in `emitted` of its first
    /// emission, or `emitted.len()` if it has none.
    first_emission: Vec<Vec<u32>>,
    /// For each input function, its output function if all of its code was
    /// emitted there and nothing else was, so that a range covering the
    /// whole input function covers the whole output function.
    whole: Vec<Option<usize>>,
}

/// A run of emitted code: (output function, start, end).
type Run = (usize, u64, u64);

impl<'a> CodeMap<'a> {
    pub(crate) fn new(
        mut inputs: Vec<InputFunction<'a>>,
        mut outputs: Vec<OutputFunction<'a>>,
    ) -> Self {
        inputs.sort_unstable_by_key(|f| f.start);
        outputs.sort_unstable_by_key(|f| f.start);
        let output_of: HashMap<_, _> = outputs.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
        let input_of: HashMap<_, _> = inputs.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
        let mut input_addresses: Vec<(InstrLocId, u64)> = inputs
            .iter()
            .flat_map(|f| f.instrs.iter().map(|&(a, loc)| (loc, a as u64)))
            .collect();
        input_addresses.sort_unstable();
        input_addresses.dedup_by_key(|x| x.0);
        let input_index = |address: u64| -> Option<usize> {
            let i = inputs
                .partition_point(|f| f.start <= address)
                .checked_sub(1)?;
            (address <= inputs[i].end).then_some(i)
        };
        let mut emitted = Vec::new();
        let mut known_before = Vec::with_capacity(outputs.len());
        // The input function that each output function's located code came
        // from: `Some(Some(i))` for one, `Some(None)` for several.
        let mut sole: Vec<Option<Option<usize>>> = Vec::with_capacity(outputs.len());
        for (o, f) in outputs.iter().enumerate() {
            debug_assert!(f.instrs.windows(2).all(|w| w[0].1 <= w[1].1));
            // The body start is known if the function was in the input.
            let input = input_of.get(&f.id).copied();
            let mut from = input.map(Some);
            let mut known = vec![0, u32::from(input.is_some())];
            let mut last = None;
            for (i, &(loc, _)) in f.instrs.iter().enumerate() {
                let address = if loc.is_default() {
                    None
                } else {
                    input_addresses
                        .binary_search_by_key(&loc, |x| x.0)
                        .ok()
                        .map(|i| input_addresses[i].1)
                };
                if let Some(address) = address {
                    emitted.push((loc, o as u32, i as u32 + 1));
                    // Consecutive instructions are usually from one function.
                    let in_last = last.is_some_and(|l: usize| {
                        inputs[l].start <= address && address <= inputs[l].end
                    });
                    if !in_last {
                        last = input_index(address);
                    }
                    from = match from {
                        None => Some(last),
                        Some(s) if s == last => Some(s),
                        Some(_) => Some(None),
                    };
                }
                known.push(known.last().unwrap() + u32::from(address.is_some()));
            }
            known_before.push(known);
            sole.push(from);
        }
        emitted.sort_unstable();

        let mut first_emission = Vec::with_capacity(inputs.len());
        let mut whole = Vec::with_capacity(inputs.len());
        for (i, f) in inputs.iter().enumerate() {
            let o = output_of.get(&f.id).copied();
            let mut only_o = o.is_some_and(|o| sole[o] == Some(Some(i)));
            let first: Vec<u32> = f
                .instrs
                .iter()
                .map(|&(_, loc)| {
                    let e = emitted.partition_point(|x| x.0 < loc);
                    for x in emitted[e..].iter().take_while(|x| x.0 == loc) {
                        only_o &= Some(x.1 as usize) == o;
                    }
                    e as u32
                })
                .collect();
            first_emission.push(first);
            whole.push(o.filter(|_| only_o));
        }

        CodeMap {
            inputs,
            outputs,
            output_of,
            input_of,
            emitted,
            input_addresses,
            known_before,
            first_emission,
            whole,
        }
    }

    /// The index of the input function whose body contains `address`,
    /// including its end.
    fn input_index(&self, address: u64) -> Option<usize> {
        let i = self
            .inputs
            .partition_point(|f| f.start <= address)
            .checked_sub(1)?;
        (address <= self.inputs[i].end).then_some(i)
    }

    /// The input function whose body contains `address`, including its end.
    fn input_function(&self, address: u64) -> Option<&InputFunction<'a>> {
        Some(&self.inputs[self.input_index(address)?])
    }

    /// Where instruction `i` of input function `f` was emitted, as (output
    /// function, position).
    fn emissions(&self, f: usize, i: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let loc = self.inputs[f].instrs[i].1;
        let first = self.first_emission[f][i] as usize;
        self.emitted[first..]
            .iter()
            .take_while(move |x| x.0 == loc)
            .map(|&(_, o, p)| (o as usize, p as usize))
    }

    /// The input address of the code at `position` in output function `o`.
    fn input_address(&self, o: usize, position: usize) -> Option<u64> {
        let f = &self.outputs[o];
        if position == 0 {
            return self.input_of.get(&f.id).map(|&i| self.inputs[i].start);
        }
        let loc = f.instrs.get(position - 1)?.0;
        let i = self
            .input_addresses
            .binary_search_by_key(&loc, |x| x.0)
            .ok()?;
        Some(self.input_addresses[i].1)
    }

    /// A single code address, or `None` if its code was removed.
    fn address(&self, address: u64) -> Option<u64> {
        let fi = self.input_index(address)?;
        let f = &self.inputs[fi];
        let o = *self.output_of.get(&f.id)?;
        let out = &self.outputs[o];
        if address <= f.start {
            return Some(out.start);
        }
        let first = f.instrs.partition_point(|x| (x.0 as u64) < address);
        for i in first..f.instrs.len() {
            if let Some((_, p)) = self.emissions(fi, i).find(|e| e.0 == o) {
                return Some(out.address(p));
            }
        }
        Some(out.end)
    }

    /// The emitted code covering what `begin..end` covered, as runs sorted
    /// by address. Empty if all of it was removed.
    fn ranges(&self, begin: u64, end: u64) -> Vec<Run> {
        if is_tombstone(begin) || begin >= end {
            return Vec::new();
        }
        let mut hits = Vec::new();
        let mut runs = Vec::new();
        let first = self.inputs.partition_point(|f| f.end <= begin);
        for (fi, f) in self.inputs.iter().enumerate().skip(first) {
            if f.start >= end {
                break;
            }
            let (b, e) = (begin.max(f.start), end.min(f.end));
            // All of a function that was emitted as a whole.
            if let (true, Some(o)) = (b == f.start && e == f.end, self.whole[fi]) {
                let out = &self.outputs[o];
                runs.push((o, out.start, out.end));
                continue;
            }
            if b == f.start {
                if let Some(&o) = self.output_of.get(&f.id) {
                    hits.push((o, 0));
                }
            }
            let i = f.instrs.partition_point(|x| (x.0 as u64) < b);
            for (i, &(a, _)) in f.instrs.iter().enumerate().skip(i) {
                if a as u64 >= e {
                    break;
                }
                hits.extend(self.emissions(fi, i));
            }
        }
        if !hits.is_sorted() {
            hits.sort_unstable();
        }
        hits.dedup();
        // Merge positions into runs, across gaps of code with no input
        // location (such as instructions a pass added).
        let mut hits = hits.into_iter();
        let Some(mut run) = hits.next().map(|(o, p)| (o, p, p)) else {
            runs.sort_unstable_by_key(|r| r.1);
            return runs;
        };
        let mut finish = |(o, first, mut last): (usize, usize, usize)| {
            let f = &self.outputs[o];
            let known = &self.known_before[o];
            // A location emitted with no bytes (such as an elided `else`)
            // shares its address with the code after it; if that code has no
            // location of its own, it's what the location became.
            while last < f.instrs.len()
                && f.address(last + 1) == f.address(last)
                && known[last + 2] == known[last + 1]
            {
                last += 1;
            }
            let (s, e) = (f.address(first), f.address(last + 1));
            if s < e {
                runs.push((o, s, e));
            }
        };
        for (o, p) in hits {
            let known = &self.known_before[o];
            if o == run.0 && known[p] == known[run.2 + 1] {
                run.2 = p;
            } else {
                finish(run);
                run = (o, p, p);
            }
        }
        finish(run);
        runs.sort_unstable_by_key(|r| r.1);
        runs
    }
}

/// New indices of the input module's locals and globals, or `None` for ones
/// that are no longer emitted.
pub(crate) struct Indices<'a> {
    /// Takes the input function, the output function the code is in, and
    /// the input local index.
    pub local: &'a dyn Fn(FunctionId, FunctionId, u32) -> Option<u32>,
    /// Takes the input global index.
    pub global: &'a dyn Fn(u32) -> Option<u32>,
}

/// How to convert the entries of one unit.
struct Context<'a, 'b> {
    code: &'a CodeMap<'b>,
    indices: Option<&'a Indices<'b>>,
    /// The unit has a nonzero base address, which DWARF 2-4 list entries
    /// are relative to. Lists then start with a base address entry of 0, and
    /// hold offsets from it.
    relative_to_unit: bool,
}

impl Context<'_, '_> {
    /// A code address in an attribute.
    fn address(&self, address: u64) -> Option<Address> {
        if is_tombstone(address) {
            return Some(Address::Constant(address));
        }
        Some(Address::Constant(
            self.code.address(address).unwrap_or(DEAD_CODE),
        ))
    }
}

/// Addresses in expressions are data addresses.
fn data_address(address: u64) -> Option<Address> {
    Some(Address::Constant(address))
}

/// `indices` is `None` when the input indices weren't recorded, in which
/// case `DW_OP_WASM_location` is copied unchanged.
pub(crate) fn convert(
    from: &read::Dwarf<R>,
    code: &CodeMap,
    indices: Option<&Indices>,
) -> ConvertResult<write::Dwarf> {
    let mut dwarf = write::Dwarf::default();
    let mut units = dwarf.convert(from)?;
    while let Some((mut unit, root)) = units.read_unit()? {
        if let Some(program) = unit.read_line_program(None, None)? {
            let (program, files) = convert_line_program(program, code)?;
            unit.set_line_program(program, files);
        }
        let mut cx = Context {
            code,
            indices,
            relative_to_unit: false,
        };
        // In DWARF 2-4, range and location list entries are relative to the
        // unit's `DW_AT_low_pc` unless a list sets its own base. gimli's
        // writer treats the unit as having a base address when its converted
        // `DW_AT_low_pc` is nonzero.
        let root_pc = convert_pc(&root, &cx)?;
        cx.relative_to_unit = root.read_unit.encoding().version <= 4
            && matches!(root_pc, Pc::Single(a) | Pc::Range(a, _) if a != 0);
        // The function each entry is in, for the local indices in its
        // expressions: the one its own `DW_AT_low_pc` is in, or else its
        // parent's.
        // Entries are read depth first, so this holds the ancestors of the
        // current entry.
        let mut functions = Vec::new();
        let root_id = unit.unit.root();
        let function = entry_function(&root, None, code)?;
        functions.push((root_id, function));
        convert_attributes(&mut unit, root_id, &root, root_pc, function, &cx)?;
        let mut entry = root;
        while let Some(id) = unit.read_entry(&mut entry)? {
            let id = unit.add_entry(id, &entry);
            while functions.last().is_some_and(|f| Some(f.0) != entry.parent) {
                functions.pop();
            }
            let parent = functions.last().and_then(|f| f.1);
            let function = entry_function(&entry, parent, code)?;
            functions.push((id, function));
            let pc = convert_pc(&entry, &cx)?;
            convert_attributes(&mut unit, id, &entry, pc, function, &cx)?;
        }
    }
    Ok(dwarf)
}

/// One sequence of the input line program.
struct Sequence {
    start: u64,
    end: u64,
    /// Rows with their addresses, sorted.
    rows: Vec<(u64, write::LineRow)>,
}

/// Each output function gets a sequence from the unit that its own input
/// code belonged to, with a row wherever the input row covering its
/// instructions changes. Instructions with no input location continue the
/// row before them.
fn convert_line_program(
    mut program: write::ConvertLineProgram<'_, R>,
    code: &CodeMap,
) -> ConvertResult<(write::LineProgram, Vec<write::FileId>)> {
    let mut sequences = Vec::new();
    while let Some(sequence) = program.read_sequence()? {
        let Some(start) = sequence.start else {
            continue;
        };
        if is_tombstone(start) {
            continue;
        }
        let end = match sequence.end {
            ConvertLineSequenceEnd::Length(length) => start + length,
            ConvertLineSequenceEnd::Address(address) => address,
        };
        let rows = sequence
            .rows
            .into_iter()
            .map(|row| (start + row.address_offset, row))
            .collect();
        sequences.push(Sequence { start, end, rows });
    }
    sequences.sort_by_key(|s| s.start);

    // The rows covering an input address: those at it, or else the last one
    // before it, in the same function. (A sequence can run on from one
    // function into the next, whose start then seems to have the last row
    // of the function before.) Returned as (sequence, rows).
    let covering = |address: u64| -> Option<(usize, std::ops::Range<usize>)> {
        let s = sequences
            .partition_point(|s| s.start <= address)
            .checked_sub(1)?;
        let sequence = &sequences[s];
        if address >= sequence.end {
            return None;
        }
        let j = sequence.rows.partition_point(|r| r.0 <= address);
        let last = sequence.rows.get(j.checked_sub(1)?)?.0;
        if code.input_function(address).is_some_and(|f| last < f.start) {
            return None;
        }
        let i = if last == address {
            sequence.rows.partition_point(|r| r.0 < address)
        } else {
            j - 1
        };
        Some((s, i..j))
    };

    for (o, f) in code.outputs.iter().enumerate() {
        // Only the unit whose line program covers the function's own input
        // code describes it.
        let Some(&input) = code.input_of.get(&f.id) else {
            continue;
        };
        let input = &code.inputs[input];
        let owns = covering(input.start).is_some()
            || input
                .instrs
                .first()
                .is_some_and(|&(a, _)| covering(a as u64).is_some());
        if !owns {
            continue;
        }
        // The rows in effect, and the start of the open sequence. Output
        // instructions with no input location keep the rows in effect; an
        // input address no row covers ends the sequence, as in the input.
        let mut current = None;
        let mut open = None;
        for position in 0..=f.instrs.len() {
            let Some(input) = code.input_address(o, position) else {
                continue;
            };
            let address = f.address(position);
            let Some(rows) = covering(input) else {
                if let Some(start) = open.take() {
                    program.end_sequence(address - start);
                    current = None;
                }
                continue;
            };
            if current.as_ref() == Some(&rows) {
                continue;
            }
            let start = *open.get_or_insert_with(|| {
                program.begin_sequence(Some(Address::Constant(address)));
                address
            });
            for (_, row) in &sequences[rows.0].rows[rows.1.clone()] {
                let mut row = *row;
                row.address_offset = address - start;
                program.generate_row(row);
            }
            current = Some(rows);
        }
        if let Some(start) = open {
            program.end_sequence(f.end - start);
        }
    }
    Ok(program.program())
}

fn entry_function(
    entry: &write::ConvertUnitEntry<'_, R>,
    parent: Option<FunctionId>,
    code: &CodeMap,
) -> ConvertResult<Option<FunctionId>> {
    let low_pc = match entry.attr_value(constants::DW_AT_low_pc) {
        Some(value) => entry.read_unit.attr_address(value)?,
        None => None,
    };
    Ok(match low_pc {
        Some(low_pc) if is_tombstone(low_pc) => None,
        Some(low_pc) => code
            .input_function(low_pc)
            .filter(|f| low_pc < f.end)
            .map(|f| f.id),
        None => parent,
    })
}

/// An entry's converted `DW_AT_low_pc` and `DW_AT_high_pc`.
#[derive(Clone, PartialEq)]
enum Pc {
    /// Neither, or left as they were.
    Unchanged,
    /// `DW_AT_low_pc` alone.
    Single(u64),
    /// One range: its start, and `DW_AT_high_pc` with whether it's a size.
    Range(u64, (u64, bool)),
    /// The code is now in several places, so the pair becomes `DW_AT_ranges`.
    Ranges(Vec<Run>),
    /// All of the code was removed. Has whether `DW_AT_high_pc` is a size.
    Removed(bool),
}

fn convert_pc(entry: &write::ConvertUnitEntry<'_, R>, cx: &Context) -> ConvertResult<Pc> {
    let read_unit = entry.read_unit;
    let low = match entry.attr_value(constants::DW_AT_low_pc) {
        Some(value) => read_unit.attr_address(value)?,
        None => None,
    };
    let Some(low) = low else {
        return Ok(Pc::Unchanged);
    };
    if is_tombstone(low) {
        return Ok(Pc::Unchanged);
    }
    let high = match entry.attr_value(constants::DW_AT_high_pc) {
        None => None,
        Some(AttributeValue::Udata(size)) => Some((low + size, true)),
        Some(value) => read_unit.attr_address(value)?.map(|a| (a, false)),
    };
    let Some((high, is_size)) = high else {
        return Ok(Pc::Single(cx.code.address(low).unwrap_or(DEAD_CODE)));
    };
    let mut runs = cx.code.ranges(low, high);
    Ok(match runs.len() {
        0 => Pc::Removed(is_size),
        1 => {
            let (_, s, e) = runs.pop().unwrap();
            Pc::Range(s, (if is_size { e - s } else { e }, is_size))
        }
        _ => Pc::Ranges(runs),
    })
}

fn convert_attributes(
    unit: &mut write::ConvertUnit<'_, R>,
    id: write::UnitEntryId,
    entry: &write::ConvertUnitEntry<'_, R>,
    pc: Pc,
    function: Option<FunctionId>,
    cx: &Context,
) -> ConvertResult<()> {
    let address = |a| cx.address(a);
    let is_root = entry.parent.is_none();
    match &pc {
        Pc::Unchanged => {}
        Pc::Single(a) => {
            let value = write::AttributeValue::Address(Address::Constant(*a));
            unit.unit.get_mut(id).set(constants::DW_AT_low_pc, value);
        }
        Pc::Range(low, (high, is_size)) => {
            let low = write::AttributeValue::Address(Address::Constant(*low));
            let high = match (*is_size, u32::try_from(*high)) {
                // Keep the fixed-width form LLVM emits: consumers that patch
                // code addresses in place (binaryen's DWARF update) rely on
                // the size not changing.
                (true, Ok(size)) => write::AttributeValue::Data4(size),
                (true, Err(_)) => write::AttributeValue::Udata(*high),
                (false, _) => write::AttributeValue::Address(Address::Constant(*high)),
            };
            let e = unit.unit.get_mut(id);
            e.set(constants::DW_AT_low_pc, low);
            e.set(constants::DW_AT_high_pc, high);
        }
        Pc::Removed(is_size) => {
            let dead = write::AttributeValue::Address(Address::Constant(DEAD_CODE));
            let high = if *is_size {
                write::AttributeValue::Data4(0)
            } else {
                dead.clone()
            };
            let e = unit.unit.get_mut(id);
            e.set(constants::DW_AT_low_pc, dead);
            e.set(constants::DW_AT_high_pc, high);
        }
        Pc::Ranges(runs) => {
            let runs: Vec<_> = runs.iter().map(|&(_, s, e)| (s, e)).collect();
            let list = write_range_list(&runs, cx.relative_to_unit && !is_root);
            let list = unit.unit.ranges.add(list);
            let e = unit.unit.get_mut(id);
            if is_root {
                // The unit's base address.
                e.set(
                    constants::DW_AT_low_pc,
                    write::AttributeValue::Address(Address::Constant(0)),
                );
            }
            e.set(
                constants::DW_AT_ranges,
                write::AttributeValue::RangeListRef(list),
            );
        }
    }
    for attr in &entry.attrs {
        let read_unit = entry.read_unit;
        let name = attr.name();
        if pc != Pc::Unchanged
            && (name == constants::DW_AT_low_pc || name == constants::DW_AT_high_pc)
        {
            continue;
        }
        let value =
            match attr.value() {
                // Not supported by gimli, and safe to drop.
                _ if name == constants::DW_AT_GNU_locviews => continue,
                // gimli can't write macro sections, so these offsets would point
                // at nothing.
                AttributeValue::DebugMacinfoRef(_) | AttributeValue::DebugMacroRef(_) => continue,
                _ if name == constants::DW_AT_GNU_macros => continue,
                AttributeValue::LocationListsRef(offset) => {
                    convert_location_list(unit, read_unit, offset, function, cx)?
                }
                AttributeValue::DebugLocListsIndex(index) => {
                    let offset = read_unit.locations_offset(index)?;
                    convert_location_list(unit, read_unit, offset, function, cx)?
                }
                AttributeValue::RangeListsRef(offset) => {
                    let offset = read_unit.ranges_offset_from_raw(offset);
                    convert_range_list(unit, read_unit, offset, cx)?
                }
                AttributeValue::DebugRngListsIndex(index) => {
                    let offset = read_unit.ranges_offset(index)?;
                    convert_range_list(unit, read_unit, offset, cx)?
                }
                AttributeValue::Exprloc(expression) => {
                    let remapped = remap_wasm_locations(
                        &expression,
                        read_unit.encoding(),
                        function.map(|f| (f, f)),
                        cx,
                    )?;
                    match remapped {
                        Some(expression) => write::AttributeValue::Exprloc(
                            unit.convert_expression(read_unit, expression, &data_address)?,
                        ),
                        // The variable is now optimized out.
                        None => continue,
                    }
                }
                _ => unit.convert_attribute_value(read_unit, attr, &address)?,
            };
        unit.unit.get_mut(id).set(name, value);
    }
    Ok(())
}

fn write_range_list(runs: &[(u64, u64)], relative_to_unit: bool) -> write::RangeList {
    let mut list = Vec::new();
    if relative_to_unit && !runs.is_empty() {
        list.push(write::Range::BaseAddress {
            address: Address::Constant(0),
        });
    }
    for &(begin, end) in runs {
        list.push(if relative_to_unit {
            write::Range::OffsetPair { begin, end }
        } else {
            write::Range::StartEnd {
                begin: Address::Constant(begin),
                end: Address::Constant(end),
            }
        });
    }
    write::RangeList(list)
}

fn convert_location_list(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    offset: gimli::LocationListsOffset,
    function: Option<FunctionId>,
    cx: &Context,
) -> ConvertResult<write::AttributeValue> {
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.locations(offset)?;
    while let Some(entry) = from.next()? {
        // gimli resolves `DW_LLE_default_location` to the whole address
        // space, which no other entry can cover with 4-byte addresses. It
        // applies wherever no other entry does, so it needs no mapping, and
        // its locals are those of the entry's function.
        if entry.range.begin == 0 && entry.range.end == u64::MAX {
            let functions = function.map(|f| (f, f));
            if let Some(data) =
                remap_wasm_locations(&entry.data, read_unit.encoding(), functions, cx)?
            {
                let data = unit.convert_expression(read_unit, data, &data_address)?;
                list.push(write::Location::DefaultLocation { data });
            }
            continue;
        }
        let runs = cx.code.ranges(entry.range.begin, entry.range.end);
        if runs.is_empty() {
            continue;
        }
        let input = cx.code.input_function(entry.range.begin).map(|f| f.id);
        // The expression for each output function, as local indices differ.
        let mut converted: HashMap<usize, Option<write::Expression>> = HashMap::new();
        for (o, begin, end) in runs {
            let data = match converted.get(&o) {
                Some(data) => data.clone(),
                None => {
                    let output = cx.code.outputs[o].id;
                    let remapped = remap_wasm_locations(
                        &entry.data,
                        read_unit.encoding(),
                        input.map(|i| (i, output)),
                        cx,
                    )?;
                    let data = match remapped {
                        Some(e) => Some(unit.convert_expression(read_unit, e, &data_address)?),
                        None => None,
                    };
                    converted.insert(o, data.clone());
                    data
                }
            };
            let Some(data) = data else {
                continue;
            };
            if cx.relative_to_unit && list.is_empty() {
                list.push(write::Location::BaseAddress {
                    address: Address::Constant(0),
                });
            }
            list.push(if cx.relative_to_unit {
                write::Location::OffsetPair { begin, end, data }
            } else {
                write::Location::StartEnd {
                    begin: Address::Constant(begin),
                    end: Address::Constant(end),
                    data,
                }
            });
        }
    }
    let id = unit.unit.locations.add(write::LocationList(list));
    Ok(write::AttributeValue::LocationListRef(id))
}

fn convert_range_list(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    offset: gimli::RangeListsOffset,
    cx: &Context,
) -> ConvertResult<write::AttributeValue> {
    let mut runs = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.ranges(offset)?;
    while let Some(range) = from.next()? {
        runs.extend(
            cx.code
                .ranges(range.begin, range.end)
                .into_iter()
                .map(|(_, s, e)| (s, e)),
        );
    }
    let list = write_range_list(&runs, cx.relative_to_unit);
    let id = unit.unit.ranges.add(list);
    Ok(write::AttributeValue::RangeListRef(id))
}

/// Remaps the local and global indices in an expression's
/// `DW_OP_WASM_location` operations, or returns `None` if it names a local
/// or global that walrus removed. `functions` is the input function whose
/// local indices the expression uses and the output function its code is
/// now in; without it, the expression describes code that isn't in the
/// module (such as a function the linker discarded) and its local indices
/// are left as they are.
///
/// A new index can take more or fewer LEB128 bytes than the old one, so the
/// offsets of `DW_OP_bra` and `DW_OP_skip` are recomputed. (gimli re-encodes
/// the result, and recomputes them again for its own encoding.)
fn remap_wasm_locations(
    expression: &read::Expression<R>,
    encoding: gimli::Encoding,
    functions: Option<(FunctionId, FunctionId)>,
    cx: &Context,
) -> ConvertResult<Option<read::Expression<R>>> {
    let Some(indices) = cx.indices else {
        return Ok(Some(expression.clone()));
    };
    // Each operation's offset in the input, its bytes in the output, and
    // for a branch, the input offset of its target.
    let mut ops: Vec<(usize, Vec<u8>, Option<isize>)> = Vec::new();
    let mut changed = false;
    let len = expression.0.len();
    let mut rest = expression.0.clone();
    while !rest.is_empty() {
        let offset = len - rest.len();
        let mut bytes = rest.clone();
        let op = read::Operation::parse(&mut rest, encoding)?;
        bytes.truncate(bytes.len() - rest.len())?;
        let bytes = bytes.to_slice()?;
        let (old, new) = match op {
            read::Operation::WasmLocal { index } => (
                index,
                match functions {
                    Some((input, output)) => (indices.local)(input, output, index),
                    None => Some(index),
                },
            ),
            read::Operation::WasmGlobal { index } => (index, (indices.global)(index)),
            read::Operation::Bra { target } | read::Operation::Skip { target } => {
                let end = (len - rest.len()) as isize;
                ops.push((offset, bytes.to_vec(), Some(end + isize::from(target))));
                continue;
            }
            _ => {
                ops.push((offset, bytes.to_vec(), None));
                continue;
            }
        };
        let Some(new) = new else {
            return Ok(None);
        };
        // The opcode and kind, then the index: a fixed 4 bytes for kind 3,
        // LEB128 otherwise.
        let mut out = bytes[..2].to_vec();
        if bytes[1] == 3 {
            out.extend_from_slice(&new.to_le_bytes());
        } else {
            write_uleb128(&mut out, new.into());
        }
        changed |= new != old;
        ops.push((offset, out, None));
    }
    if !changed {
        return Ok(Some(expression.clone()));
    }
    // The output offset of each operation, and of the end.
    let mut offsets = Vec::with_capacity(ops.len() + 1);
    let mut total = 0;
    for (_, bytes, _) in &ops {
        offsets.push(total);
        total += bytes.len();
    }
    offsets.push(total);
    let new_offset = |old: isize| -> Option<isize> {
        let old = usize::try_from(old).ok()?;
        let i = match ops.binary_search_by_key(&old, |op| op.0) {
            Ok(i) => i,
            Err(i) if i == ops.len() && old == len => i,
            // Not the start of an operation, so the input was invalid.
            Err(_) => return None,
        };
        Some(offsets[i] as isize)
    };
    let mut out = Vec::with_capacity(total);
    for (i, (_, bytes, target)) in ops.iter().enumerate() {
        let Some(target) = *target else {
            out.extend_from_slice(bytes);
            continue;
        };
        let relative = new_offset(target).map(|t| t - offsets[i + 1] as isize);
        let Some(relative) = relative.and_then(|r| i16::try_from(r).ok()) else {
            return Ok(None);
        };
        out.push(bytes[0]);
        out.extend_from_slice(&relative.to_le_bytes());
    }
    Ok(Some(read::Expression(EndianRcSlice::new(
        out.into(),
        LittleEndian,
    ))))
}

fn write_uleb128(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}
