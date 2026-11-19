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
    /// A namespace without a name, which is a C++ anonymous namespace, is
    /// `(anonymous namespace)`, as `llvm-cxxfilt` writes it.
    fn qualify(&self, namespaces: &[String], name: &str) -> String {
        let namespaces: Vec<&str> = namespaces
            .iter()
            .map(|namespace| match namespace.as_str() {
                "" => "(anonymous namespace)",
                namespace => namespace,
            })
            .collect();
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

    /// The path from the last named segment: compiler segments such as
    /// `{closure#0}` are in many functions, so they don't tell them apart.
    fn declaration_key<'a>(&self, name: &'a str) -> &'a str {
        let mut start = name.len();
        for segment in name.rsplit("::") {
            start -= segment.len();
            if !segment.starts_with('{') {
                break;
            }
            start = start.saturating_sub(2);
        }
        &name[start..]
    }

    fn shared_name(&self, a: &str, b: &str) -> Option<String> {
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
        if chars[i] == '<' && turbofish && chars[i..].starts_with(&['<', 'i', 'm', 'p', 'l', ' ']) {
            // A legacy `<impl Trait for Type>` path segment names the impl,
            // so it stays, without the generic arguments inside it.
            let end = closing(&chars, i);
            let inner: String = chars[i + 1..end].iter().collect();
            out.push('<');
            out.push_str(&strip_generics(&inner));
            out.push('>');
            i = end + 1;
            continue;
        }
        if chars[i] == '<' && (after_ident || turbofish) {
            i = closing(&chars, i) + 1;
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

/// The index of the `>` that closes the `<` at `start`, or the end.
fn closing(chars: &[char], start: usize) -> usize {
    let mut depth = 0;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '<' => depth += 1,
            '>' if i > 0 && chars[i - 1] == '-' => {}
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

/// The parts of a name around its first legacy `<impl …>` segment: the
/// text up to and including `<impl `, what the impl names, and the text
/// from its closing `>`.
fn impl_segment(name: &str) -> Option<(&str, &str, &str)> {
    let start = name.find("<impl ")? + "<impl ".len();
    let mut depth = 1;
    let mut previous = ' ';
    let end = name[start..]
        .char_indices()
        .find(|&(_, c)| {
            match c {
                '<' => depth += 1,
                '>' if previous != '-' => depth -= 1,
                _ => {}
            }
            previous = c;
            depth == 0
        })
        .map_or(name.len(), |(i, _)| start + i);
    Some((&name[..start], &name[start..end], &name[end..]))
}

/// `<A as T>::f` and `<B as T>::f` to `<_ as T>::f`, `<A>::f` and `<B>::f`
/// to `<_>::f`, and legacy `<impl T for A>::f` and `<impl T for B>::f` to
/// `<impl T for _>::f`, or `None` if the names differ in more than their
/// self type.
///
/// Each source function has one original scope, which all of its instances
/// share, so its name must be right for every instance: the self type that
/// differs becomes `_`. Names that differ elsewhere, such as functions that
/// one macro generates for different traits, belong to different functions.
fn common_name(a: &str, b: &str) -> Option<String> {
    if let (Some((pa, ia, ra)), Some((pb, ib, rb))) = (impl_segment(a), impl_segment(b)) {
        if pa != pb || ra != rb {
            return None;
        }
        return match (ia.rsplit_once(" for "), ib.rsplit_once(" for ")) {
            (Some((ta, _)), Some((tb, _))) if ta == tb => Some(format!("{pa}{ta} for _{ra}")),
            (None, None) => Some(format!("{pa}_{ra}")),
            _ => None,
        };
    }
    let ((_, ra), (_, rb)) = (self_type(a)?, self_type(b)?);
    (ra == rb).then(|| format!("<_{ra}"))
}

/// The self type of a name that starts with a qualified path, `<A as T>::f`
/// or `<A>::f` for an inherent impl, and the text after it. The self type
/// can be a qualified path itself, as in `<<A as X>::Out as T>::f`.
fn self_type(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix('<')?;
    let mut depth = 0;
    let mut previous = ' ';
    for (i, c) in rest.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if previous != '-' && depth == 0 => return Some(rest.split_at(i)),
            '>' if previous != '-' => depth -= 1,
            ' ' if depth == 0 && rest[i..].starts_with(" as ") => return Some(rest.split_at(i)),
            _ => {}
        }
        previous = c;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_namespaces_have_a_name() {
        let namespaces = ["a".to_owned(), String::new()];
        assert_eq!(
            RustNames.qualify(&namespaces, "helper"),
            "a::(anonymous namespace)::helper"
        );
    }

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
            )
            .as_deref(),
            Some("<_ as core::ops::FnMut>::call_mut")
        );
        assert_eq!(
            common_name("<<A as X>::Out as T>::f", "<<B as X>::Out as T>::f").as_deref(),
            Some("<_ as T>::f")
        );
    }

    #[test]
    fn functions_that_differ_in_more_than_their_self_type_dont() {
        let cases = [
            // Both from `const_eval_select!`.
            (
                "<*const _>::is_null::runtime",
                "core::slice::index::slice_index_fail::do_panic::runtime",
            ),
            (
                "<u64 as core::fmt::LowerHex>::fmt",
                "<u64 as core::fmt::UpperHex>::fmt",
            ),
            (
                "<&core::panic::panic_info::PanicMessage as core::fmt::Display>::fmt",
                "<&dyn core::fmt::Debug as core::fmt::Debug>::fmt",
            ),
            ("a::x::f", "b::x::f"),
            ("f", "g"),
            (
                "core::fmt::num::<impl core::fmt::LowerHex for u64>::fmt",
                "core::fmt::num::<impl core::fmt::UpperHex for u64>::fmt",
            ),
        ];
        for (a, b) in cases {
            assert_eq!(common_name(a, b), None, "{a} and {b}");
        }
    }

    #[test]
    fn closures_of_different_functions_have_different_keys() {
        let key = |name| RustNames.declaration_key(name);
        assert_eq!(key("worker::fetch::{closure#0}"), "fetch::{closure#0}");
        assert_eq!(
            key("worker::__wasm_bindgen_generated_fetch::{closure#0}"),
            "__wasm_bindgen_generated_fetch::{closure#0}"
        );
        assert_eq!(
            key("worker::fetch::{closure#0}::{closure#0}"),
            "fetch::{closure#0}::{closure#0}"
        );
        assert_eq!(key("<_ as core::ops::FnMut>::call_mut"), "call_mut");
        assert_eq!(
            key("<a::A as b::T>::f::{constant#0}::{closure#1}"),
            "f::{constant#0}::{closure#1}"
        );
        assert_eq!(key("a::f::{{closure}}"), "f::{{closure}}");
        assert_eq!(key("{closure#0}"), "{closure#0}");
    }

    #[test]
    fn legacy_impl_segments_keep_the_impl() {
        let display = "core::fmt::num::imp::<impl core::fmt::Display for u32>::fmt";
        assert_eq!(strip_generics(display), display);
        assert_eq!(
            strip_generics("<impl core::fmt::Debug for alloc::vec::Vec<T,A>>::fmt"),
            "<impl core::fmt::Debug for alloc::vec::Vec>::fmt"
        );
        assert_eq!(
            strip_generics("core::num::<impl u32>::checked_add"),
            "core::num::<impl u32>::checked_add"
        );
        assert_eq!(strip_generics("a::f::<u8>"), "a::f");
        assert_eq!(
            common_name(display, &display.replace("u32", "u64")).as_deref(),
            Some("core::fmt::num::imp::<impl core::fmt::Display for _>::fmt")
        );
        assert_eq!(
            common_name(
                "core::num::<impl u32>::checked_add",
                "core::num::<impl u64>::checked_add"
            )
            .as_deref(),
            Some("core::num::<impl _>::checked_add")
        );
    }

    #[test]
    fn inherent_impls_share_a_name() {
        assert_eq!(
            common_name("<u32>::checked_mul", "<usize>::checked_mul").as_deref(),
            Some("<_>::checked_mul")
        );
        assert_eq!(
            common_name("<*mut u8>::add", "<*mut u16>::add").as_deref(),
            Some("<_>::add")
        );
        assert_eq!(
            common_name("<[u8]>::iter", "<[u16]>::iter").as_deref(),
            Some("<_>::iter")
        );
        assert_eq!(
            common_name("<u32>::checked_mul", "<u32 as T>::checked_mul"),
            None
        );
    }
}
