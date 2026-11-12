//! Convert the input module's DWARF to match the emitted code.
//!
//! walrus re-encodes every instruction and reorders functions, so code
//! addresses don't move by a constant. `gimli::write::Dwarf::from` only
//! suits a constant shift: it passes the offsets in range lists, location
//! lists, line programs and `DW_AT_high_pc` through unchanged, or through
//! the address callback as if they were addresses. So this drives
//! `gimli::write::Dwarf::convert` itself, and maps every range from its
//! absolute start and end.
//!
//! walrus also renumbers locals (it groups them by type and drops unused
//! ones) and may remove globals, so the indices in `DW_OP_WASM_location`
//! are remapped too.

use std::collections::HashMap;

use crate::FunctionId;
use gimli::write::{self, Address, ConvertLineSequenceEnd, ConvertResult};
use gimli::{constants, read, AttributeValue, EndianRcSlice, LittleEndian, Reader};

/// An owned reader, so that an expression with remapped indices can be
/// converted by gimli like any other.
pub(crate) type R = EndianRcSlice<LittleEndian>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AddressSearchPreference {
    /// Normal range comparison (inclusive start, exclusive end)
    ExclusiveFunctionEnd,
    /// Prefer treating a beginning point as the ending point of the previous function.
    InclusiveFunctionEnd,
}

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

/// Where code moved, in code-section-relative addresses.
pub(crate) struct Addresses<'a> {
    /// Maps an input address to its output address.
    pub map: &'a dyn Fn(u64, AddressSearchPreference) -> Option<u64>,
    /// Input function bodies, sorted, with the functions they became.
    pub input_bodies: Vec<(u64, u64, FunctionId)>,
    /// Output function bodies, sorted.
    pub output_bodies: Vec<(u64, u64)>,
}

impl Addresses<'_> {
    /// A single address, such as `DW_AT_low_pc` or `DW_OP_addr`.
    fn address(&self, address: u64) -> Option<Address> {
        if is_tombstone(address) {
            return Some(Address::Constant(address));
        }
        let address = (self.map)(address, AddressSearchPreference::InclusiveFunctionEnd);
        Some(Address::Constant(address.unwrap_or(DEAD_CODE)))
    }

    /// The output ranges covering what `begin..end` covered. A range over
    /// several functions becomes one range per function, because functions
    /// are reordered. Parts in code that no longer exists are dropped.
    fn ranges(&self, begin: u64, end: u64) -> Vec<(u64, u64)> {
        let mut out = Vec::new();
        if is_tombstone(begin) || begin >= end {
            return out;
        }
        let first = self.input_bodies.partition_point(|&(_, e, _)| e <= begin);
        for &(body_start, body_end, _) in &self.input_bodies[first..] {
            if body_start >= end {
                break;
            }
            let (b, e) = (begin.max(body_start), end.min(body_end));
            let b = (self.map)(b, AddressSearchPreference::ExclusiveFunctionEnd);
            let e = (self.map)(e, AddressSearchPreference::InclusiveFunctionEnd);
            if let (Some(b), Some(e)) = (b, e) {
                if b < e {
                    out.push((b, e));
                }
            }
        }
        out
    }

    /// The function whose input body contains `address`. A body starts
    /// after its size, but LLVM's `DW_AT_low_pc` for a function is the
    /// address of that size, so it counts too.
    fn input_function(&self, address: u64) -> Option<FunctionId> {
        if is_tombstone(address) {
            return None;
        }
        let i = self.input_bodies.partition_point(|&(_, e, _)| e <= address);
        let (start, end, id) = *self.input_bodies.get(i)?;
        let size_len = gimli::leb128::write::unsigned(&mut std::io::sink(), end - start)
            .expect("writes to a sink") as u64;
        (address + size_len >= start).then_some(id)
    }

    /// The output function body containing `address`, including its end.
    fn output_body(&self, address: u64) -> Option<(u64, u64)> {
        let i = self.output_bodies.partition_point(|&(s, _)| s <= address);
        let body = *self.output_bodies.get(i.checked_sub(1)?)?;
        (address <= body.1).then_some(body)
    }
}

/// New indices of the input module's locals and globals, or `None` for ones
/// that are no longer emitted.
pub(crate) struct Indices<'a> {
    /// Takes a function and its input local index.
    pub local: &'a dyn Fn(FunctionId, u32) -> Option<u32>,
    pub global: &'a dyn Fn(u32) -> Option<u32>,
}

/// `indices` is `None` when the input indices weren't recorded, in which
/// case `DW_OP_WASM_location` is copied unchanged.
pub(crate) fn convert(
    from: &read::Dwarf<R>,
    addresses: &Addresses,
    indices: Option<&Indices>,
) -> ConvertResult<write::Dwarf> {
    let mut dwarf = write::Dwarf::default();
    let mut units = dwarf.convert(from)?;
    while let Some((mut unit, root)) = units.read_unit()? {
        if let Some(program) = unit.read_line_program(None, None)? {
            let (program, files) = convert_line_program(program, addresses)?;
            unit.set_line_program(program, files);
        }
        // In DWARF 2-4, range and location list entries are relative to the
        // unit's `DW_AT_low_pc` unless a list sets its own base.
        let low_pc = match root.attr_value(constants::DW_AT_low_pc) {
            Some(value) => root.read_unit.attr_address(value)?,
            None => None,
        };
        // gimli's writer treats the unit as having a base address when its
        // converted `DW_AT_low_pc` is nonzero.
        let cx = Context {
            addresses,
            indices,
            relative_to_unit: root.read_unit.encoding().version <= 4
                && low_pc.is_some_and(|a| addresses.address(a) != Some(Address::Constant(0))),
        };
        // The function each entry is in, for the local indices in its
        // expressions: the one its own `DW_AT_low_pc` is in, or else its
        // parent's.
        let mut functions = HashMap::new();
        let root_id = unit.unit.root();
        let function = entry_function(&root, None, addresses)?;
        functions.insert(root_id, function);
        convert_attributes(&mut unit, root_id, &root, function, &cx)?;
        let mut entry = root;
        while let Some(id) = unit.read_entry(&mut entry)? {
            let id = unit.add_entry(id, &entry);
            let parent = entry
                .parent
                .and_then(|p| functions.get(&p).copied().flatten());
            let function = entry_function(&entry, parent, addresses)?;
            functions.insert(id, function);
            convert_attributes(&mut unit, id, &entry, function, &cx)?;
        }
    }
    Ok(dwarf)
}

/// Rows are moved to their instructions' new addresses. Each output function
/// gets its own sequence, sorted by address, because functions are reordered
/// and rows must not go backwards within a sequence. Rows for instructions
/// that no longer exist are dropped.
fn convert_line_program(
    mut program: write::ConvertLineProgram<'_, R>,
    addresses: &Addresses,
) -> ConvertResult<(write::LineProgram, Vec<write::FileId>)> {
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
        let end = (addresses.map)(end, AddressSearchPreference::InclusiveFunctionEnd);
        let mut rows = Vec::with_capacity(sequence.rows.len());
        for row in sequence.rows {
            let address = start + row.address_offset;
            let Some(address) =
                (addresses.map)(address, AddressSearchPreference::InclusiveFunctionEnd)
            else {
                continue;
            };
            let Some(body) = addresses.output_body(address) else {
                continue;
            };
            rows.push((body, address, row));
        }
        // Stable, so rows at the same address keep their order.
        rows.sort_by_key(|&(body, address, _)| (body.0, address));
        for group in rows.chunk_by(|a, b| a.0 == b.0) {
            let (body, base, _) = group[0];
            program.begin_sequence(Some(Address::Constant(base)));
            for &(_, address, mut row) in group {
                row.address_offset = address - base;
                program.generate_row(row);
            }
            // The sequence ends where it did, if that is still in this
            // function and after its rows, or else at the function's end.
            let last = group[group.len() - 1].1;
            let end = match end {
                Some(end) if end > last && addresses.output_body(end) == Some(body) => end,
                _ => body.1.max(last + 1),
            };
            program.end_sequence(end - base);
        }
    }
    Ok(program.program())
}

fn entry_function(
    entry: &write::ConvertUnitEntry<'_, R>,
    parent: Option<FunctionId>,
    addresses: &Addresses,
) -> ConvertResult<Option<FunctionId>> {
    let low_pc = match entry.attr_value(constants::DW_AT_low_pc) {
        Some(value) => entry.read_unit.attr_address(value)?,
        None => None,
    };
    Ok(match low_pc {
        Some(low_pc) => addresses.input_function(low_pc),
        None => parent,
    })
}

/// How to convert the entries of one unit.
struct Context<'a, 'b> {
    addresses: &'a Addresses<'b>,
    indices: Option<&'a Indices<'b>>,
    /// The unit has a nonzero base address, which DWARF 2-4 list entries
    /// are relative to. Lists then start with a base address entry of 0, and
    /// hold offsets from it.
    relative_to_unit: bool,
}

fn convert_attributes(
    unit: &mut write::ConvertUnit<'_, R>,
    id: write::UnitEntryId,
    entry: &write::ConvertUnitEntry<'_, R>,
    function: Option<FunctionId>,
    cx: &Context,
) -> ConvertResult<()> {
    let addresses = cx.addresses;
    let address = |a| addresses.address(a);
    for attr in &entry.attrs {
        let read_unit = entry.read_unit;
        let value = match attr.value() {
            // Not supported by gimli, and safe to drop.
            _ if attr.name() == constants::DW_AT_GNU_locviews => continue,
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
            AttributeValue::Udata(size) if attr.name() == constants::DW_AT_high_pc => {
                convert_high_pc(entry, size, addresses)?
            }
            AttributeValue::Exprloc(expression) => {
                match remap_wasm_locations(&expression, read_unit.encoding(), function, cx)? {
                    Some(expression) => write::AttributeValue::Exprloc(
                        unit.convert_expression(read_unit, expression, &address)?,
                    ),
                    // The variable is now optimized out.
                    None => continue,
                }
            }
            _ => unit.convert_attribute_value(read_unit, attr, &address)?,
        };
        unit.unit.get_mut(id).set(attr.name(), value);
    }
    Ok(())
}

/// `DW_AT_high_pc` as a size: the distance between the new `low_pc` and the
/// new address of `low_pc + size`.
fn convert_high_pc(
    entry: &write::ConvertUnitEntry<'_, R>,
    size: u64,
    addresses: &Addresses,
) -> ConvertResult<write::AttributeValue> {
    let low = match entry.attr_value(constants::DW_AT_low_pc) {
        Some(value) => entry.read_unit.attr_address(value)?,
        None => None,
    };
    let size = match low {
        Some(low) if !is_tombstone(low) => {
            let map = |a| (addresses.map)(a, AddressSearchPreference::InclusiveFunctionEnd);
            match (map(low), map(low + size)) {
                (Some(new_low), Some(new_high)) => new_high.saturating_sub(new_low),
                _ => size,
            }
        }
        _ => size,
    };
    // Keep the fixed-width form LLVM emits: consumers that patch code
    // addresses in place (binaryen's DWARF update) rely on the size not
    // changing.
    Ok(match u32::try_from(size) {
        Ok(size) => write::AttributeValue::Data4(size),
        Err(_) => write::AttributeValue::Udata(size),
    })
}

fn convert_location_list(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    offset: gimli::LocationListsOffset,
    function: Option<FunctionId>,
    cx: &Context,
) -> ConvertResult<write::AttributeValue> {
    let address = |a| cx.addresses.address(a);
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.locations(offset)?;
    while let Some(entry) = from.next()? {
        // gimli resolves `DW_LLE_default_location` to the whole address
        // space, which no other entry can cover with 4-byte addresses. It
        // applies wherever no other entry does, so it needs no mapping, and
        // its locals are those of the entry's function.
        if entry.range.begin == 0 && entry.range.end == u64::MAX {
            if let Some(data) =
                remap_wasm_locations(&entry.data, read_unit.encoding(), function, cx)?
            {
                let data = unit.convert_expression(read_unit, data, &address)?;
                list.push(write::Location::DefaultLocation { data });
            }
            continue;
        }
        let ranges = cx.addresses.ranges(entry.range.begin, entry.range.end);
        if ranges.is_empty() {
            continue;
        }
        let input = cx.addresses.input_function(entry.range.begin);
        let Some(data) = remap_wasm_locations(&entry.data, read_unit.encoding(), input, cx)? else {
            continue;
        };
        let data = unit.convert_expression(read_unit, data, &address)?;
        for (begin, end) in ranges {
            if cx.relative_to_unit && list.is_empty() {
                list.push(write::Location::BaseAddress {
                    address: Address::Constant(0),
                });
            }
            let data = data.clone();
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
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.ranges(offset)?;
    while let Some(range) = from.next()? {
        for (begin, end) in cx.addresses.ranges(range.begin, range.end) {
            if cx.relative_to_unit && list.is_empty() {
                list.push(write::Range::BaseAddress {
                    address: Address::Constant(0),
                });
            }
            list.push(if cx.relative_to_unit {
                write::Range::OffsetPair { begin, end }
            } else {
                write::Range::StartEnd {
                    begin: Address::Constant(begin),
                    end: Address::Constant(end),
                }
            });
        }
    }
    let id = unit.unit.ranges.add(write::RangeList(list));
    Ok(write::AttributeValue::RangeListRef(id))
}

/// Remaps the local and global indices in an expression's
/// `DW_OP_WASM_location` operations, or returns `None` if it names a local
/// or global that walrus removed. Local indices are those of `function`.
///
/// A new index is written in as many bytes as the old one when it fits, so
/// branch offsets stay valid. An expression with a branch over an operation
/// that had to grow is dropped rather than left wrong.
fn remap_wasm_locations(
    expression: &read::Expression<R>,
    encoding: gimli::Encoding,
    function: Option<FunctionId>,
    cx: &Context,
) -> ConvertResult<Option<read::Expression<R>>> {
    let Some(indices) = cx.indices else {
        return Ok(Some(expression.clone()));
    };
    let mut out = Vec::new();
    let (mut changed, mut resized, mut branches) = (false, false, false);
    let mut rest = expression.0.clone();
    while !rest.is_empty() {
        let mut bytes = rest.clone();
        let op = read::Operation::parse(&mut rest, encoding)?;
        bytes.truncate(bytes.len() - rest.len())?;
        let bytes = bytes.to_slice()?;
        let (old, new) = match op {
            // Without a function, the entry describes code that isn't in
            // the module (such as a function the linker discarded), so it
            // is left as it is.
            read::Operation::WasmLocal { index } => (
                index,
                function.map_or(Some(index), |f| (indices.local)(f, index)),
            ),
            read::Operation::WasmGlobal { index } => (index, (indices.global)(index)),
            op => {
                branches |= matches!(
                    op,
                    read::Operation::Bra { .. } | read::Operation::Skip { .. }
                );
                out.extend_from_slice(&bytes);
                continue;
            }
        };
        let Some(new) = new else {
            return Ok(None);
        };
        // The opcode and kind, then the index: a fixed 4 bytes for kind 3,
        // LEB128 otherwise.
        out.extend_from_slice(&bytes[..2]);
        if bytes[1] == 3 {
            out.extend_from_slice(&new.to_le_bytes());
        } else {
            let len = write_uleb128_padded(&mut out, new.into(), bytes.len() - 2);
            resized |= len != bytes.len() - 2;
        }
        changed |= new != old;
    }
    if !changed {
        return Ok(Some(expression.clone()));
    }
    if branches && resized {
        return Ok(None);
    }
    Ok(Some(read::Expression(EndianRcSlice::new(
        out.into(),
        LittleEndian,
    ))))
}

/// Writes `value` as LEB128 in at least `min_len` bytes, and returns the
/// number of bytes written.
fn write_uleb128_padded(out: &mut Vec<u8>, mut value: u64, min_len: usize) -> usize {
    let mut len = 0;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        len += 1;
        if value == 0 && len >= min_len {
            out.push(byte);
            return len;
        }
        out.push(byte | 0x80);
    }
}
