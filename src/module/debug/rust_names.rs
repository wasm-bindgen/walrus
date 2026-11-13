//! The names of Rust functions in the source map.
//!
//! The source map is built from DWARF alone, which records names as the
//! compiler wrote them: mangled linkage names, and `DW_AT_name`s under
//! namespaces. How to turn those into the names a developer reads, and
//! which names belong to copies of one source function, depends on the
//! language. Those rules sit behind [`FunctionNames`], and this module has
//! the rules for Rust, the only ones for now. Other languages could supply
//! their own rules later, as debuggers that read DWARF do with language
//! plugins.

use super::scopes::FunctionNames;

/// Names functions as `rustc-demangle` does, without the hash.
pub(crate) struct RustNames;

impl FunctionNames for RustNames {
    fn qualify(&self, namespaces: &[String], name: &str) -> String {
        format!("{}::{name}", namespaces.join("::"))
    }

    fn name(&self, linkage_name: Option<&str>, name: Option<&str>) -> Option<String> {
        match linkage_name.and_then(|l| rustc_demangle::try_demangle(l).ok()) {
            Some(demangled) => Some(format!("{demangled:#}")),
            None => name.map(str::to_owned),
        }
    }

    fn group_name(&self, name: &str) -> String {
        strip_generics(name)
    }

    fn declaration_key<'a>(&self, name: &'a str) -> &'a str {
        name.rsplit("::").next().unwrap_or(name)
    }

    fn shared_name(&self, a: &str, b: &str) -> String {
        common_name(a, b)
    }
}

/// Drops generic arguments (`Vec<u8>` to `Vec`, `f::<T>` to `f`), `.llvm.`
/// suffixes and compiler shims (`f::{shim:vtable#0}` to `f`), so that
/// monomorphizations and shims share their function's name.
fn strip_generics(name: &str) -> String {
    let name = match name.find(" (.llvm.").or_else(|| name.find(".llvm.")) {
        Some(i) => &name[..i],
        None => name,
    };
    // A shim (`{shim:vtable#0}` in v0 mangling, `{{vtable.shim}}` in legacy
    // mangling) has its function's source position, and the name section
    // still names it in the outermost frame.
    let mut name = name
        .replace("{{vtable.shim}}", "")
        .replace("{{reify.shim}}", "");
    while let Some(i) = name.find("::{shim:") {
        let Some(end) = name[i..].find('}') else {
            break;
        };
        name.replace_range(i..=i + end, "");
    }
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let after_ident = out
            .chars()
            .last()
            .is_some_and(|p| p.is_alphanumeric() || p == '_' || p == '}');
        let turbofish = out.ends_with("::");
        if chars[i] == '<' && (after_ident || turbofish) {
            let mut depth = 0;
            while i < chars.len() {
                match chars[i] {
                    '<' => depth += 1,
                    '>' if i > 0 && chars[i - 1] == '-' => {}
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            if turbofish {
                out.truncate(out.len() - 2);
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `<A as T>::f` and `<B as T>::f` to `<_ as T>::f`; otherwise `_` and the
/// path segments they share at the end.
///
/// Each source function has one original scope, which all of its instances
/// share, so its name must be right for every instance: the parts that
/// differ become `_`. A function's outermost frame still gets its
/// instance's own name from the name section.
fn common_name(a: &str, b: &str) -> String {
    let shared: usize = a
        .chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x.len_utf8())
        .sum();
    let suffix = &a[a.len() - shared..];
    if let Some(i) = suffix.find(" as ") {
        return format!("<_{}", &suffix[i..]);
    }
    match suffix.find("::") {
        Some(i) => format!("_{}", &suffix[i..]),
        None => a.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shims_have_their_function_name() {
        assert_eq!(
            strip_generics("<F as core::ops::function::FnOnce<()>>::call_once::{shim:vtable#0}"),
            "<F as core::ops::function::FnOnce>::call_once"
        );
        assert_eq!(
            strip_generics("core::ops::function::FnOnce::call_once{{vtable.shim}}"),
            "core::ops::function::FnOnce::call_once"
        );
        assert_eq!(strip_generics("a::b::{closure#0}"), "a::b::{closure#0}");
    }

    #[test]
    fn copies_with_different_self_types_share_a_name() {
        assert_eq!(
            common_name(
                "<a::A as core::ops::FnMut>::call_mut",
                "<b::B as core::ops::FnMut>::call_mut"
            ),
            "<_ as core::ops::FnMut>::call_mut"
        );
        assert_eq!(common_name("a::x::f", "b::x::f"), "_::x::f");
        assert_eq!(common_name("f", "g"), "f");
    }
}
