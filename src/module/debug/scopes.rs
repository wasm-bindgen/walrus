//! Original scopes and generated ranges, in the [Scopes] format, from the
//! functions and inlined calls that DWARF describes.
//!
//! Each source gets a `Module` scope that covers the whole file, and each
//! source function a `Function` scope nested in it: it starts at the
//! declaration line, and it ends after the last position its code or its
//! calls map to, since DWARF records no end. Copies of one source function
//! (monomorphizations, compiler shims, or an inlined function's abstract and
//! concrete entries) share a scope. Each function in the module gets a
//! generated range, which is a stack frame, and each inlined call a range
//! nested in its caller's, with its call site.
//!
//! This module only reads DWARF. Names, and which names belong to copies of
//! one function, depend on the language, and come from [`FunctionNames`].
//!
//! [Scopes]: https://tc39.es/ecma426/branch/proposal-scopes/

use super::source_map::{vlq, vlq_unsigned, Names, SourceMap};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};

/// How to name functions from what DWARF records, for one language, and
/// which names belong to copies of one source function.
pub(crate) trait FunctionNames {
    /// The name of a `DW_AT_name` inside `DW_TAG_namespace` entries. A
    /// namespace without a `DW_AT_name` is empty.
    fn qualify(&self, namespaces: &[String], name: &str) -> String;

    /// The name of one function, from the nearest `DW_AT_linkage_name` and
    /// qualified `DW_AT_name` along its entries.
    fn name(&self, linkage_name: Option<&str>, name: Option<&str>) -> Option<String>;

    /// The name that all copies of the function named `name` share. Scopes
    /// in one source with the same group name that overlap are one scope,
    /// with that name.
    fn group_name(&self, name: &str) -> String;

    /// The part of `name` that functions declared at one place share when
    /// their names differ, such as in their self types. Declared scopes that
    /// start at the same position with the same key are one scope if
    /// [`shared_name`](Self::shared_name) gives them a name.
    fn declaration_key<'a>(&self, name: &'a str) -> &'a str;

    /// The name of one scope for functions named `a` and `b` that are
    /// declared at one place, or `None` if each keeps a scope of its own.
    fn shared_name(&self, a: &str, b: &str) -> Option<String>;
}

/// A 0-based source line and column.
pub(crate) type Pos = (u64, u64);

/// The position of DWARF's 1-based line and column, where column 0 is the
/// start of the line.
pub(crate) fn position(line: u64, column: u64) -> Pos {
    (line.saturating_sub(1), column.max(1) - 1)
}

struct Scope {
    source: usize,
    name: Option<String>,
    start: Pos,
    /// Exclusive.
    end: Pos,
    /// The first position its code or its calls map to at or after the
    /// declaration line, if any.
    first: Option<Pos>,
    /// The functions it describes, as their canonical entries' offsets.
    keys: Vec<usize>,
    /// Whether one of them has a declaration line.
    declared: bool,
    children: Vec<usize>,
}

struct Range {
    start: u64,
    end: u64,
    scope: Option<usize>,
    frame: bool,
    call: Option<(usize, Pos)>,
    children: Vec<Range>,
}

pub(crate) struct Scopes {
    scopes: Vec<Scope>,
    /// The top-level scopes of each source.
    roots: BTreeMap<usize, Vec<usize>>,
    ranges: Vec<Range>,
}

/// Each function's source positions: where its own code maps to and where
/// it calls inlined functions, by source.
type Positions = HashMap<usize, BTreeMap<usize, Vec<Pos>>>;

pub(crate) fn build(info: &SourceMap, naming: &dyn FunctionNames) -> Scopes {
    // An entry's function is the end of its chain of abstract origins.
    let canonical = |mut offset: usize| {
        for _ in 0..16 {
            match info.functions.get(&offset).and_then(|f| f.origin) {
                Some(origin) if info.functions.contains_key(&origin) => offset = origin,
                _ => break,
            }
        }
        offset
    };
    let keys: Vec<usize> = info.code.iter().map(|c| canonical(c.function)).collect();
    // Where each function's chain starts: the first entry that describes its
    // code, which is the nearest to the code.
    let mut starts: HashMap<usize, usize> = HashMap::new();
    for (code, &key) in info.code.iter().zip(&keys) {
        let start = starts.entry(key).or_insert(code.function);
        *start = (*start).min(code.function);
    }
    let (positions, calls) = positions(info, &keys);
    let mut scopes = merge(info, naming, &starts, &positions);
    let key_scope: HashMap<usize, usize> = scopes
        .iter()
        .enumerate()
        .flat_map(|(i, s)| s.keys.iter().map(move |&k| (k, i)))
        .collect();
    let roots = nest(&mut scopes, &calls);

    let cx = RangeContext {
        info,
        keys: &keys,
        key_scope: &key_scope,
        scopes: &scopes,
    };
    let mut top: Vec<Range> = (0..info.code.len())
        .filter(|&i| info.code[i].parent.is_none())
        .flat_map(|i| cx.ranges(i))
        .collect();
    top.sort_by_key(|r| r.start);
    // Separate functions' code doesn't overlap; keep the first if it does.
    let mut ranges: Vec<Range> = Vec::with_capacity(top.len());
    for range in top {
        if ranges.last().is_none_or(|last| last.end <= range.start) {
            ranges.push(range);
        }
    }
    Scopes {
        scopes,
        roots,
        ranges,
    }
}

/// Each function's positions, and the positions of the calls inlined into
/// it, from the line table rows of the innermost entry that covers them.
fn positions(info: &SourceMap, keys: &[usize]) -> (Positions, HashMap<usize, Vec<(usize, Pos)>>) {
    let mut positions = Positions::new();
    let mut calls: HashMap<usize, Vec<(usize, Pos)>> = HashMap::new();
    let mut intervals: Vec<(u64, usize, u64, usize)> = Vec::new();
    for (i, code) in info.code.iter().enumerate() {
        for &(start, end) in &code.ranges {
            intervals.push((start, code.depth, end, i));
        }
    }
    intervals.sort_unstable();
    let (mut open, mut next) = (Vec::<(u64, usize, u64, usize)>::new(), 0);
    for &(offset, location) in &info.rows {
        while next < intervals.len() && intervals[next].0 <= offset {
            open.push(intervals[next]);
            next += 1;
        }
        while open.last().is_some_and(|i| i.2 <= offset) {
            open.pop();
        }
        if let (Some(&(.., i)), Some((source, line, column))) = (open.last(), location) {
            let at = positions.entry(keys[i]).or_default();
            at.entry(source).or_default().push(position(line, column));
        }
    }
    for code in &info.code {
        if let (Some((source, line, column)), Some(parent)) = (code.call, code.parent) {
            if line > 0 {
                let call = position(line, column);
                calls.entry(keys[parent]).or_default().push((source, call));
                let at = positions.entry(keys[parent]).or_default();
                at.entry(source).or_default().push(call);
            }
        }
    }
    (positions, calls)
}

/// One original scope per source function.
fn merge(
    info: &SourceMap,
    naming: &dyn FunctionNames,
    starts: &HashMap<usize, usize>,
    positions: &Positions,
) -> Vec<Scope> {
    let mut sorted: Vec<(usize, usize)> = starts.iter().map(|(&k, &s)| (k, s)).collect();
    sorted.sort_unstable();
    // Copies of a function have the same group name.
    let mut groups: BTreeMap<(usize, Option<String>), Vec<Scope>> = BTreeMap::new();
    for (key, start) in sorted {
        if let Some(scope) = describe(info, naming, key, start, positions) {
            let name = scope.name.as_deref().map(|n| naming.group_name(n));
            groups.entry((scope.source, name)).or_default().push(scope);
        }
    }
    let mut scopes: Vec<Scope> = Vec::new();
    for ((_, name), mut group) in groups {
        group.sort_by_key(|s| (s.start, s.end));
        let mut current: Option<Scope> = None;
        for scope in group {
            match current.as_mut() {
                Some(c) if scope.start < c.end => {
                    c.end = c.end.max(scope.end);
                    c.first = earliest(c.first, scope.first);
                    c.keys.extend(scope.keys);
                    c.declared |= scope.declared;
                }
                _ => {
                    scopes.extend(current.take());
                    current = Some(Scope {
                        name: name.clone(),
                        ..scope
                    });
                }
            }
        }
        scopes.extend(current);
    }
    // Functions declared at one place can keep different names, such as
    // Rust's functions whose self types differ (`<A as FnMut>::call_mut`
    // and `<B as FnMut>::call_mut`), including one macro's impls for
    // different types. Merge declared scopes that start at the same place,
    // have the same declaration key and have a shared name. Others, such as
    // impls of different traits (`LowerHex::fmt` and `UpperHex::fmt`), stay
    // apart for `nest`.
    let declaration_key = |s: &Scope| {
        let name = s.name.as_deref().unwrap_or_default();
        naming.declaration_key(name).to_owned()
    };
    let mut by_decl: HashMap<(usize, Pos, String), Vec<usize>> = HashMap::new();
    let mut out: Vec<Scope> = Vec::with_capacity(scopes.len());
    for scope in scopes {
        if !scope.declared {
            out.push(scope);
            continue;
        }
        let decl = (scope.source, scope.start, declaration_key(&scope));
        let candidates = by_decl.entry(decl).or_default();
        let shared = candidates.iter().find_map(|&i| {
            let name = match (&out[i].name, &scope.name) {
                (Some(a), Some(b)) if a != b => Some(naming.shared_name(a, b)?),
                (a, _) => a.clone(),
            };
            Some((i, name))
        });
        match shared {
            Some((i, name)) => {
                let o = &mut out[i];
                o.name = name;
                o.end = o.end.max(scope.end);
                o.first = earliest(o.first, scope.first);
                o.keys.extend(scope.keys);
            }
            None => {
                candidates.push(out.len());
                out.push(scope);
            }
        }
    }
    let sort = |scopes: &mut Vec<Scope>| {
        scopes.sort_by(|a, b| {
            (a.source, a.start, Reverse(a.end), &a.name).cmp(&(
                b.source,
                b.start,
                Reverse(b.end),
                &b.name,
            ))
        })
    };
    sort(&mut out);
    tighten(&mut out);
    sort(&mut out);
    out
}

/// Moves the start of each sorted scope that starts inside another one, on a
/// later line than it, from the start of its declaration line to its first
/// position, when it has none on that line. Such a scope is often a closure
/// whose body starts on a later line, and otherwise it would cover the calls
/// its parent makes before it on the declaration line. Scopes declared on
/// one line, such as the functions of one `#[derive]`, aren't nested in
/// each other, so they keep their starts.
fn tighten(scopes: &mut [Scope]) {
    let mut open: Vec<usize> = Vec::new();
    for i in 0..scopes.len() {
        if i > 0 && scopes[i].source != scopes[i - 1].source {
            open.clear();
        }
        let start = scopes[i].start;
        open.retain(|&p| start < scopes[p].end);
        let inside = open.iter().any(|&p| scopes[p].start.0 < start.0);
        let scope = &mut scopes[i];
        if let Some(first) = scope
            .first
            .filter(|&f| inside && start < f && f < scope.end)
        {
            scope.start = first;
        }
        open.push(i);
    }
}

/// The original scope of the function `key`, if it has a name or a
/// position. Each attribute comes from the nearest entry that has it along
/// the chain of `DW_AT_abstract_origin` and `DW_AT_specification` from
/// `start`, as a concrete entry, its abstract instance and its declaration
/// can each carry some of them.
fn describe(
    info: &SourceMap,
    naming: &dyn FunctionNames,
    key: usize,
    start: usize,
    positions: &Positions,
) -> Option<Scope> {
    let (mut linkage, mut decl_file, mut decl_line, mut name) = (None, None, None, None);
    let mut offset = start;
    for _ in 0..16 {
        let Some(f) = info.functions.get(&offset) else {
            break;
        };
        linkage = linkage.or(f.linkage_name.as_deref());
        decl_file = decl_file.or(f.decl_file);
        decl_line = decl_line.or(f.decl_line);
        name = name.or(f.name.as_deref());
        match f.origin {
            Some(origin) if origin != offset => offset = origin,
            _ => break,
        }
    }
    let name = naming.name(linkage, name);
    let decl = decl_file.zip(decl_line);
    let by_source = positions.get(&key);
    let (source, start, end, first) = if let Some((source, line)) = decl {
        let decl = position(line, 0);
        match by_source.and_then(|m| m.get(&source)) {
            Some(at) => {
                // Positions above the declaration come from code inlined
                // without an entry of its own; ignore them.
                let first = at.iter().filter(|&&p| p >= decl).min().copied();
                // The first position on the declaration line, else the start
                // of that line.
                let start = first.filter(|p| p.0 == decl.0).unwrap_or(decl);
                let last = at
                    .iter()
                    .filter(|&&p| p >= decl)
                    .max()
                    .copied()
                    .unwrap_or(start);
                (source, start, (last.0, last.1 + 1), first)
            }
            None => (source, decl, (decl.0 + 1, 0), None),
        }
    } else {
        // Without a declaration, the source most of its code maps to.
        let (&source, at) = by_source?
            .iter()
            .max_by_key(|(&s, at)| (at.len(), Reverse(s)))?;
        let (first, last) = (at.iter().min()?, at.iter().max()?);
        (source, *first, (last.0, last.1 + 1), Some(*first))
    };
    Some(Scope {
        source,
        name,
        start,
        end,
        first,
        keys: vec![key],
        declared: decl.is_some(),
        children: Vec::new(),
    })
}

fn earliest(a: Option<Pos>, b: Option<Pos>) -> Option<Pos> {
    a.into_iter().chain(b).min()
}

/// Nests the sorted scopes of each source. When a scope starts inside
/// another but ends after it, the one whose cut loses fewer call sites (then,
/// the one that sticks out less) is cut: either the outer one ends where the
/// inner one starts, so they become siblings, or the inner one is clipped.
fn nest(
    scopes: &mut [Scope],
    calls: &HashMap<usize, Vec<(usize, Pos)>>,
) -> BTreeMap<usize, Vec<usize>> {
    let lost = |scopes: &[Scope], s: usize, from: Pos, to: Pos| -> usize {
        let scope = &scopes[s];
        let calls = scope
            .keys
            .iter()
            .flat_map(|k| calls.get(k).into_iter().flatten());
        calls
            .filter(|&&(f, p)| f == scope.source && p >= from && p < to)
            .count()
    };
    let distance = |a: Pos, b: Pos| -> Pos {
        if a.0 == b.0 {
            (0, b.1.saturating_sub(a.1))
        } else {
            (b.0.saturating_sub(a.0), 0)
        }
    };
    let mut roots: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut open: Vec<usize> = Vec::new();
    for i in 0..scopes.len() {
        if i > 0 && scopes[i].source != scopes[i - 1].source {
            open.clear();
        }
        let parent = loop {
            while open
                .last()
                .is_some_and(|&t| scopes[t].end <= scopes[i].start)
            {
                open.pop();
            }
            let Some(&p) = open.last() else {
                break None;
            };
            if scopes[i].end <= scopes[p].end {
                break Some(p);
            }
            let lose_inner = lost(scopes, i, scopes[p].end, scopes[i].end);
            let lose_outer = lost(scopes, p, scopes[i].start, scopes[p].end);
            let outside = distance(scopes[p].end, scopes[i].end);
            let inside = distance(scopes[i].start, scopes[p].end);
            if lose_inner > lose_outer || (lose_inner == lose_outer && outside > inside) {
                scopes[p].end = scopes[i].start;
                continue;
            }
            scopes[i].end = scopes[p].end;
            break Some(p);
        };
        match parent {
            Some(p) => scopes[p].children.push(i),
            None => roots.entry(scopes[i].source).or_default().push(i),
        }
        open.push(i);
    }
    roots
}

struct RangeContext<'a> {
    info: &'a SourceMap<'a>,
    keys: &'a [usize],
    key_scope: &'a HashMap<usize, usize>,
    scopes: &'a [Scope],
}

impl RangeContext<'_> {
    /// The ranges of an entry, one per piece of its code, with the ranges of
    /// what is inlined into it nested in them.
    fn ranges(&self, n: usize) -> Vec<Range> {
        let code = &self.info.code[n];
        let mut children: Vec<Range> = code.children.iter().flat_map(|&c| self.ranges(c)).collect();
        children.sort_by_key(|r| r.start);
        let mut children = children.into_iter().peekable();
        let scope = self.key_scope.get(&self.keys[n]).copied();
        // A call site needs the called function's scope.
        let call = scope.and(code.call).map(|(source, line, column)| {
            let caller = code.parent.and_then(|p| self.key_scope.get(&self.keys[p]));
            match (caller.map(|&s| &self.scopes[s]), line) {
                // LLVM writes `DW_AT_call_line` 0 when it merges calls from
                // different lines. Point at the caller's start, which is in
                // the calling function, rather than at line 1 of the file.
                (Some(caller), 0) => (caller.source, caller.start),
                // Without the caller's scope, the start of the call's file.
                (None, 0) => (source, (0, 0)),
                _ => (source, position(line, column)),
            }
        });
        let mut out = Vec::with_capacity(code.ranges.len());
        for &(start, end) in &code.ranges {
            let mut nested: Vec<Range> = Vec::new();
            while let Some(mut child) = children.next_if(|c| c.start < end) {
                // Drop what starts before this piece or overlaps a sibling.
                if child.start < nested.last().map_or(start, |p| p.end) {
                    continue;
                }
                clip(&mut child, end);
                nested.push(child);
            }
            out.push(Range {
                start,
                end,
                scope,
                frame: code.subprogram,
                call,
                children: nested,
            });
        }
        out
    }
}

/// Ends `range` and its descendants by `end`.
fn clip(range: &mut Range, end: u64) {
    range.end = range.end.min(end);
    range.children.retain(|c| c.start < end);
    for child in &mut range.children {
        clip(child, end);
    }
}

/// Writes the items of one encoded string.
#[derive(Default)]
struct Encoder {
    out: String,
    position: Pos,
    name: i64,
    kind: i64,
    source: i64,
    scope: i64,
    /// The number of scopes written.
    count: usize,
}

impl Encoder {
    fn item(&mut self, tag: char) {
        if !self.out.is_empty() {
            self.out.push(',');
        }
        self.out.push(tag);
    }

    /// A position relative to the last one: a line, then a column relative to
    /// the last one if on the same line.
    fn position(&mut self, (line, column): Pos) {
        let lines = line - self.position.0;
        vlq_unsigned(&mut self.out, lines);
        let relative = if lines == 0 {
            column - self.position.1
        } else {
            column
        };
        vlq_unsigned(&mut self.out, relative);
        self.position = (line, column);
    }

    /// The start of a scope: a `B` item with its flags, position, name and
    /// kind.
    fn start(
        &mut self,
        start: Pos,
        name: Option<&str>,
        kind: &str,
        frame: bool,
        names: &mut Names,
    ) {
        self.count += 1;
        self.item('B');
        let flags = u64::from(name.is_some()) | 2 | if frame { 4 } else { 0 };
        vlq_unsigned(&mut self.out, flags);
        self.position(start);
        if let Some(name) = name {
            let index = names.index(name) as i64;
            vlq(&mut self.out, index - self.name);
            self.name = index;
        }
        let index = names.index(kind) as i64;
        vlq(&mut self.out, index - self.kind);
        self.kind = index;
    }

    fn end(&mut self, end: Pos) {
        self.item('C');
        self.position(end);
    }

    /// A source's scope: a `Module` around its top-level function scopes,
    /// from the start of the file to the start of the line after the last
    /// one ends.
    fn source(
        &mut self,
        scopes: &Scopes,
        roots: &[usize],
        names: &mut Names,
        flat: &mut [(usize, usize)],
        new_index: &[usize],
    ) {
        self.start((0, 0), None, "Module", false, names);
        for &root in roots {
            self.scope(scopes, root, names, flat, new_index);
        }
        let last = roots.iter().map(|&r| scopes.scopes[r].end.0).max();
        self.end((last.map_or(0, |line| line + 1), 0));
    }

    fn scope(
        &mut self,
        scopes: &Scopes,
        s: usize,
        names: &mut Names,
        flat: &mut [(usize, usize)],
        new_index: &[usize],
    ) {
        let scope = &scopes.scopes[s];
        flat[s] = (new_index[scope.source], self.count);
        self.start(scope.start, scope.name.as_deref(), "Function", true, names);
        for &child in &scope.children {
            self.scope(scopes, child, names, flat, new_index);
        }
        self.end(scope.end);
    }

    fn range(&mut self, range: &Range, flat: &[(usize, usize)], new_index: &[usize]) {
        self.item('E');
        let definition = range.scope.map(|s| flat[s]);
        let flags = if definition.is_some() { 2 } else { 0 } | if range.frame { 4 } else { 0 };
        vlq_unsigned(&mut self.out, flags);
        // Wasm has one generated line, so only columns change.
        vlq_unsigned(&mut self.out, range.start - self.position.1);
        self.position.1 = range.start;
        if let Some((source, scope)) = definition {
            let (source, scope) = (source as i64, scope as i64);
            vlq(&mut self.out, source - self.source);
            if source == self.source {
                vlq(&mut self.out, scope - self.scope);
            } else {
                vlq_unsigned(&mut self.out, scope as u64);
            }
            (self.source, self.scope) = (source, scope);
        }
        if let Some((source, (line, column))) = range.call {
            self.item('I');
            for value in [new_index[source] as u64, line, column] {
                vlq_unsigned(&mut self.out, value);
            }
        }
        for child in &range.children {
            self.range(child, flat, new_index);
        }
        self.item('F');
        vlq_unsigned(&mut self.out, range.end - self.position.1);
        self.position.1 = range.end;
    }
}

impl Scopes {
    /// Marks the sources that scopes and call sites name.
    pub(crate) fn mark_sources(&self, used: &mut [bool]) {
        fn mark(range: &Range, used: &mut [bool]) {
            if let Some((source, _)) = range.call {
                used[source] = true;
            }
            for child in &range.children {
                mark(child, used);
            }
        }
        for &source in self.roots.keys() {
            used[source] = true;
        }
        for range in &self.ranges {
            mark(range, used);
        }
    }

    /// The `scopes` of each of the map's sources, and the `ranges`, with one
    /// segment per top-level range so that a reader can decode only the one
    /// around an offset.
    /// `new_index` maps the sources of the `SourceMap` to the map's `sources`,
    /// of which there are `sources`.
    pub(crate) fn encode(
        &self,
        new_index: &[usize],
        sources: usize,
        names: &mut Names,
    ) -> (Vec<Option<String>>, Vec<String>) {
        // Each scope's source and index in it, in pre-order, which counts
        // the source's `Module` scope.
        let mut flat = vec![(0, 0); self.scopes.len()];
        let mut scopes = vec![None; sources];
        for (&source, roots) in &self.roots {
            let mut encoder = Encoder::default();
            encoder.source(self, roots, names, &mut flat, new_index);
            scopes[new_index[source]] = Some(encoder.out);
        }
        let ranges = self
            .ranges
            .iter()
            .map(|range| {
                let mut encoder = Encoder::default();
                encoder.range(range, &flat, new_index);
                encoder.out
            })
            .collect();
        (scopes, ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::debug::rust_names::RustNames;
    use crate::module::debug::source_map::{CodeEntry, FunctionEntry};

    fn function(
        name: Option<&str>,
        decl_line: Option<u64>,
        origin: Option<usize>,
    ) -> FunctionEntry {
        FunctionEntry {
            name: name.map(str::to_owned),
            linkage_name: None,
            decl_file: decl_line.map(|_| 0),
            decl_line,
            origin,
        }
    }

    #[test]
    fn nearest_declaration() {
        let mut info = SourceMap::new(0, None);
        // A concrete entry, its abstract instance and its declaration.
        let entry = |linkage: Option<&str>, file, line, origin| FunctionEntry {
            name: Some("f".to_owned()),
            linkage_name: linkage.map(str::to_owned),
            decl_file: file,
            decl_line: line,
            origin,
        };
        info.functions.insert(1, entry(None, None, None, Some(2)));
        info.functions
            .insert(2, entry(None, Some(0), Some(10), Some(3)));
        info.functions.insert(
            3,
            entry(Some("_RNvCs1234_5krate1f"), Some(1), Some(3), None),
        );
        let scope = describe(&info, &RustNames, 3, 1, &Positions::new()).unwrap();
        assert_eq!(scope.name.as_deref(), Some("krate::f"));
        assert_eq!((scope.source, scope.start), (0, (9, 0)));
        // A declaration line without a file takes the file from further on.
        info.functions.get_mut(&2).unwrap().decl_file = None;
        let scope = describe(&info, &RustNames, 3, 1, &Positions::new()).unwrap();
        assert_eq!((scope.source, scope.start), (1, (9, 0)));
    }

    #[test]
    fn merge_at_one_declaration() {
        // Functions that one macro generates, declared at the macro.
        let names = [
            "<&dyn core::fmt::Debug as core::fmt::Debug>::fmt",
            "<a::A as core::fmt::Display>::fmt",
            "<b::B as core::fmt::Display>::fmt",
            "<u64 as core::fmt::LowerHex>::fmt",
            "<u64 as core::fmt::UpperHex>::fmt",
        ];
        let mut info = SourceMap::new(0, None);
        let mut starts = HashMap::new();
        for (key, name) in names.into_iter().enumerate() {
            let entry = FunctionEntry {
                name: Some(name.to_owned()),
                linkage_name: None,
                decl_file: Some(0),
                decl_line: Some(5),
                origin: None,
            };
            info.functions.insert(key, entry);
            starts.insert(key, key);
        }
        let scopes = merge(&info, &RustNames, &starts, &Positions::new());
        let names: Vec<_> = scopes.iter().map(|s| s.name.as_deref().unwrap()).collect();
        assert_eq!(
            names,
            [
                "<&dyn core::fmt::Debug as core::fmt::Debug>::fmt",
                "<_ as core::fmt::Display>::fmt",
                "<u64 as core::fmt::LowerHex>::fmt",
                "<u64 as core::fmt::UpperHex>::fmt",
            ]
        );
    }

    #[test]
    fn overlapping_copies() {
        // Two copies of one closure whose first positions on its declaration
        // line differ, so they don't start at the same place.
        let mut info = SourceMap::new(0, None);
        let mut positions = Positions::new();
        for (key, ty, first) in [(0, "u32", (4, 3)), (1, "u64", (4, 7))] {
            let name = format!("indexmap::inner::equal::<{ty}>::{{closure#0}}");
            info.functions
                .insert(key, function(Some(&name), Some(5), None));
            positions.insert(key, BTreeMap::from([(0, vec![first, (6, 0)])]));
        }
        let starts = HashMap::from([(0, 0), (1, 1)]);
        let scopes = merge(&info, &RustNames, &starts, &positions);
        let scopes: Vec<_> = scopes
            .iter()
            .map(|s| (s.name.as_deref().unwrap(), s.start, s.keys.len()))
            .collect();
        assert_eq!(scopes, [("indexmap::inner::equal::{closure#0}", (4, 3), 2)]);
    }

    #[test]
    fn abstract_origin_start() {
        // `f` has an out-of-line copy at offset 20, a concrete entry whose
        // abstract instance at offset 10 has the declaration, and is also
        // inlined into `g`. Only the inlined code maps to the declaration
        // line, and both are one function, so its scope starts at that
        // position rather than at the start of the line.
        let mut info = SourceMap::new(0, None);
        info.functions
            .insert(10, function(Some("krate::f"), Some(346), None));
        info.functions.insert(20, function(None, None, Some(10)));
        info.functions
            .insert(30, function(Some("krate::g"), Some(400), None));
        let code = |function, range, parent: Option<usize>| CodeEntry {
            function,
            subprogram: parent.is_none(),
            ranges: vec![range],
            call: parent.map(|_| (0, 400, 5)),
            parent,
            children: Vec::new(),
            depth: usize::from(parent.is_some()),
        };
        info.code = vec![
            code(20, (0, 10), None),
            code(30, (10, 30), None),
            code(10, (15, 20), Some(1)),
        ];
        info.code[1].children.push(2);
        info.rows = vec![
            (0, Some((0, 347, 5))),
            (10, Some((0, 400, 1))),
            (15, Some((0, 346, 9))),
            (20, Some((0, 401, 1))),
        ];
        let scopes = build(&info, &RustNames).scopes;
        let f: Vec<_> = scopes
            .iter()
            .filter(|s| s.name.as_deref() == Some("krate::f"))
            .map(|s| s.start)
            .collect();
        assert_eq!(f, [(345, 8)]);
    }

    #[test]
    fn tighten_nested_starts() {
        let scope = |start: Pos, end: Pos, first: Pos| Scope {
            source: 0,
            name: None,
            start,
            end,
            first: Some(first),
            keys: Vec::new(),
            declared: true,
            children: Vec::new(),
        };
        let mut scopes = [
            // A function, which is not nested, so keeps its start.
            scope((10, 0), (20, 0), (11, 4)),
            // Declared on the function's line, such as by a `#[derive]`.
            scope((10, 0), (11, 0), (10, 6)),
            // A closure whose body starts on the line after its declaration.
            scope((12, 0), (14, 0), (13, 8)),
        ];
        tighten(&mut scopes);
        let starts: Vec<_> = scopes.iter().map(|s| s.start).collect();
        assert_eq!(starts, [(10, 0), (10, 0), (13, 8)]);
    }

    #[test]
    fn range_encoding() {
        let range = |start, end, children| Range {
            start,
            end,
            scope: None,
            frame: false,
            call: None,
            children,
        };
        let scopes = Scopes {
            scopes: Vec::new(),
            roots: BTreeMap::new(),
            ranges: vec![range(
                100,
                200,
                vec![range(110, 120, Vec::new()), range(130, 150, Vec::new())],
            )],
        };
        let (_, ranges) = scopes.encode(&[], 0, &mut Names::default());
        // Starts after the first are relative (flags 0, no line) and ends
        // have one value: a line flag with a line delta of 0 reads
        // differently in Chrome's codec, which takes the column as absolute.
        assert_eq!(ranges, ["EAkD,EAK,FK,EAK,FU,FyB"]);
    }
}
