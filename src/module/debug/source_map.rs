//! Builds a source map of the emitted module, with the `mappings` from the
//! line tables of its rewritten DWARF.
//!
//! In a source map of a Wasm module, the whole module is generated line 0 and
//! a generated column is a byte offset in the module.

use crate::CodeTransform;
use gimli::write::{EndianVec, Sections};
use gimli::{ColumnType, EndianSlice, LittleEndian};
use std::collections::HashMap;

type Reader<'a> = EndianSlice<'a, LittleEndian>;

/// A source location as DWARF writes it: a source index, a 1-based line and
/// a 1-based column, where column 0 is the start of the line.
type Location = (usize, u64, u64);

/// What the source map is built from.
#[derive(Default)]
struct DebugInfo {
    sources: Vec<String>,
    source_indices: HashMap<String, usize>,
    /// Each line table row's module offset and location, if it has one.
    rows: Vec<(u64, Option<Location>)>,
}

/// Returns the JSON source map of the module whose code `transform`
/// describes, from its rewritten DWARF `sections`.
pub(crate) fn build(
    sections: &Sections<EndianVec<LittleEndian>>,
    transform: &CodeTransform,
) -> String {
    let dwarf = gimli::Dwarf::load(|id| -> gimli::Result<Reader> {
        let data = sections.get(id).map_or(&[][..], |w| w.slice());
        Ok(EndianSlice::new(data, LittleEndian))
    })
    .expect("cannot load the rewritten DWARF");
    let code_end = transform.function_ranges.iter().map(|(_, r)| r.end).max();
    let code_len = code_end.map_or(0, |end| end - transform.code_section_start) as u64;
    let mut info = DebugInfo::default();
    info.read(&dwarf, transform.code_section_start as u64, code_len)
        .expect("cannot read the rewritten DWARF");

    // Readers take the last mapping at an offset, so where one sequence ends
    // and the next starts, the next one's location goes last.
    info.rows
        .sort_by_key(|&(offset, location)| (offset, location.is_some()));

    let mut json = String::from("{\"version\":3,\"sources\":[");
    push_list(&mut json, info.sources.iter());
    json.push_str("],\"names\":[],\"mappings\":\"");
    json.push_str(&mappings(&info.rows));
    json.push_str("\"}");
    json
}

impl DebugInfo {
    /// Reads the line tables.
    fn read(
        &mut self,
        dwarf: &gimli::Dwarf<Reader>,
        code_start: u64,
        code_len: u64,
    ) -> gimli::Result<()> {
        let mut units = dwarf.units();
        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            self.read_line_table(dwarf, &unit, code_start, code_len)?;
        }
        Ok(())
    }

    fn read_line_table(
        &mut self,
        dwarf: &gimli::Dwarf<Reader>,
        unit: &gimli::Unit<Reader>,
        code_start: u64,
        code_len: u64,
    ) -> gimli::Result<()> {
        let Some(program) = unit.line_program.clone() else {
            return Ok(());
        };
        let mut files = HashMap::new();
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
                    let source = self.file(dwarf, unit, &mut files, row.file_index())?;
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
        // compilation directory, unless one of them is absolute. Directory 0
        // is the compilation directory itself, so it isn't joined again.
        let mut path = match unit.comp_dir {
            Some(dir) => dir.to_string_lossy().into_owned(),
            None => String::new(),
        };
        if let Some(dir) = file
            .directory(header)
            .filter(|_| file.directory_index() != 0)
        {
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

/// Encodes sorted `rows`, each the location from its offset on, as the
/// mappings of a source map. A mapping without a location ends the one before
/// it.
fn mappings(rows: &[(u64, Option<Location>)]) -> String {
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
            let (l, c) = position(l, c);
            vlq(&mut out, s as i64 - source);
            vlq(&mut out, l as i64 - line);
            vlq(&mut out, c as i64 - column);
            (source, line, column) = (s as i64, l as i64, c as i64);
        }
        written = Some(location);
    }
    out
}

/// The 0-based line and column of DWARF's 1-based line and column, where
/// column 0 is the start of the line.
fn position(line: u64, column: u64) -> (u64, u64) {
    (line.saturating_sub(1), column.max(1) - 1)
}

/// Appends `value` as a base64 VLQ, the encoding of source map fields.
fn vlq(out: &mut String, value: i64) {
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rest = (value.unsigned_abs() << 1) | u64::from(value < 0);
    loop {
        let digit = (rest & 31) as usize;
        rest >>= 5;
        out.push(BASE64[digit | if rest > 0 { 32 } else { 0 }] as char);
        if rest == 0 {
            return;
        }
    }
}

/// Appends JSON strings, separated by commas.
fn push_list<'a>(out: &mut String, items: impl Iterator<Item = &'a String>) {
    for (i, item) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
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
        assert_eq!(mappings(&rows), "YAIQ,I,ICFR,UAGA");
    }
}
