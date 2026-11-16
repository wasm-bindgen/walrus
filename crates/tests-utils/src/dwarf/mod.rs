//! Checks that walrus keeps a module's DWARF correct.
//!
//! [`check_preserved`] compares what a debugger would learn at each
//! instruction of the input module with what it learns at the same
//! instruction of walrus's output: the source line, the scopes (subprograms,
//! inlined subroutines and lexical blocks) covering it, and where each
//! variable is. Instructions are paired by walking both modules' functions
//! side by side, so the comparison doesn't rely on walrus's own record of
//! where it put each instruction.

use anyhow::{bail, ensure, Context, Result};
use gimli::{constants, read, EndianSlice, LittleEndian, Operation};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use wasmparser::{KnownCustom, Name, Operator, Parser, Payload, TypeRef};

mod synth;
pub use synth::synthesize;

type Reader<'a> = EndianSlice<'a, LittleEndian>;

/// Addresses linkers write for code they discarded, as walrus reads them.
fn is_tombstone(address: u64) -> bool {
    address == 0 || address >= 0xFFFF_FFFE
}

/// What [`check_preserved`] compared.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// Instructions whose debug info was compared.
    pub instructions: usize,
    /// Instructions that had a source line in the input.
    pub with_line: usize,
    /// Instructions covered by a scope in the input.
    pub in_scope: usize,
    /// Variable locations compared.
    pub locations: usize,
}

/// One instruction, at a code-section-relative address.
#[derive(Clone)]
struct Instr {
    address: u64,
    /// The operator's name, such as `LocalGet`.
    kind: String,
    local: Option<u32>,
    global: Option<u32>,
}

/// A function defined in the module.
struct Func {
    name: Option<String>,
    params: u32,
    /// Parameters and declared locals.
    locals: u32,
    /// The body, after its size.
    body: (u64, u64),
    /// The instructions walrus keeps, as [`normalize`] finds them.
    instrs: Vec<Instr>,
    /// All the instructions.
    raw: Vec<Instr>,
}

/// The code of a module and its custom sections.
struct Code<'a> {
    funcs: Vec<Func>,
    exports: HashMap<u32, String>,
    imported_funcs: u32,
    globals: u32,
    customs: Vec<(&'a str, &'a [u8])>,
}

fn operator_kind(op: &Operator) -> String {
    let debug = format!("{op:?}");
    debug
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn parse_code(wasm: &[u8]) -> Result<Code<'_>> {
    let mut type_params = Vec::new();
    let mut func_types = Vec::new();
    let mut imported_funcs = 0;
    let mut globals = 0;
    let mut names = HashMap::new();
    let mut exports = HashMap::new();
    let mut customs = Vec::new();
    let mut code_start = 0;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for ty in group?.types() {
                        type_params.push(match &ty.composite_type.inner {
                            wasmparser::CompositeInnerType::Func(f) => f.params().len() as u32,
                            _ => 0,
                        });
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    match import?.ty {
                        TypeRef::Func(t) | TypeRef::FuncExact(t) => {
                            func_types.push(t);
                            imported_funcs += 1;
                        }
                        TypeRef::Global(_) => globals += 1,
                        _ => {}
                    }
                }
            }
            Payload::GlobalSection(reader) => globals += reader.count(),
            Payload::FunctionSection(reader) => {
                for t in reader {
                    func_types.push(t?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export?;
                    if export.kind == wasmparser::ExternalKind::Func {
                        exports
                            .entry(export.index)
                            .or_insert(export.name.to_string());
                    }
                }
            }
            Payload::CodeSectionStart { range, .. } => code_start = range.start,
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                let range = (
                    (range.start - code_start) as u64,
                    (range.end - code_start) as u64,
                );
                let mut declared = 0;
                for local in body.get_locals_reader()? {
                    declared += local?.0;
                }
                let mut instrs = Vec::new();
                let mut ops = body.get_operators_reader()?;
                while !ops.eof() {
                    let (op, offset) = ops.read_with_offset()?;
                    let (local, global) = match op {
                        Operator::LocalGet { local_index }
                        | Operator::LocalSet { local_index }
                        | Operator::LocalTee { local_index } => (Some(local_index), None),
                        Operator::GlobalGet { global_index }
                        | Operator::GlobalSet { global_index } => (None, Some(global_index)),
                        _ => (None, None),
                    };
                    instrs.push(Instr {
                        address: (offset - code_start) as u64,
                        kind: operator_kind(&op),
                        local,
                        global,
                    });
                }
                bodies.push((range, declared, instrs));
            }
            Payload::CustomSection(section) => {
                if let KnownCustom::Name(reader) = section.as_known() {
                    for subsection in reader {
                        if let Ok(Name::Function(map)) = subsection {
                            for naming in map.into_iter().flatten() {
                                names.insert(naming.index, naming.name.to_string());
                            }
                        }
                    }
                }
                customs.push((section.name(), section.data()));
            }
            _ => {}
        }
    }
    let funcs = bodies
        .into_iter()
        .enumerate()
        .map(|(i, (body, declared, raw))| {
            let index = imported_funcs + i as u32;
            let params = func_types
                .get(index as usize)
                .and_then(|&t| type_params.get(t as usize))
                .copied()
                .unwrap_or(0);
            Func {
                name: names.get(&index).cloned(),
                params,
                locals: params + declared,
                body,
                instrs: normalize(&raw),
                raw,
            }
        })
        .collect();
    Ok(Code {
        funcs,
        exports: exports
            .into_iter()
            .filter(|&(i, _)| i >= imported_funcs)
            .map(|(i, n)| (i - imported_funcs, n))
            .collect(),
        imported_funcs,
        globals,
        customs,
    })
}

impl Code<'_> {
    /// The end of the body before function `i`'s, or 0.
    fn body_before(&self, i: usize) -> u64 {
        let start = self.funcs[i].body.0;
        self.funcs
            .iter()
            .map(|f| f.body.1)
            .filter(|&end| end <= start)
            .max()
            .unwrap_or(0)
    }
}

/// The instructions walrus keeps, so a function's instructions line up
/// before and after: it drops `nop`s, code after an unconditional branch,
/// `return` or `throw` up to the end of the block, and empty `else` arms.
fn normalize(instrs: &[Instr]) -> Vec<Instr> {
    let mut out: Vec<Instr> = Vec::with_capacity(instrs.len());
    // Unreachable code's depth of nested blocks, if in unreachable code.
    let mut unreachable: Option<u32> = None;
    for instr in instrs {
        let kind = instr.kind.as_str();
        if let Some(depth) = &mut unreachable {
            match kind {
                "Block" | "Loop" | "If" | "Try" | "TryTable" => *depth += 1,
                "End" | "Delegate" if *depth > 0 => *depth -= 1,
                "Else" | "Catch" | "CatchAll" if *depth > 0 => {}
                "End" | "Delegate" | "Else" | "Catch" | "CatchAll" => unreachable = None,
                _ => {}
            }
            if unreachable.is_some() {
                continue;
            }
        }
        match kind {
            "Nop" => continue,
            // walrus keeps the code after a `return_call`.
            "Unreachable" | "Br" | "BrTable" | "Return" | "Throw" | "ThrowRef" | "Rethrow" => {
                unreachable = Some(0)
            }
            _ => {}
        }
        out.push(instr.clone());
    }
    // An empty `else` arm isn't emitted.
    let mut i = 0;
    while i + 1 < out.len() {
        if out[i].kind == "Else" && out[i + 1].kind == "End" {
            out.remove(i);
        } else {
            i += 1;
        }
    }
    out
}

/// Pairs each output function with the input function it came from, by
/// name, then by export name. Returns `(input, output)` indices into
/// `Code::funcs`.
fn pair_functions(input: &Code, output: &Code) -> Result<Vec<(usize, usize)>> {
    fn keys(code: &Code) -> Vec<Option<String>> {
        let mut seen = HashMap::new();
        code.funcs
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let key = match (&f.name, code.exports.get(&(i as u32))) {
                    (Some(name), _) => format!("name {name}"),
                    (None, Some(export)) => format!("export {export}"),
                    (None, None) => return None,
                };
                // The k-th function with a name pairs with the k-th one.
                let n = seen.entry(key.clone()).or_insert(0);
                *n += 1;
                Some(format!("{key} #{n}"))
            })
            .collect()
    }
    let input_keys = keys(input);
    let output_keys = keys(output);
    let by_key: HashMap<&str, usize> = input_keys
        .iter()
        .enumerate()
        .filter_map(|(i, k)| Some((k.as_deref()?, i)))
        .collect();
    let mut pairs = Vec::new();
    for (o, key) in output_keys.iter().enumerate() {
        let Some(key) = key else {
            bail!(
                "output function {} has no name or export to pair it by",
                o as u32 + output.imported_funcs
            );
        };
        let Some(&i) = by_key.get(key.as_str()) else {
            bail!("output function `{key}` isn't in the input");
        };
        pairs.push((i, o));
    }
    Ok(pairs)
}

fn load_dwarf<'a>(customs: &[(&'a str, &'a [u8])]) -> Result<read::Dwarf<Reader<'a>>> {
    Ok(read::Dwarf::load(|id| {
        let data = customs
            .iter()
            .find(|(name, _)| *name == id.name())
            .map_or(&[][..], |(_, data)| *data);
        Ok::<_, gimli::Error>(EndianSlice::new(data, LittleEndian))
    })?)
}

/// A DIE, by its unit and the tags and names of it and the DIEs above it,
/// with a count to tell apart siblings with the same tag and name. walrus
/// keeps every DIE, but gimli moves base types to the start of the unit.
type DieId = (usize, String);

/// A line table row: file, line and column.
type Line = (String, u64, u64);

struct Sequence {
    start: u64,
    end: u64,
    rows: Vec<(u64, Line)>,
}

/// A location-class attribute (`DW_AT_location` or `DW_AT_frame_base`) of a
/// DIE.
struct Location {
    die: DieId,
    attr: constants::DwAt,
    /// The entries, or one entry covering everything for an expression.
    entries: Vec<(u64, u64, Expr)>,
    /// A location list, rather than an expression.
    list: bool,
    /// Where an expression means something: the ranges of this DIE, or else
    /// of the nearest DIE above it that gives ranges, which are empty for
    /// discarded code. `None` if no DIE gives ranges.
    scope: Option<Scope>,
}

type Scope = Vec<(u64, u64)>;

/// An expression in effect, with its scope if it's a location list's
/// default entry.
type Located<'a> = (&'a Expr, Option<&'a Scope>);

/// A decoded expression.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Expr(Vec<Op>);

#[derive(Clone, PartialEq, Eq, Debug)]
enum Op {
    Local(u32),
    Global(u32),
    /// Any other operation, printed, with DIE references as DIE positions
    /// and branch targets as operation positions.
    Other(String),
}

/// The debug info of a module, in a form that can be queried by address.
struct Info {
    /// Each unit's DIEs, with the tags and names of their children in order,
    /// apart from base types.
    shapes: Vec<BTreeMap<String, Vec<String>>>,
    sequences: Vec<Sequence>,
    scopes: Vec<(DieId, Vec<(u64, u64)>)>,
    locations: Vec<Location>,
}

fn attr_string<'a>(
    dwarf: &read::Dwarf<Reader<'a>>,
    unit: &read::Unit<Reader<'a>>,
    value: read::AttributeValue<Reader<'a>>,
) -> Result<String> {
    Ok(dwarf
        .attr_string(unit, value)?
        .to_string_lossy()
        .into_owned())
}

fn decode_expr(
    expr: read::Expression<Reader<'_>>,
    encoding: gimli::Encoding,
    die_index: &HashMap<usize, String>,
    address: &dyn Fn(gimli::DebugAddrIndex<usize>) -> Result<u64>,
) -> Result<Expr> {
    let start = expr.0;
    let mut bytes = expr.0;
    let mut ops = Vec::new();
    // Each operation's byte offset, and where the bytes after it start.
    let mut offsets = Vec::new();
    while !bytes.is_empty() {
        let at = start.len() - bytes.len();
        let op = Operation::parse(&mut bytes, encoding)?;
        offsets.push(at);
        ops.push((op, start.len() - bytes.len()));
    }
    offsets.push(start.len());
    let die = |offset: gimli::UnitOffset<usize>| match die_index.get(&offset.0) {
        Some(id) => format!("die {id}"),
        None => format!("die@?{}", offset.0),
    };
    let target = |after: usize, relative: i16| {
        let to = after as i64 + relative as i64;
        match offsets.iter().position(|&o| o as i64 == to) {
            Some(i) => format!("op#{i}"),
            None => format!("op@?{to}"),
        }
    };
    let mut out = Vec::new();
    for (op, after) in ops {
        out.push(match op {
            // `DW_OP_addr` and `DW_OP_addrx` are the same address.
            Operation::Address { address } => Op::Other(format!("Address {address:#x}")),
            Operation::AddressIndex { index } => {
                Op::Other(format!("Address {:#x}", address(index)?))
            }
            Operation::ConstantIndex { index } => {
                Op::Other(format!("Constant {:#x}", address(index)?))
            }
            Operation::WasmLocal { index } => Op::Local(index),
            Operation::WasmGlobal { index } => Op::Global(index),
            Operation::Bra { target: t } => Op::Other(format!("Bra {}", target(after, t))),
            Operation::Skip { target: t } => Op::Other(format!("Skip {}", target(after, t))),
            Operation::Convert { base_type } => Op::Other(format!("Convert {}", die(base_type))),
            Operation::Reinterpret { base_type } => {
                Op::Other(format!("Reinterpret {}", die(base_type)))
            }
            Operation::TypedLiteral { base_type, value } => Op::Other(format!(
                "TypedLiteral {} {:?}",
                die(base_type),
                value.slice()
            )),
            Operation::Deref {
                base_type,
                size,
                space,
            } => Op::Other(format!("Deref {} {size} {space}", die(base_type))),
            Operation::RegisterOffset {
                register,
                offset,
                base_type,
            } => Op::Other(format!(
                "RegisterOffset {register:?} {offset} {}",
                die(base_type)
            )),
            Operation::ParameterRef { offset } => {
                Op::Other(format!("ParameterRef {}", die(offset)))
            }
            Operation::ImplicitValue { data } => {
                Op::Other(format!("ImplicitValue {:?}", data.slice()))
            }
            Operation::EntryValue { expression } => {
                Op::Other(format!("EntryValue {:?}", expression.slice()))
            }
            op => Op::Other(format!("{op:?}")),
        });
    }
    Ok(Expr(out))
}

fn read_info(dwarf: &read::Dwarf<Reader<'_>>) -> Result<Info> {
    let mut info = Info {
        shapes: Vec::new(),
        sequences: Vec::new(),
        scopes: Vec::new(),
        locations: Vec::new(),
    };
    let mut headers = dwarf.units();
    while let Some(header) = headers.next()? {
        let u = info.shapes.len();
        let unit = dwarf.unit(header)?;
        let encoding = unit.encoding();

        // Every DIE's identity, for references to it, in DFS order.
        let mut die_index = HashMap::new();
        let mut ids = Vec::new();
        let mut shape: BTreeMap<String, Vec<String>> = BTreeMap::new();
        // The open DIEs, and how many children of each tag and name each has.
        let mut open: Vec<(String, HashMap<String, usize>)> = Vec::new();
        let mut entries = unit.entries();
        while let Some(entry) = entries.next_dfs()? {
            open.truncate(entry.depth() as usize);
            let name = match entry.attr_value(constants::DW_AT_name) {
                Some(value) => attr_string(dwarf, &unit, value).unwrap_or_default(),
                None => String::new(),
            };
            let key = format!("{}:{name}", entry.tag());
            let id = match open.last_mut() {
                Some((parent, seen)) => {
                    let n = seen.entry(key.clone()).or_insert(0);
                    *n += 1;
                    if entry.tag() != constants::DW_TAG_base_type {
                        shape.entry(parent.clone()).or_default().push(key.clone());
                    }
                    format!("{parent}/{key}#{n}")
                }
                None => key,
            };
            shape.entry(id.clone()).or_default();
            die_index.insert(entry.offset().0, id.clone());
            ids.push(id.clone());
            open.push((id, HashMap::new()));
        }

        // Where each open DIE's expressions mean something, from the root
        // down, as in `Location::scope`.
        let mut stack: Vec<Option<Vec<(u64, u64)>>> = Vec::new();
        let mut entries = unit.entries();
        let mut i = 0;
        while let Some(entry) = entries.next_dfs()? {
            let id = (u, ids[i].clone());
            i += 1;
            stack.truncate(entry.depth() as usize);
            let ranges =
                die_ranges(dwarf, &unit, entry).with_context(|| format!("ranges of DIE {id:?}"))?;
            if let Some(ranges) = ranges.as_ref().filter(|r| !r.is_empty()) {
                info.scopes.push((id.clone(), ranges.clone()));
            }
            let scope = match ranges {
                Some(ranges) => Some(ranges),
                None => stack.last().cloned().flatten(),
            };
            for attr in [constants::DW_AT_location, constants::DW_AT_frame_base] {
                let Some(value) = entry.attr_value(attr) else {
                    continue;
                };
                let address = |index| Ok(dwarf.address(&unit, index)?);
                let decode = |expr| decode_expr(expr, encoding, &die_index, &address);
                let mut out = Vec::new();
                let list = !matches!(value, read::AttributeValue::Exprloc(_));
                if let read::AttributeValue::Exprloc(expr) = value {
                    out.push((0, u64::MAX, decode(expr)?));
                } else if let Some(mut iter) = dwarf
                    .attr_locations(&unit, value)
                    .with_context(|| format!("{attr} of DIE {id:?}"))?
                {
                    while let Some(entry) = iter.next()? {
                        let (begin, end) = (entry.range.begin, entry.range.end);
                        let default = begin == 0 && end == u64::MAX;
                        if !default && (is_tombstone(begin) || begin >= end) {
                            continue;
                        }
                        out.push((begin, end, decode(entry.data)?));
                    }
                } else {
                    continue;
                }
                info.locations.push(Location {
                    die: id.clone(),
                    attr,
                    entries: out,
                    list,
                    scope: scope.clone(),
                });
            }
            stack.push(scope);
        }
        info.shapes.push(shape);

        if let Some(program) = unit.line_program.clone() {
            let mut rows = program.rows();
            let mut current: Option<Sequence> = None;
            while let Some((header, row)) = rows.next_row()? {
                let seq = current.get_or_insert_with(|| Sequence {
                    start: row.address(),
                    end: 0,
                    rows: Vec::new(),
                });
                if row.end_sequence() {
                    let mut seq = current.take().unwrap();
                    seq.end = row.address();
                    if !is_tombstone(seq.start) {
                        info.sequences.push(seq);
                    }
                    continue;
                }
                let file = match row.file(header) {
                    Some(file) => {
                        let mut path = String::new();
                        if let Some(dir) = file.directory(header) {
                            path = attr_string(dwarf, &unit, dir)?;
                            path.push('/');
                        }
                        path.push_str(&attr_string(dwarf, &unit, file.path_name())?);
                        path
                    }
                    None => format!("<file {}>", row.file_index()),
                };
                let column = match row.column() {
                    read::ColumnType::LeftEdge => 0,
                    read::ColumnType::Column(c) => c.get(),
                };
                let line = row.line().map_or(0, |l| l.get());
                seq.rows.push((row.address(), (file, line, column)));
            }
        }
    }
    Ok(info)
}

/// A DIE's ranges, without tombstones, or `None` if it doesn't give any.
/// `Dwarf::die_ranges` doesn't accept a `DW_AT_high_pc` size in a fixed-size
/// form.
fn die_ranges(
    dwarf: &read::Dwarf<Reader<'_>>,
    unit: &read::Unit<Reader<'_>>,
    entry: &read::DebuggingInformationEntry<Reader<'_>>,
) -> Result<Option<Vec<(u64, u64)>>> {
    let mut ranges = Vec::new();
    if let Some(value) = entry.attr_value(constants::DW_AT_ranges) {
        if let Some(mut iter) = dwarf.attr_ranges(unit, value)? {
            while let Some(r) = iter.next()? {
                ranges.push((r.begin, r.end));
            }
        }
    } else if let Some(low) = entry.attr_value(constants::DW_AT_low_pc) {
        // Some producers write a unit's `DW_AT_low_pc` of 0 as data, which
        // LLVM accepts.
        let low = match dwarf.attr_address(unit, low)? {
            Some(low) => low,
            None => low.udata_value().context("DW_AT_low_pc isn't an address")?,
        };
        if let Some(high) = entry.attr(constants::DW_AT_high_pc) {
            let high = match high.raw_value() {
                read::AttributeValue::Data1(_)
                | read::AttributeValue::Data2(_)
                | read::AttributeValue::Data4(_)
                | read::AttributeValue::Data8(_)
                | read::AttributeValue::Udata(_) => {
                    low.wrapping_add(high.raw_value().udata_value().unwrap())
                }
                value => dwarf
                    .attr_address(unit, value)?
                    .context("DW_AT_high_pc isn't an address or a size")?,
            };
            ranges.push((low, high));
        } else {
            return Ok(None);
        }
    } else {
        return Ok(None);
    }
    ranges.retain(|&(b, e)| !is_tombstone(b) && b < e);
    Ok(Some(ranges))
}

impl Info {
    /// The rows in effect at `address`, in every sequence covering it. A
    /// row is in effect until the next one, but not past the end of its
    /// function: it doesn't apply below `after`, the end of the previous
    /// function's body.
    fn lines(&self, address: u64, after: u64) -> Vec<&Line> {
        let mut out = Vec::new();
        for seq in &self.sequences {
            if seq.start <= address && address < seq.end {
                let row = seq.rows.iter().rev().find(|(a, _)| *a <= address);
                if let Some((_, line)) = row.filter(|(a, _)| *a >= after) {
                    out.push(line);
                }
            }
        }
        out.sort();
        out
    }

    /// The DIEs whose ranges cover `address`.
    fn scopes(&self, address: u64) -> Vec<DieId> {
        let mut out: Vec<_> = self
            .scopes
            .iter()
            .filter(|(_, ranges)| ranges.iter().any(|&(b, e)| b <= address && address < e))
            .map(|(id, _)| id.clone())
            .collect();
        out.sort();
        out
    }

    /// Each location's expressions in effect at `address`, if the location
    /// means anything there.
    /// The expressions in effect at `address`, each with its scope if it's
    /// a location list's default entry.
    fn locations(&self, address: u64) -> BTreeMap<(DieId, u16), Vec<Located<'_>>> {
        let covers = |&(b, e): &(u64, u64)| b <= address && address < e;
        let mut out = BTreeMap::new();
        for loc in &self.locations {
            if loc.scope.as_ref().is_some_and(|s| !s.iter().any(covers)) {
                continue;
            }
            let exprs: Vec<_> = loc
                .entries
                .iter()
                .filter(|(b, e, _)| covers(&(*b, *e)))
                .map(|(b, e, expr)| {
                    let whole = loc.list && (*b, *e) == (0, u64::MAX);
                    (expr, loc.scope.as_ref().filter(|_| whole))
                })
                .collect();
            out.insert((loc.die.clone(), loc.attr.0), exprs);
        }
        out
    }
}

/// Calls `f` with the first subprogram named `name`, returning `None` if
/// there's no such subprogram.
fn with_subprogram<T>(
    wasm: &[u8],
    name: &str,
    f: impl FnOnce(
        &read::Dwarf<Reader<'_>>,
        &read::Unit<Reader<'_>>,
        &read::DebuggingInformationEntry<Reader<'_>>,
    ) -> Result<T>,
) -> Result<Option<T>> {
    let code = parse_code(wasm)?;
    let dwarf = load_dwarf(&code.customs)?;
    let mut headers = dwarf.units();
    while let Some(header) = headers.next()? {
        let unit = dwarf.unit(header)?;
        let mut entries = unit.entries();
        while let Some(entry) = entries.next_dfs()? {
            if entry.tag() != constants::DW_TAG_subprogram {
                continue;
            }
            let Some(value) = entry.attr_value(constants::DW_AT_name) else {
                continue;
            };
            if attr_string(&dwarf, &unit, value)? == name {
                return Ok(Some(f(&dwarf, &unit, entry)?));
            }
        }
    }
    Ok(None)
}

/// The ranges, without tombstones, of the subprogram named `name`, or
/// `None` if there's no such subprogram.
pub fn subprogram_ranges(wasm: &[u8], name: &str) -> Result<Option<Vec<(u64, u64)>>> {
    with_subprogram(wasm, name, |dwarf, unit, entry| {
        Ok(die_ranges(dwarf, unit, entry)?.unwrap_or_default())
    })
}

/// The `DW_AT_low_pc` of the subprogram named `name`, if it has one.
pub fn subprogram_low_pc(wasm: &[u8], name: &str) -> Result<Option<u64>> {
    Ok(with_subprogram(wasm, name, |dwarf, unit, entry| {
        Ok(match entry.attr_value(constants::DW_AT_low_pc) {
            Some(value) => dwarf.attr_address(unit, value)?,
            None => None,
        })
    })?
    .flatten())
}

/// Fails unless gimli can read every unit, DIE, attribute, range list,
/// location list and line program in `wasm`'s DWARF, and every range,
/// location list entry and line sequence that isn't a tombstone is within
/// one function body. Meant for walrus's output: producers needn't keep a
/// unit's ranges within one function.
pub fn check_readable(wasm: &[u8]) -> Result<()> {
    let code = parse_code(wasm)?;
    let dwarf = load_dwarf(&code.customs)?;
    let info = read_info(&dwarf).context("the DWARF can't be read")?;

    let in_body = |(begin, end): (u64, u64)| {
        code.funcs
            .iter()
            .any(|f| f.body.0 <= begin && end <= f.body.1)
    };
    for (die, ranges) in &info.scopes {
        if let Some(r) = ranges.iter().find(|r| !in_body(**r)) {
            bail!(
                "DIE {die:?}'s range {:#x}..{:#x} isn't in one function",
                r.0,
                r.1
            );
        }
    }
    for seq in &info.sequences {
        ensure!(
            in_body((seq.start, seq.end)),
            "the line sequence {:#x}..{:#x} isn't in one function",
            seq.start,
            seq.end
        );
    }
    for loc in &info.locations {
        for &(begin, end, _) in &loc.entries {
            let default = begin == 0 && end == u64::MAX;
            ensure!(
                default || in_body((begin, end)),
                "{} of DIE {:?} has an entry {begin:#x}..{end:#x} that isn't in one function",
                loc.attr,
                loc.die
            );
        }
    }
    // Read the attributes `read_info` doesn't look at, too.
    let mut headers = dwarf.units();
    while let Some(header) = headers.next()? {
        let unit = dwarf.unit(header)?;
        let mut entries = unit.entries();
        while let Some(entry) = entries.next_dfs()? {
            for attr in entry.attrs() {
                match attr.value() {
                    read::AttributeValue::DebugStrRef(_)
                    | read::AttributeValue::DebugLineStrRef(_)
                    | read::AttributeValue::DebugStrOffsetsIndex(_) => {
                        dwarf.attr_string(&unit, attr.value())?;
                    }
                    // The last child's sibling is the null entry that ends its
                    // parent's children.
                    read::AttributeValue::UnitRef(offset)
                        if attr.name() == constants::DW_AT_sibling =>
                    {
                        unit.entries_at_offset(offset)?.next_entry()?;
                    }
                    read::AttributeValue::UnitRef(offset) => {
                        unit.entry(offset)?;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Fails unless `output`, which walrus emitted from `input`, tells a
/// debugger the same things as `input` at every instruction both have.
///
/// Functions are paired by name or export name, and must have the same
/// instructions, apart from encodings walrus normalizes. Functions that
/// walrus removed aren't checked.
pub fn check_preserved(input: &[u8], output: &[u8]) -> Result<Stats> {
    let input_code = parse_code(input).context("parsing the input")?;
    let output_code = parse_code(output).context("parsing the output")?;
    let input_dwarf = load_dwarf(&input_code.customs)?;
    let output_dwarf = load_dwarf(&output_code.customs)?;
    let input_info = read_info(&input_dwarf).context("reading the input's DWARF")?;
    let output_info = read_info(&output_dwarf).context("reading the output's DWARF")?;

    ensure!(
        input_info.shapes.len() == output_info.shapes.len(),
        "the input has {} units and the output {}",
        input_info.shapes.len(),
        output_info.shapes.len()
    );
    for (u, (a, b)) in input_info
        .shapes
        .iter()
        .zip(&output_info.shapes)
        .enumerate()
    {
        for (id, children) in a {
            match b.get(id) {
                None => bail!("unit {u}'s DIE {id} isn't in the output"),
                Some(c) if c != children => bail!(
                    "unit {u}'s DIE {id} has children {children:?} in the input, {c:?} in the \
                     output"
                ),
                Some(_) => {}
            }
        }
        if let Some(id) = b.keys().find(|id| !a.contains_key(*id)) {
            bail!("unit {u}'s DIE {id} isn't in the input");
        }
    }

    let pairs = pair_functions(&input_code, &output_code)?;

    // Which input index each local and global became, from the instructions
    // that use them.
    let mut globals: HashMap<u32, u32> = HashMap::new();
    let mut locals: Vec<HashMap<u32, u32>> = Vec::new();
    for &(i, o) in &pairs {
        let (fi, fo) = (&input_code.funcs[i], &output_code.funcs[o]);
        let name = fi.name.as_deref().unwrap_or("?");
        if fi.instrs.len() != fo.instrs.len()
            || fi
                .instrs
                .iter()
                .zip(&fo.instrs)
                .any(|(a, b)| a.kind != b.kind)
        {
            let at = fi
                .instrs
                .iter()
                .zip(&fo.instrs)
                .take_while(|(a, b)| a.kind == b.kind)
                .count();
            bail!(
                "function `{name}`'s instructions differ from instruction {at}: {:?} in the \
                 input, {:?} in the output",
                fi.instrs.get(at).map(|i| &i.kind),
                fo.instrs.get(at).map(|i| &i.kind),
            );
        }
        let mut map: HashMap<u32, u32> = (0..fi.params).map(|p| (p, p)).collect();
        for (a, b) in fi.instrs.iter().zip(&fo.instrs) {
            if let (Some(x), Some(y)) = (a.local, b.local) {
                if *map.entry(x).or_insert(y) != y {
                    bail!("function `{name}`'s local {x} became two locals");
                }
            }
            if let (Some(x), Some(y)) = (a.global, b.global) {
                if *globals.entry(x).or_insert(y) != y {
                    bail!("global {x} became two globals");
                }
            }
        }
        locals.push(map);
    }

    let pair_of: HashMap<usize, usize> = pairs
        .iter()
        .enumerate()
        .map(|(p, &(i, _))| (i, p))
        .collect();

    let mut stats = Stats::default();
    let mut errors = Vec::new();
    for (p, &(i, o)) in pairs.iter().enumerate() {
        let (fi, fo) = (&input_code.funcs[i], &output_code.funcs[o]);
        let name = fi.name.as_deref().unwrap_or("?");
        for (k, (a, b)) in fi.instrs.iter().zip(&fo.instrs).enumerate() {
            stats.instructions += 1;
            let at = || {
                format!(
                    "function `{name}` instruction {k} ({}) at {:#x} in the input, {:#x} in \
                     the output",
                    a.kind, a.address, b.address
                )
            };

            let (la, lb) = (
                input_info.lines(a.address, input_code.body_before(i)),
                output_info.lines(b.address, output_code.body_before(o)),
            );
            if !la.is_empty() {
                stats.with_line += 1;
            }
            if la != lb {
                errors.push(format!("{}: line {la:?} became {lb:?}", at()));
            }

            let (sa, sb) = (input_info.scopes(a.address), output_info.scopes(b.address));
            if !sa.is_empty() {
                stats.in_scope += 1;
            }
            if sa != sb {
                errors.push(format!("{}: scopes {sa:?} became {sb:?}", at()));
            }

            let (va, vb) = (
                input_info.locations(a.address),
                output_info.locations(b.address),
            );
            let keys: std::collections::BTreeSet<_> = va.keys().chain(vb.keys()).collect();
            for key in keys {
                let empty = Vec::new();
                let ea = va.get(key).unwrap_or(&empty);
                let eb = vb.get(key).unwrap_or(&empty);
                // What walrus should have written: the same expressions with
                // the new indices of the function this instruction is in,
                // without those naming removed locals. A list's default entry
                // is one expression for code in every function its scope
                // covers, so it's removed if the new indices differ in any of
                // them that walrus kept code of.
                let mut expected = Vec::new();
                let mut unknown = false;
                for (expr, scope) in ea {
                    let new = |l: &u32| {
                        let new = locals[p].get(l);
                        let differs = |(q, f): (usize, &Func)| {
                            let Some(&q_pair) = pair_of.get(&q) else {
                                return false;
                            };
                            let covers = |s: &Scope| {
                                s.iter().any(|&(b, e)| {
                                    let at = |a: u64| b <= a && a < e;
                                    at(f.body.0) || f.instrs.iter().any(|x| at(x.address))
                                })
                            };
                            q != i && scope.is_some_and(covers) && locals[q_pair].get(l) != new
                        };
                        new.filter(|_| !input_code.funcs.iter().enumerate().any(differs))
                    };
                    let mut ops = Vec::new();
                    let mut removed = false;
                    for op in &expr.0 {
                        ops.push(match op {
                            Op::Local(l) => match new(l) {
                                Some(&n) => Op::Local(n),
                                None => {
                                    removed = true;
                                    op.clone()
                                }
                            },
                            Op::Global(g) => match globals.get(g) {
                                Some(&n) => Op::Global(n),
                                // Kept or not depending on uses this check
                                // can't see, such as exports.
                                None => {
                                    unknown = true;
                                    op.clone()
                                }
                            },
                            op => op.clone(),
                        });
                    }
                    if !removed {
                        expected.push(Expr(ops));
                    }
                }
                if unknown {
                    continue;
                }
                stats.locations += expected.len();
                let actual: Vec<Expr> = eb.iter().map(|(e, _)| (*e).clone()).collect();
                if expected != actual {
                    let attr = constants::DwAt(key.1);
                    errors.push(format!(
                        "{}: {attr} of DIE {:?} should be {expected:?} but is {actual:?}",
                        at(),
                        key.0
                    ));
                }
            }
        }
    }
    if !errors.is_empty() {
        let mut msg = format!(
            "{} differences in debug info between the input and the output:\n",
            errors.len()
        );
        for e in errors.iter().take(20) {
            writeln!(msg, "  {e}").unwrap();
        }
        if errors.len() > 20 {
            writeln!(msg, "  ...").unwrap();
        }
        bail!(msg);
    }
    Ok(stats)
}

/// Names every unnamed function `__fN`, so [`check_preserved`] can pair it,
/// keeping all other sections. Other name subsections are dropped.
pub fn name_functions(wasm: &[u8]) -> Result<Vec<u8>> {
    let code = parse_code(wasm)?;
    let mut names = wasm_encoder::NameMap::new();
    let mut imported = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ImportSection(reader) = payload? {
            for import in reader.into_imports() {
                if let TypeRef::Func(_) | TypeRef::FuncExact(_) = import?.ty {
                    imported += 1;
                }
            }
        }
    }
    let mut existing = HashMap::new();
    for (name, data) in &code.customs {
        if *name == "name" {
            let reader = wasmparser::NameSectionReader::new(wasmparser::BinaryReader::new(data, 0));
            for subsection in reader {
                if let Ok(Name::Function(map)) = subsection {
                    for naming in map.into_iter().flatten() {
                        existing.insert(naming.index, naming.name.to_string());
                    }
                }
            }
        }
    }
    let mut used: std::collections::HashSet<String> = existing.values().cloned().collect();
    for index in 0..imported + code.funcs.len() as u32 {
        let name = match existing.get(&index) {
            Some(name) => name.clone(),
            None => {
                let mut name = format!("__f{index}");
                while !used.insert(name.clone()) {
                    name.push('_');
                }
                name
            }
        };
        names.append(index, &name);
    }

    let mut module = wasm_encoder::Module::new();
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload?;
        if let Payload::CustomSection(section) = &payload {
            if section.name() != "name" {
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
    let mut section = wasm_encoder::NameSection::new();
    section.functions(&names);
    module.section(&section);
    Ok(module.finish())
}
