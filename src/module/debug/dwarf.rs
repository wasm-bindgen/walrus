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
use std::rc::Rc;

use crate::{FunctionId, InstrLocId};
use gimli::write::{self, Address, ConvertLineSequenceEnd, ConvertResult};
use gimli::{constants, read, AttributeValue, EndianRcSlice, LineEncoding, LittleEndian, Reader};

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
    /// For each input function, the output functions its code is in: its
    /// own, if it was emitted, and any a pass copied its code to. Sorted.
    spread: Vec<Vec<u32>>,
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
        let mut spread = Vec::with_capacity(inputs.len());
        for (i, f) in inputs.iter().enumerate() {
            let o = output_of.get(&f.id).copied();
            let mut only_o = o.is_some_and(|o| sole[o] == Some(Some(i)));
            let mut outputs: Vec<u32> = o.map(|o| o as u32).into_iter().collect();
            let first: Vec<u32> = f
                .instrs
                .iter()
                .map(|&(_, loc)| {
                    let e = emitted.partition_point(|x| x.0 < loc);
                    for x in emitted[e..].iter().take_while(|x| x.0 == loc) {
                        only_o &= Some(x.1 as usize) == o;
                        if outputs.last() != Some(&x.1) {
                            outputs.push(x.1);
                        }
                    }
                    e as u32
                })
                .collect();
            outputs.sort_unstable();
            outputs.dedup();
            first_emission.push(first);
            whole.push(o.filter(|_| only_o));
            spread.push(outputs);
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
            spread,
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

    /// Like [`ranges`](Self::ranges), with the input function whose code
    /// each run is, since local indices are that function's.
    fn pieces(&self, begin: u64, end: u64) -> Vec<Piece> {
        let mut pieces = Vec::new();
        if is_tombstone(begin) || begin >= end {
            return pieces;
        }
        let first = self.inputs.partition_point(|f| f.end <= begin);
        for f in &self.inputs[first..] {
            if f.start >= end {
                break;
            }
            let runs = self.ranges(begin.max(f.start), end.min(f.end));
            pieces.extend(runs.into_iter().map(|run| (f.id, run)));
        }
        pieces.sort_unstable_by_key(|p| p.1 .1);
        pieces
    }

    /// The first input function `begin..end` covers code of.
    fn first_input(&self, begin: u64, end: u64) -> Option<FunctionId> {
        if is_tombstone(begin) || begin >= end {
            return None;
        }
        let f = self
            .inputs
            .get(self.inputs.partition_point(|f| f.end <= begin))?;
        (f.start < end).then_some(f.id)
    }
}

/// A run of emitted code with the input function it came from.
type Piece = (FunctionId, Run);

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
        if let Some(program) = &root.read_unit.line_program {
            check_line_program(program)?;
        }
        // Wasm has one operation per instruction and instructions of any
        // length, so line programs are written with one byte per
        // instruction and one operation each, whatever the input's header
        // said. Rows move to new addresses, so a header's minimum
        // instruction length or operation indices wouldn't fit them, and
        // gimli's writer asserts on both (and on a `line_base` and
        // `line_range` that can't encode an advance).
        let line_encoding = Some(LineEncoding::default());
        if let Some(program) = unit.read_line_program(None, line_encoding)? {
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
            && matches!(root_pc, Pc::Single(a) | Pc::Range(_, a, _) if a != 0);
        // Where each entry's expressions apply. Entries are read depth first,
        // so this holds the ancestors of the current entry.
        let mut scopes: Vec<(write::UnitEntryId, Scope)> = Vec::new();
        let root_id = unit.unit.root();
        let scope = entry_scope(&root, None, code)?;
        convert_attributes(&mut unit, root_id, &root, root_pc, &scope, &cx)?;
        scopes.push((root_id, scope));
        let mut entry = root;
        while let Some(id) = unit.read_entry(&mut entry)? {
            let id = unit.add_entry(id, &entry);
            while scopes.last().is_some_and(|s| Some(s.0) != entry.parent) {
                scopes.pop();
            }
            let parent = scopes.last().map(|s| &s.1);
            let pc = convert_pc(&entry, &cx)?;
            let scope = entry_scope(&entry, parent, code)?;
            convert_attributes(&mut unit, id, &entry, pc, &scope, &cx)?;
            scopes.push((id, scope));
        }
    }
    Ok(dwarf)
}

/// Line programs gimli would panic on, rather than return an error, are an
/// error instead: its writer asserts that a DWARF 2-4 file has a name, which
/// a header can't omit but a `DW_LNE_define_file` can, and its reader
/// negates a `DW_LNS_advance_line` operand, which overflows for `i64::MIN`.
fn check_line_program(program: &read::IncompleteLineProgram<R>) -> ConvertResult<()> {
    let header = program.header();
    let mut instructions = header.instructions();
    while let Some(instruction) = instructions.next_instruction(header)? {
        match instruction {
            read::LineInstruction::DefineFile(file)
                if header.version() <= 4
                    && matches!(file.path_name(), AttributeValue::String(name) if name.is_empty()) =>
            {
                return Err(read::Error::MissingFileEntryFormatPath.into());
            }
            read::LineInstruction::AdvanceLine(i64::MIN) => {
                return Err(read::Error::BadSignedLeb128.into());
            }
            _ => {}
        }
    }
    Ok(())
}

/// One sequence of the input line program.
struct Sequence {
    start: u64,
    end: u64,
    /// Rows with their addresses, sorted.
    rows: Vec<(u64, write::LineRow)>,
}

/// Each output function gets sequences from every unit whose line program
/// covers some of its code (usually just the unit its own input code
/// belonged to, but a pass can copy code between functions), with a row
/// wherever the input row covering its instructions changes. Instructions
/// with no input location continue the row before them, and a sequence
/// ends where the unit's coverage does.
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

    // The output functions with code from the input functions the unit's
    // sequences cover, in address order.
    let mut targets = Vec::new();
    for sequence in &sequences {
        let first = code.inputs.partition_point(|f| f.end <= sequence.start);
        for (i, f) in code.inputs.iter().enumerate().skip(first) {
            if f.start >= sequence.end {
                break;
            }
            targets.extend(code.spread[i].iter().map(|&o| o as usize));
        }
    }
    targets.sort_unstable();
    targets.dedup();

    for o in targets {
        let f = &code.outputs[o];
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
                row.op_index = 0;
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

/// Where an entry's expressions apply: its own code, as its `DW_AT_ranges`
/// or its `DW_AT_low_pc` and `DW_AT_high_pc` give it, or else its parent's.
fn entry_scope(
    entry: &write::ConvertUnitEntry<'_, R>,
    parent: Option<&Scope>,
    code: &CodeMap,
) -> ConvertResult<Scope> {
    let read_unit = entry.read_unit;
    let parent_function = parent.and_then(|p| p.function);
    let parent_pieces = || parent.map_or_else(|| Rc::from([]), |p| p.pieces.clone());
    let ranges = match entry.attr_value(constants::DW_AT_ranges) {
        Some(AttributeValue::RangeListsRef(offset)) => {
            Some(read_unit.ranges_offset_from_raw(offset))
        }
        Some(AttributeValue::DebugRngListsIndex(index)) => Some(read_unit.ranges_offset(index)?),
        _ => None,
    };
    let mut input = Vec::new();
    if let Some(offset) = ranges {
        // Resolved entries: absolute addresses, tombstones skipped. (A
        // `DW_AT_low_pc` alongside is the base address, not code.)
        let mut ranges = read_unit.ranges(offset)?;
        while let Some(range) = ranges.next()? {
            input.push((range.begin, range.end));
        }
    } else {
        let low = match entry.attr_value(constants::DW_AT_low_pc) {
            Some(value) => read_unit.attr_address(value)?,
            None => None,
        };
        let Some(low) = low else {
            return Ok(Scope {
                function: parent_function,
                pieces: parent_pieces(),
            });
        };
        let high = match entry.attr_value(constants::DW_AT_high_pc) {
            None => None,
            Some(AttributeValue::Udata(size)) => Some(low.saturating_add(size)),
            Some(value) => Some(read_unit.attr_address(value)?.unwrap_or(low)),
        };
        if !is_tombstone(low) {
            input.push((low, high.unwrap_or(low)));
        }
        if high.is_none() {
            // A single address: its function, with its parent's code.
            return Ok(Scope {
                function: input.first().and_then(|&(low, _)| {
                    let f = code.input_function(low)?;
                    (low < f.end).then_some(f.id)
                }),
                pieces: if input.is_empty() {
                    Rc::from([])
                } else {
                    parent_pieces()
                },
            });
        }
    }
    input.sort_unstable();
    let function = input
        .iter()
        .find_map(|&(b, e)| code.first_input(b, e))
        .or_else(|| {
            // An empty range: the function it's at the start of.
            let &(low, _) = input.first()?;
            let f = code.input_function(low)?;
            (low < f.end).then_some(f.id)
        });
    let mut pieces: Vec<Piece> = input.iter().flat_map(|&(b, e)| code.pieces(b, e)).collect();
    pieces.sort_unstable_by_key(|p| p.1 .1);
    Ok(Scope {
        function,
        pieces: pieces.into(),
    })
}

/// Where an entry's expressions apply. Code a linker discarded has neither
/// a function nor pieces, and its expressions are left as they are.
struct Scope {
    /// The input function whose local indices they use, where no piece
    /// says otherwise: the first the entry's code is in.
    function: Option<FunctionId>,
    /// The emitted code the entry or its closest ancestor with code covers.
    pieces: Rc<[Piece]>,
}

impl Scope {
    /// The input functions whose local indices its expressions use, with
    /// the output function the code is now in: `function`'s own first, if
    /// it was emitted.
    fn keys(&self, code: &CodeMap) -> Vec<(FunctionId, usize)> {
        let mut keys: Vec<_> = self
            .function
            .and_then(|f| Some((f, *code.output_of.get(&f)?)))
            .into_iter()
            .collect();
        for &(f, (o, _, _)) in self.pieces.iter() {
            if !keys.contains(&(f, o)) {
                keys.push((f, o));
            }
        }
        keys
    }
}

/// An entry's converted `DW_AT_low_pc` and `DW_AT_high_pc`.
#[derive(Clone, PartialEq)]
enum Pc {
    /// Neither, or left as they were.
    Unchanged,
    /// `DW_AT_low_pc` alone.
    Single(u64),
    /// One range: its output function, its start, and `DW_AT_high_pc` with
    /// whether it's a size.
    Range(usize, u64, (u64, bool)),
    /// The code is now in several places, so the pair becomes `DW_AT_ranges`.
    Ranges(Vec<Run>),
    /// All of the code was removed. Has whether `DW_AT_high_pc` is a size.
    Removed(bool),
}

fn convert_pc(entry: &write::ConvertUnitEntry<'_, R>, cx: &Context) -> ConvertResult<Pc> {
    let read_unit = entry.read_unit;
    let Some(low) = entry.attr_value(constants::DW_AT_low_pc) else {
        return Ok(Pc::Unchanged);
    };
    let high_pc = entry.attr_value(constants::DW_AT_high_pc);
    let high_is_size = matches!(high_pc, Some(AttributeValue::Udata(_)));
    let Some(low) = read_unit.attr_address(low)? else {
        // Not an address, so where its code is isn't known.
        return Ok(match high_pc {
            None => Pc::Single(DEAD_CODE),
            Some(_) => Pc::Removed(high_is_size),
        });
    };
    if is_tombstone(low) {
        // Discarded code. Its size or a tombstone end can stay, but another
        // end address would be mapped as if it were the end of live code,
        // and an end that's neither would be copied as it is.
        let keep = match high_pc {
            None => true,
            Some(_) if high_is_size => true,
            Some(value) => read_unit.attr_address(value)?.is_some_and(is_tombstone),
        };
        return Ok(if keep {
            Pc::Unchanged
        } else {
            Pc::Removed(false)
        });
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
            let (o, s, e) = runs.pop().unwrap();
            Pc::Range(o, s, (if is_size { e - s } else { e }, is_size))
        }
        _ => Pc::Ranges(runs),
    })
}

fn convert_attributes(
    unit: &mut write::ConvertUnit<'_, R>,
    id: write::UnitEntryId,
    entry: &write::ConvertUnitEntry<'_, R>,
    pc: Pc,
    scope: &Scope,
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
        Pc::Range(_, low, (high, is_size)) => {
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
        let value = match attr.value() {
            // Not supported by gimli, and safe to drop.
            _ if name == constants::DW_AT_GNU_locviews => continue,
            // gimli can't write macro sections, so these offsets would point
            // at nothing.
            AttributeValue::DebugMacinfoRef(_) | AttributeValue::DebugMacroRef(_) => continue,
            _ if name == constants::DW_AT_GNU_macros => continue,
            AttributeValue::LocationListsRef(offset) => {
                convert_location_list(unit, read_unit, offset, scope, cx)?
            }
            AttributeValue::DebugLocListsIndex(index) => {
                let offset = read_unit.locations_offset(index)?;
                convert_location_list(unit, read_unit, offset, scope, cx)?
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
                match convert_exprloc(unit, read_unit, name, &expression, scope, cx)? {
                    Some(value) => value,
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
    scope: &Scope,
    cx: &Context,
) -> ConvertResult<write::AttributeValue> {
    let encoding = read_unit.encoding();
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.locations(offset)?;
    while let Some(entry) = from.next()? {
        // gimli resolves `DW_LLE_default_location` to the whole address
        // space, which no other entry can cover with 4-byte addresses. It
        // applies wherever no other entry does, so it needs no mapping, but
        // it's one expression for all of the scope's code: it's kept only if
        // it names the same locals in every function that code is now in.
        if entry.range.begin == 0 && entry.range.end == u64::MAX {
            let (_, remapped) = remap_for_scope(&entry.data, encoding, scope, cx)?;
            if let Some(data) = agreed(remapped) {
                let data = unit.convert_expression(read_unit, data, &data_address)?;
                list.push(write::Location::DefaultLocation { data });
            }
            continue;
        }
        // The expression for each input function's code in each output
        // function, as local indices differ.
        let mut converted: HashMap<(FunctionId, usize), Option<write::Expression>> = HashMap::new();
        for (input, (o, begin, end)) in cx.code.pieces(entry.range.begin, entry.range.end) {
            let data = match converted.get(&(input, o)) {
                Some(data) => data.clone(),
                None => {
                    let functions = Some((input, cx.code.outputs[o].id));
                    let remapped = remap_wasm_locations(&entry.data, encoding, functions, cx)?;
                    let data = match remapped {
                        Some(e) => Some(unit.convert_expression(read_unit, e, &data_address)?),
                        None => None,
                    };
                    converted.insert((input, o), data.clone());
                    data
                }
            };
            let Some(data) = data else {
                continue;
            };
            push_location(&mut list, begin, end, data, cx);
        }
    }
    let id = unit.unit.locations.add(write::LocationList(list));
    Ok(write::AttributeValue::LocationListRef(id))
}

/// An expression remapped for each function it's for, `None` where it names
/// a local or global walrus removed.
type Remapped = Vec<Option<read::Expression<R>>>;

/// `expression` remapped for each of `scope`'s [keys](Scope::keys), which
/// are returned with them. With no keys (it names no locals, or the scope's
/// code isn't known or wasn't emitted), it's remapped once, for `scope`'s
/// function.
fn remap_for_scope(
    expression: &read::Expression<R>,
    encoding: gimli::Encoding,
    scope: &Scope,
    cx: &Context,
) -> ConvertResult<(Vec<(FunctionId, usize)>, Remapped)> {
    // Only local indices depend on the function. A unit's scope can cover
    // every function, so don't remap the rest once for each.
    let keys = if names_locals(expression, encoding)? {
        scope.keys(cx.code)
    } else {
        Vec::new()
    };
    let mut remapped = Vec::with_capacity(keys.len().max(1));
    for &(input, o) in &keys {
        let functions = Some((input, cx.code.outputs[o].id));
        remapped.push(remap_wasm_locations(expression, encoding, functions, cx)?);
    }
    if keys.is_empty() {
        let functions = scope.function.map(|f| (f, f));
        remapped.push(remap_wasm_locations(expression, encoding, functions, cx)?);
    }
    Ok((keys, remapped))
}

fn names_locals(
    expression: &read::Expression<R>,
    encoding: gimli::Encoding,
) -> ConvertResult<bool> {
    let mut ops = expression.clone().operations(encoding);
    while let Some(op) = ops.next()? {
        if matches!(op, read::Operation::WasmLocal { .. }) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The expression, if every remapping of it is the same.
fn agreed(remapped: Remapped) -> Option<read::Expression<R>> {
    let mut remapped = remapped.into_iter();
    let first = remapped.next()??;
    let same = |e: Option<read::Expression<R>>| e.is_some_and(|e| e.0.bytes() == first.0.bytes());
    remapped.all(same).then_some(first)
}

fn push_location(
    list: &mut Vec<write::Location>,
    begin: u64,
    end: u64,
    data: write::Expression,
    cx: &Context,
) {
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

/// Attributes whose value can be a location list as well as an expression.
fn can_be_location_list(name: constants::DwAt) -> bool {
    matches!(
        name,
        constants::DW_AT_location
            | constants::DW_AT_frame_base
            | constants::DW_AT_return_addr
            | constants::DW_AT_static_link
            | constants::DW_AT_string_length
            | constants::DW_AT_segment
    )
}

/// An expression applies to the code of its scope, using the local indices
/// of the input function each part of that code came from. Where that code
/// is now in functions that number those locals differently (a pass copied
/// it, or the scope spans several functions), it becomes a location list
/// with an expression for each, if the attribute can be one.
fn convert_exprloc(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    name: constants::DwAt,
    expression: &read::Expression<R>,
    scope: &Scope,
    cx: &Context,
) -> ConvertResult<Option<write::AttributeValue>> {
    let (keys, remapped) = remap_for_scope(expression, read_unit.encoding(), scope, cx)?;
    let bytes = |e: &Option<read::Expression<R>>| e.as_ref().map(|e| e.0.bytes().to_vec());
    let same = remapped.windows(2).all(|w| bytes(&w[0]) == bytes(&w[1]));
    if same || !can_be_location_list(name) {
        return Ok(match remapped.into_iter().next().flatten() {
            Some(e) => Some(write::AttributeValue::Exprloc(unit.convert_expression(
                read_unit,
                e,
                &data_address,
            )?)),
            None => None,
        });
    }
    let mut converted = HashMap::new();
    let mut list = Vec::new();
    for &(input, (o, begin, end)) in scope.pieces.iter() {
        let data = match converted.get(&(input, o)) {
            Some(data) => Option::clone(data),
            None => {
                let i = keys.iter().position(|&k| k == (input, o)).unwrap();
                let data = match remapped[i].clone() {
                    Some(e) => Some(unit.convert_expression(read_unit, e, &data_address)?),
                    None => None,
                };
                converted.insert((input, o), data.clone());
                data
            }
        };
        if let Some(data) = data {
            push_location(&mut list, begin, end, data, cx);
        }
    }
    if list.is_empty() {
        return Ok(None);
    }
    let id = unit.unit.locations.add(write::LocationList(list));
    Ok(Some(write::AttributeValue::LocationListRef(id)))
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
