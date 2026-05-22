//! Regression test: parsing a legacy `try` whose `catch` / `catch_all`
//! handler has a multi-value signature.
//!
//! A legacy-exceptions handler block's signature is implicit in the
//! encoding (`catch <tag>` carries no blocktype). Its synthesized type
//! is `[tag params] -> [try results]`; when that pair is multi-value
//! and was never declared as a `func` type, the IR builder used to
//! panic in `local_function::context::impl_push_control`
//! ("attempted to push a control frame for an instruction sequence
//! with a type that does not exist"). `parse_local_functions` now
//! pre-registers these types, mirroring `add_entry_ty`.

use walrus::Module;

fn parse(wat: &str) -> Module {
    let wasm = wat::parse_str(wat).expect("wat should assemble");
    Module::from_buffer(&wasm).expect("walrus should parse the module")
}

/// Round-trip: parse, emit, re-parse. Ensures the synthesized handler
/// type does not break emission.
fn round_trip(wat: &str) {
    let mut module = parse(wat);
    let emitted = module.emit_wasm();
    Module::from_buffer(&emitted).expect("emitted module should re-parse");
}

/// Minimal case — `[i32] -> [i32]` handler, no GC. Tag param `i32`,
/// `try (result i32)`, so the handler signature is `[i32] -> [i32]`,
/// which the module never declares as a `func` type.
#[test]
fn try_catch_multivalue_handler_i32() {
    round_trip(
        r#"(module
          (tag $e (param i32))
          (func $f (result i32)
            try (result i32)
              i32.const 0
            catch $e
            end))"#,
    );
}

/// `catch_all` handler — signature `[] -> [try results]`. The `try`
/// here takes a param, so its own blocktype is `[i32] -> [i32 i32]`;
/// the `catch_all` handler is `[] -> [i32 i32]`, which is neither the
/// `try` blocktype nor the function type (`[] -> [i32]`) — so the
/// handler type genuinely has no pre-existing match and exercises the
/// `CatchAll` arm of the fix.
#[test]
fn try_catch_all_multivalue_handler() {
    round_trip(
        r#"(module
          (func $f (result i32)
            i32.const 7
            try (param i32) (result i32 i32)
              i32.const 9
            catch_all
              unreachable
            end
            drop))"#,
    );
}

/// GC reference types in the handler signature — `[ref null $s] ->
/// [ref null $s]`. This is the shape the bug was originally reported
/// against (a Kotlin/Wasm module); the GC types are incidental, the
/// `i32` case above is the true minimal repro.
#[test]
fn try_catch_multivalue_handler_gc_ref() {
    round_trip(
        r#"(module
          (type $s (struct (field i32)))
          (tag $e (param (ref null $s)))
          (func $f (result (ref null $s))
            try (result (ref null $s))
              ref.null $s
            catch $e
            end))"#,
    );
}

/// Two `catch` clauses on one `try` — both handlers share the same
/// `try` result types, exercising the frame-stays-on-stack path.
#[test]
fn try_multiple_catch_multivalue_handlers() {
    round_trip(
        r#"(module
          (tag $a (param i32))
          (tag $b (param i32))
          (func $f (result i32)
            try (result i32)
              i32.const 0
            catch $a
            catch $b
            end))"#,
    );
}
