//! Builds a source map of the emitted module from its rewritten DWARF: the
//! `mappings` from the line tables, and the [Scopes] of functions and inlined
//! calls from the debugging information entries.
//!
//! In a source map of a Wasm module, the whole module is generated line 0 and
//! a generated column is a byte offset in the module.
//!
//! [Scopes]: https://tc39.es/ecma426/branch/proposal-scopes/

use super::scopes::{self, FunctionNames};
use crate::CodeTransform;
use gimli::write::{EndianVec, Sections};
use gimli::{AttributeValue, ColumnType, EndianSlice, LittleEndian};
use std::collections::HashMap;

type Reader<'a> = EndianSlice<'a, LittleEndian>;

/// A source location as DWARF writes it: a source index, a 1-based line and
/// a 1-based column, where column 0 is the start of the line.
pub(crate) type Location = (usize, u64, u64);

/// A `DW_TAG_subprogram`: a function in the source.
#[derive(Default)]
pub(crate) struct FunctionEntry {
    /// `DW_AT_name`, under the namespaces the entry is in.
    pub name: Option<String>,
    pub linkage_name: Option<String>,
    pub decl_file: Option<usize>,
    pub decl_line: Option<u64>,
    /// `DW_AT_abstract_origin` or `DW_AT_specification`.
    pub origin: Option<usize>,
}

/// A `DW_TAG_subprogram` or `DW_TAG_inlined_subroutine` that has code.
pub(crate) struct CodeEntry {
    /// The `.debug_info` offset of the entry that describes the function:
    /// the subprogram itself, or the inlined subroutine's abstract origin.
    pub function: usize,
    /// Whether this is a subprogram, which is a function of its own in the
    /// module rather than one inlined into it.
    pub subprogram: bool,
    /// Sorted, disjoint module offset ranges of its code.
    pub ranges: Vec<(u64, u64)>,
    /// `DW_AT_call_file`, `DW_AT_call_line` and `DW_AT_call_column`.
    pub call: Option<Location>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub depth: usize,
}

/// What the source map is built from.
#[derive(Default)]
pub(crate) struct DebugInfo {
    pub sources: Vec<String>,
    source_indices: HashMap<String, usize>,
    /// Each line table row's module offset and location, if it has one.
    pub rows: Vec<(u64, Option<Location>)>,
    pub functions: HashMap<usize, FunctionEntry>,
    pub code: Vec<CodeEntry>,
}

/// Returns the JSON source map of the module whose code `transform`
/// describes, from its rewritten DWARF `sections`, with functions named by
/// `naming`, and with `scopes` the Scopes of functions and inlined calls.
pub(crate) fn build(
    sections: &Sections<EndianVec<LittleEndian>>,
    transform: &CodeTransform,
    naming: &dyn FunctionNames,
    scopes: bool,
) -> String {
    let dwarf = gimli::Dwarf::load(|id| -> gimli::Result<Reader> {
        let data = sections.get(id).map_or(&[][..], |w| w.slice());
        Ok(EndianSlice::new(data, LittleEndian))
    })
    .expect("cannot load the rewritten DWARF");
    let code_end = transform.function_ranges.iter().map(|(_, r)| r.end).max();
    let code_len = code_end.map_or(0, |end| end - transform.code_section_start) as u64;
    let mut info = DebugInfo::default();
    // Only scopes need the debugging information entries.
    let entries = scopes.then_some(naming);
    info.read(
        &dwarf,
        entries,
        transform.code_section_start as u64,
        code_len,
    )
    .expect("cannot read the rewritten DWARF");

    // Readers take the last mapping at an offset, so where one sequence ends
    // and the next starts, the next one's location goes last.
    info.rows
        .sort_by_key(|&(offset, location)| (offset, location.is_some()));
    let scopes = scopes.then(|| scopes::build(&info, naming));

    // Keep only the sources that a mapping, a scope or a call site names,
    // rather than every file the line tables and entries list.
    let mut used = vec![false; info.sources.len()];
    for (_, location) in &info.rows {
        if let Some((source, ..)) = location {
            used[*source] = true;
        }
    }
    if let Some(scopes) = &scopes {
        scopes.mark_sources(&mut used);
    }
    let mut new_index = vec![usize::MAX; used.len()];
    let mut sources = Vec::new();
    for (i, path) in info.sources.iter().enumerate() {
        if used[i] {
            new_index[i] = sources.len();
            sources.push(path);
        }
    }

    let mut names = Names::default();
    let encoded = scopes.map(|scopes| scopes.encode(&new_index, sources.len(), &mut names));

    let mut json = String::from("{\"version\":3,\"sources\":[");
    push_list(&mut json, sources.into_iter().map(Some));
    json.push_str("],\"names\":[");
    push_list(&mut json, names.list.iter().map(Some));
    json.push_str("],\"mappings\":\"");
    json.push_str(&mappings(&info.rows, &new_index));
    json.push('"');
    if let Some((encoded_scopes, encoded_ranges)) = encoded {
        json.push_str(",\"scopes\":[");
        push_list(&mut json, encoded_scopes.iter().map(Option::as_ref));
        json.push_str("],\"ranges\":[");
        push_list(&mut json, encoded_ranges.iter().map(Some));
        json.push(']');
    }
    json.push('}');
    json
}

impl DebugInfo {
    /// Reads the line tables and, with `entries`, the functions and inlined
    /// calls of the debugging information entries, named by it.
    fn read(
        &mut self,
        dwarf: &gimli::Dwarf<Reader>,
        entries: Option<&dyn FunctionNames>,
        code_start: u64,
        code_len: u64,
    ) -> gimli::Result<()> {
        let mut units = dwarf.units();
        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let mut files = HashMap::new();
            self.read_line_table(dwarf, &unit, &mut files, code_start, code_len)?;
            let Some(naming) = entries else {
                continue;
            };
            let mut tree = unit.entries_tree(None)?;
            let mut walk = Walk {
                dwarf,
                unit: &unit,
                files: &mut files,
                naming,
                namespaces: Vec::new(),
                code_start,
                code_len,
            };
            walk.entry(self, tree.root()?, None)?;
        }
        Ok(())
    }

    fn read_line_table(
        &mut self,
        dwarf: &gimli::Dwarf<Reader>,
        unit: &gimli::Unit<Reader>,
        files: &mut HashMap<u64, usize>,
        code_start: u64,
        code_len: u64,
    ) -> gimli::Result<()> {
        let Some(program) = unit.line_program.clone() else {
            return Ok(());
        };
        let mut rows = program.rows();
        let mut sequence = Vec::new();
        // Whether the current sequence is in code the module still has.
        let mut sequence_live = None;
        while let Some((_, row)) = rows.next_row()? {
            let address = row.address();
            let live = *sequence_live.get_or_insert(0 < address && address < code_len);
            let offset = code_start + address;
            if row.end_sequence() {
                if live {
                    sequence.push((offset, None));
                    self.rows.append(&mut sequence);
                }
                sequence.clear();
                sequence_live = None;
                continue;
            }
            if !live {
                continue;
            }
            // Line 0 is code that no single source line produced, so it gets
            // no location rather than the location of the row before it.
            let location = match row.line() {
                Some(line) => {
                    let source = self.file(dwarf, unit, files, row.file_index())?;
                    let column = match row.column() {
                        ColumnType::LeftEdge => 0,
                        ColumnType::Column(column) => column.get(),
                    };
                    source.map(|source| (source, line.get(), column))
                }
                None => None,
            };
            sequence.push((offset, location));
        }
        Ok(())
    }

    /// The source index of a unit's file, which `files` caches.
    fn file(
        &mut self,
        dwarf: &gimli::Dwarf<Reader>,
        unit: &gimli::Unit<Reader>,
        files: &mut HashMap<u64, usize>,
        index: u64,
    ) -> gimli::Result<Option<usize>> {
        if let Some(&source) = files.get(&index) {
            return Ok(Some(source));
        }
        let Some(program) = &unit.line_program else {
            return Ok(None);
        };
        let header = program.header();
        let Some(file) = header.file(index) else {
            return Ok(None);
        };
        // The file's name, under its directory and then the unit's
        // compilation directory, unless one of them is absolute.
        let mut path = match unit.comp_dir {
            Some(dir) => dir.to_string_lossy().into_owned(),
            None => String::new(),
        };
        if let Some(dir) = file.directory(header) {
            push_path(&mut path, &dwarf.attr_string(unit, dir)?.to_string_lossy());
        }
        push_path(
            &mut path,
            &dwarf.attr_string(unit, file.path_name())?.to_string_lossy(),
        );
        let path = normalize(&path);
        let next = self.sources.len();
        let source = *self.source_indices.entry(path).or_insert_with_key(|path| {
            self.sources.push(path.clone());
            next
        });
        files.insert(index, source);
        Ok(Some(source))
    }
}

fn push_path(path: &mut String, part: &str) {
    let absolute = part.starts_with(['/', '\\']) || part.as_bytes().get(1) == Some(&b':');
    if absolute || path.is_empty() {
        *path = part.to_owned();
    } else if !part.is_empty() {
        if !path.ends_with(['/', '\\']) {
            path.push('/');
        }
        path.push_str(part);
    }
}

/// Turns Windows separators (`\\`) into `/`, as source map URLs use, and
/// removes `.` and `..` segments, as browsers do for those URLs, so that one
/// file reached through different directories is one source.
fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    // The root: empty for `/a`, or a drive for `C:/a`.
    let root = |s: &str| s.is_empty() || (s.len() == 2 && s.ends_with(':'));
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            // `/..` is `/`, and `C:/..` is `C:/`.
            ".." if out.len() == 1 && root(out[0]) => {}
            ".." if out.last().is_some_and(|s| !s.is_empty() && *s != "..") => {
                out.pop();
            }
            segment => out.push(segment),
        }
    }
    out.join("/")
}

/// Walks the debugging information entries of one unit.
struct Walk<'a, 'b> {
    dwarf: &'a gimli::Dwarf<Reader<'b>>,
    unit: &'a gimli::Unit<Reader<'b>>,
    files: &'a mut HashMap<u64, usize>,
    naming: &'a dyn FunctionNames,
    namespaces: Vec<String>,
    code_start: u64,
    code_len: u64,
}

impl<'b> Walk<'_, 'b> {
    fn entry(
        &mut self,
        info: &mut DebugInfo,
        node: gimli::EntriesTreeNode<Reader<'b>>,
        parent: Option<usize>,
    ) -> gimli::Result<()> {
        let entry = node.entry();
        let mut child_parent = parent;
        let namespaces = self.namespaces.len();
        match entry.tag() {
            gimli::DW_TAG_namespace => {
                let name = match entry.attr_value(gimli::DW_AT_name) {
                    Some(name) => self.string(name)?,
                    None => String::new(),
                };
                self.namespaces.push(name);
            }
            tag @ (gimli::DW_TAG_subprogram | gimli::DW_TAG_inlined_subroutine) => {
                let subprogram = tag == gimli::DW_TAG_subprogram;
                let offset = entry
                    .offset()
                    .to_debug_info_offset(&self.unit.header)
                    .unwrap()
                    .0;
                let mut function = FunctionEntry::default();
                let mut decl_file = None;
                let (mut call_file, mut call_line, mut call_column) = (None, 0, 0);
                for attr in entry.attrs() {
                    match attr.name() {
                        gimli::DW_AT_name => function.name = Some(self.string(attr.value())?),
                        gimli::DW_AT_linkage_name | gimli::DW_AT_MIPS_linkage_name => {
                            function.linkage_name = Some(self.string(attr.value())?)
                        }
                        gimli::DW_AT_decl_file => decl_file = attr.udata_value(),
                        gimli::DW_AT_decl_line => function.decl_line = attr.udata_value(),
                        gimli::DW_AT_abstract_origin | gimli::DW_AT_specification => {
                            function.origin = match attr.value() {
                                AttributeValue::UnitRef(o) => {
                                    o.to_debug_info_offset(&self.unit.header).map(|o| o.0)
                                }
                                AttributeValue::DebugInfoRef(o) => Some(o.0),
                                _ => None,
                            }
                        }
                        gimli::DW_AT_call_file => call_file = attr.udata_value(),
                        gimli::DW_AT_call_line => call_line = attr.udata_value().unwrap_or(0),
                        gimli::DW_AT_call_column => call_column = attr.udata_value().unwrap_or(0),
                        _ => {}
                    }
                }
                if let Some(file) = decl_file {
                    function.decl_file = self.file(info, file)?;
                }
                let call = match call_file {
                    Some(file) if !subprogram => self
                        .file(info, file)?
                        .map(|file| (file, call_line, call_column)),
                    _ => None,
                };
                if let Some(name) = &mut function.name {
                    if !self.namespaces.is_empty() {
                        *name = self.naming.qualify(&self.namespaces, name);
                    }
                }
                let origin = function.origin;
                if subprogram {
                    info.functions.insert(offset, function);
                }

                let mut ranges = Vec::new();
                // walrus leaves out the pieces of a range list in removed code.
                let mut any_range = entry.attr(gimli::DW_AT_ranges).is_some();
                let mut iter = self.dwarf.die_ranges(self.unit, entry)?;
                while let Some(range) = iter.next()? {
                    any_range = true;
                    if 0 < range.begin && range.begin < range.end && range.end <= self.code_len {
                        ranges.push((self.code_start + range.begin, self.code_start + range.end));
                    } else if range.begin != 0 && range.begin < 0xffff_fff0 {
                        log::debug!(
                            "source map: dropping range [{:#x}, {:#x}) outside the code of entry {offset:#x}",
                            range.begin,
                            range.end
                        );
                    }
                }
                if any_range && ranges.is_empty() {
                    // Removed code: skip the entry and everything inlined into it.
                    self.namespaces.truncate(namespaces);
                    return Ok(());
                }
                if !ranges.is_empty() {
                    ranges.sort_unstable();
                    let mut merged: Vec<(u64, u64)> = Vec::new();
                    for range in ranges {
                        match merged.last_mut() {
                            Some(last) if range.0 <= last.1 => last.1 = last.1.max(range.1),
                            _ => merged.push(range),
                        }
                    }
                    let id = info.code.len();
                    info.code.push(CodeEntry {
                        function: if subprogram {
                            offset
                        } else {
                            origin.unwrap_or(offset)
                        },
                        subprogram,
                        ranges: merged,
                        call,
                        parent,
                        children: Vec::new(),
                        depth: parent.map_or(0, |p| info.code[p].depth + 1),
                    });
                    if let Some(p) = parent {
                        info.code[p].children.push(id);
                    }
                    child_parent = Some(id);
                }
            }
            _ => {}
        }
        let mut children = node.children();
        while let Some(child) = children.next()? {
            self.entry(info, child, child_parent)?;
        }
        self.namespaces.truncate(namespaces);
        Ok(())
    }

    fn string(&self, value: AttributeValue<Reader>) -> gimli::Result<String> {
        Ok(self
            .dwarf
            .attr_string(self.unit, value)?
            .to_string_lossy()
            .into_owned())
    }

    fn file(&mut self, info: &mut DebugInfo, index: u64) -> gimli::Result<Option<usize>> {
        info.file(self.dwarf, self.unit, self.files, index)
    }
}

/// Encodes sorted `rows`, each the location from its offset on, as the
/// mappings of a source map. A mapping without a location ends the one before
/// it.
fn mappings(rows: &[(u64, Option<Location>)], new_index: &[usize]) -> String {
    let mut out = String::new();
    let mut written: Option<Option<Location>> = None;
    let (mut offset, mut source, mut line, mut column) = (0, 0, 0, 0);
    for (i, &(at, location)) in rows.iter().enumerate() {
        let replaced = rows.get(i + 1).is_some_and(|next| next.0 == at);
        let unchanged = written.unwrap_or(None) == location;
        if replaced || unchanged {
            continue;
        }
        if written.is_some() {
            out.push(',');
        }
        vlq(&mut out, at as i64 - offset);
        offset = at as i64;
        if let Some((s, l, c)) = location {
            let (s, (l, c)) = (new_index[s], scopes::position(l, c));
            vlq(&mut out, s as i64 - source);
            vlq(&mut out, l as i64 - line);
            vlq(&mut out, c as i64 - column);
            (source, line, column) = (s as i64, l as i64, c as i64);
        }
        written = Some(location);
    }
    out
}

/// Appends `value` as a base64 VLQ, the encoding of source map fields.
pub(crate) fn vlq(out: &mut String, value: i64) {
    vlq_unsigned(out, (value.unsigned_abs() << 1) | u64::from(value < 0));
}

/// Appends `value` as a base64 VLQ without a sign bit, as the Scopes items'
/// unsigned values are.
pub(crate) fn vlq_unsigned(out: &mut String, mut rest: u64) {
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    loop {
        let digit = (rest & 31) as usize;
        rest >>= 5;
        out.push(BASE64[digit | if rest > 0 { 32 } else { 0 }] as char);
        if rest == 0 {
            return;
        }
    }
}

/// The `names` of the source map.
#[derive(Default)]
pub(crate) struct Names {
    list: Vec<String>,
    indices: HashMap<String, usize>,
}

impl Names {
    pub(crate) fn index(&mut self, name: &str) -> usize {
        if let Some(&i) = self.indices.get(name) {
            return i;
        }
        self.list.push(name.to_owned());
        self.indices.insert(name.to_owned(), self.list.len() - 1);
        self.list.len() - 1
    }
}

/// Appends JSON strings, or `null`s, separated by commas.
fn push_list<'a>(out: &mut String, items: impl Iterator<Item = Option<&'a String>>) {
    for (i, item) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
        let Some(item) = item else {
            out.push_str("null");
            continue;
        };
        out.push('"');
        for c in item.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_matches_the_spec_examples() {
        let encode = |value| {
            let mut out = String::new();
            vlq(&mut out, value);
            out
        };
        assert_eq!(encode(0), "A");
        assert_eq!(encode(1), "C");
        assert_eq!(encode(-1), "D");
        assert_eq!(encode(15), "e");
        assert_eq!(encode(16), "gB");
        assert_eq!(encode(-16), "hB");
        assert_eq!(encode(1000), "w+B");
    }

    #[test]
    fn paths_are_normalized() {
        assert_eq!(
            normalize("/rustc/x/library/core/src/../../stdarch/src/mod.rs"),
            "/rustc/x/library/stdarch/src/mod.rs"
        );
        assert_eq!(normalize("/a/./b/../../../c"), "/c");
        assert_eq!(normalize("../a/./b/../c"), "../a/c");
    }

    #[test]
    fn windows_paths_use_slashes_and_are_normalized() {
        assert_eq!(normalize("C:\\a\\..\\b\\.\\c.rs"), "C:/b/c.rs");
        assert_eq!(normalize("C:\\..\\a.rs"), "C:/a.rs");
        assert_eq!(normalize("C:\\proj/src\\lib.rs"), "C:/proj/src/lib.rs");
        assert_eq!(normalize("..\\a\\b.rs"), "../a/b.rs");
        // A unit's directory and file joined into one path.
        let mut path = "C:\\Users\\me\\proj".to_owned();
        push_path(&mut path, "src\\..\\src");
        push_path(&mut path, "lib.rs");
        assert_eq!(normalize(&path), "C:/Users/me/proj/src/lib.rs");
        let mut path = "C:\\proj".to_owned();
        push_path(&mut path, "D:\\deps\\x.rs");
        assert_eq!(normalize(&path), "D:/deps/x.rs");
    }

    #[test]
    fn mappings_end_at_code_without_a_location() {
        let rows = vec![
            (10, None),
            (12, Some((0, 5, 9))),
            (14, Some((0, 5, 9))),
            (16, None),
            (17, None),
            (20, Some((1, 3, 0))),
            (30, None),
            // A sequence that starts where another one ends.
            (30, Some((1, 6, 1))),
        ];
        // The leading row without a location and repeated locations are
        // dropped, and the location at 30 wins over the end before it.
        assert_eq!(mappings(&rows, &[0, 1]), "YAIQ,I,ICFR,UAGA");
    }
}
