//! Convert the input module's DWARF to match the emitted code.
//!
//! walrus re-encodes every instruction and reorders functions, so code
//! addresses don't move by a constant. `gimli::write::Dwarf::from` only
//! suits a constant shift: it passes the offsets in range lists, location
//! lists, line programs and `DW_AT_high_pc` through unchanged, or through
//! the address callback as if they were addresses. So this drives
//! `gimli::write::Dwarf::convert` itself, and maps every range from its
//! absolute start and end.

use gimli::write::{self, Address, ConvertLineSequenceEnd, ConvertResult};
use gimli::{constants, read, AttributeValue, Reader};

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
    /// Input function bodies, sorted.
    pub input_bodies: Vec<(u64, u64)>,
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
        let first = self.input_bodies.partition_point(|&(_, e)| e <= begin);
        for &(body_start, body_end) in &self.input_bodies[first..] {
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

    /// The output function body containing `address`, including its end.
    fn output_body(&self, address: u64) -> Option<(u64, u64)> {
        let i = self.output_bodies.partition_point(|&(s, _)| s <= address);
        let body = *self.output_bodies.get(i.checked_sub(1)?)?;
        (address <= body.1).then_some(body)
    }
}

pub(crate) fn convert<R: Reader<Offset = usize>>(
    from: &read::Dwarf<R>,
    addresses: &Addresses,
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
        let lists = Lists {
            addresses,
            relative_to_unit: root.read_unit.encoding().version <= 4
                && low_pc.is_some_and(|a| addresses.address(a) != Some(Address::Constant(0))),
        };
        let root_id = unit.unit.root();
        convert_attributes(&mut unit, root_id, &root, &lists)?;
        let mut entry = root;
        while let Some(id) = unit.read_entry(&mut entry)? {
            let id = unit.add_entry(id, &entry);
            convert_attributes(&mut unit, id, &entry, &lists)?;
        }
    }
    Ok(dwarf)
}

/// Rows are moved to their instructions' new addresses. Each output function
/// gets its own sequence, sorted by address, because functions are reordered
/// and rows must not go backwards within a sequence. Rows for instructions
/// that no longer exist are dropped.
fn convert_line_program<R: Reader<Offset = usize>>(
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

/// How to write the range and location lists of one unit.
struct Lists<'a, 'b> {
    addresses: &'a Addresses<'b>,
    /// The unit has a nonzero base address, which DWARF 2-4 list entries
    /// are relative to. Lists then start with a base address entry of 0, and
    /// hold offsets from it.
    relative_to_unit: bool,
}

fn convert_attributes<R: Reader<Offset = usize>>(
    unit: &mut write::ConvertUnit<'_, R>,
    id: write::UnitEntryId,
    entry: &write::ConvertUnitEntry<'_, R>,
    lists: &Lists,
) -> ConvertResult<()> {
    let addresses = lists.addresses;
    let address = |a| addresses.address(a);
    for attr in &entry.attrs {
        let read_unit = entry.read_unit;
        let value = match attr.value() {
            // Not supported by gimli, and safe to drop.
            _ if attr.name() == constants::DW_AT_GNU_locviews => continue,
            AttributeValue::LocationListsRef(offset) => {
                convert_location_list(unit, read_unit, offset, lists)?
            }
            AttributeValue::DebugLocListsIndex(index) => {
                let offset = read_unit.locations_offset(index)?;
                convert_location_list(unit, read_unit, offset, lists)?
            }
            AttributeValue::RangeListsRef(offset) => {
                let offset = read_unit.ranges_offset_from_raw(offset);
                convert_range_list(unit, read_unit, offset, lists)?
            }
            AttributeValue::DebugRngListsIndex(index) => {
                let offset = read_unit.ranges_offset(index)?;
                convert_range_list(unit, read_unit, offset, lists)?
            }
            AttributeValue::Udata(size) if attr.name() == constants::DW_AT_high_pc => {
                convert_high_pc(entry, size, addresses)?
            }
            _ => unit.convert_attribute_value(read_unit, attr, &address)?,
        };
        unit.unit.get_mut(id).set(attr.name(), value);
    }
    Ok(())
}

/// `DW_AT_high_pc` as a size: the distance between the new `low_pc` and the
/// new address of `low_pc + size`.
fn convert_high_pc<R: Reader<Offset = usize>>(
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

fn convert_location_list<R: Reader<Offset = usize>>(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    offset: gimli::LocationListsOffset,
    lists: &Lists,
) -> ConvertResult<write::AttributeValue> {
    let address = |a| lists.addresses.address(a);
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.locations(offset)?;
    while let Some(entry) = from.next()? {
        let ranges = lists.addresses.ranges(entry.range.begin, entry.range.end);
        if ranges.is_empty() {
            continue;
        }
        let data = unit.convert_expression(read_unit, entry.data, &address)?;
        for (begin, end) in ranges {
            if lists.relative_to_unit && list.is_empty() {
                list.push(write::Location::BaseAddress {
                    address: Address::Constant(0),
                });
            }
            let data = data.clone();
            list.push(if lists.relative_to_unit {
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

fn convert_range_list<R: Reader<Offset = usize>>(
    unit: &mut write::ConvertUnit<'_, R>,
    read_unit: read::UnitRef<'_, R>,
    offset: gimli::RangeListsOffset,
    lists: &Lists,
) -> ConvertResult<write::AttributeValue> {
    let mut list = Vec::new();
    // Resolved entries: absolute addresses, tombstones skipped.
    let mut from = read_unit.ranges(offset)?;
    while let Some(range) = from.next()? {
        for (begin, end) in lists.addresses.ranges(range.begin, range.end) {
            if lists.relative_to_unit && list.is_empty() {
                list.push(write::Range::BaseAddress {
                    address: Address::Constant(0),
                });
            }
            list.push(if lists.relative_to_unit {
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
