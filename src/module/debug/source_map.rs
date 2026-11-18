//! Builds a source map of the emitted module, with the `mappings` from the
//! lines of its DWARF line tables at the addresses they were emitted to.
//!
//! In a source map of a Wasm module, the whole module is generated line 0 and
//! a generated column is a byte offset in the module.

use super::dwarf::{Files, Line};
use gimli::write::FileId;
use std::collections::HashMap;

/// A source location as DWARF writes it: a source index, a 1-based line and
/// a 1-based column, where column 0 is the start of the line.
type Location = (usize, u64, u64);

/// A source map, built from the lines of each unit.
pub(crate) struct SourceMap {
    /// The module offset of the code section's contents, which DWARF
    /// addresses are relative to.
    code_start: u64,
    sources: Vec<String>,
    source_indices: HashMap<String, usize>,
    /// Each line's module offset and location, if it has one.
    rows: Vec<(u64, Option<Location>)>,
}

impl SourceMap {
    pub(crate) fn new(code_start: u64) -> Self {
        SourceMap {
            code_start,
            sources: Vec::new(),
            source_indices: HashMap::new(),
            rows: Vec::new(),
        }
    }

    /// Adds the lines of one unit, whose `files` they name.
    pub(crate) fn add_unit(&mut self, files: &Files, lines: Vec<Line>) {
        // The source index of each of the unit's files.
        let mut sources = HashMap::new();
        self.rows.reserve(lines.len());
        for (address, line) in lines {
            // Line 0 is code that no single source line produced, so it gets
            // no location rather than the location of the row before it.
            let location = line
                .filter(|&(_, line, _)| line != 0)
                .map(|(file, line, column)| {
                    let source = *sources
                        .entry(file)
                        .or_insert_with(|| self.source(files, file));
                    (source, line, column)
                });
            self.rows.push((self.code_start + address, location));
        }
    }

    /// The source index of a unit's file.
    fn source(&mut self, files: &Files, file: FileId) -> usize {
        // The file's name, under its directory and then the unit's
        // compilation directory, unless one of them is absolute.
        let (comp_dir, dir, name) = files.path(file);
        let mut path = String::from_utf8_lossy(comp_dir).into_owned();
        if let Some(dir) = dir {
            push_path(&mut path, &String::from_utf8_lossy(dir));
        }
        push_path(&mut path, &String::from_utf8_lossy(name));
        let path = normalize(&path);
        let next = self.sources.len();
        *self.source_indices.entry(path).or_insert_with_key(|path| {
            self.sources.push(path.clone());
            next
        })
    }

    /// Returns the JSON source map.
    pub(crate) fn finish(mut self) -> String {
        // Readers take the last mapping at an offset, so where some lines end
        // and others start, the location goes last.
        self.rows
            .sort_by_key(|&(offset, location)| (offset, location.is_some()));

        let mut json = String::from("{\"version\":3,\"sources\":[");
        push_list(&mut json, self.sources.iter());
        json.push_str("],\"names\":[],\"mappings\":\"");
        json.push_str(&mappings(&self.rows));
        json.push_str("\"}");
        json
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
