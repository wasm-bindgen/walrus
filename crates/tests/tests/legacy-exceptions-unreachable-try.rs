//! Regression test: a legacy `try` in unreachable code.
//!
//! The IR builder drops instructions in unreachable code, including the
//! `Try` itself, but `catch`, `catch_all` and `delegate` still tried to
//! attach a clause to it and panicked with "No Try instruction found to add
//! catch clause to" on these valid modules.

use walrus::Module;

/// Parse, emit, validate the output and re-parse it.
fn round_trip(wat: &str) {
    let wasm = wat::parse_str(wat).expect("wat should assemble");
    let mut module = Module::from_buffer(&wasm).expect("walrus should parse the module");
    let emitted = module.emit_wasm();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&emitted)
        .expect("emitted module should validate");
    Module::from_buffer(&emitted).expect("emitted module should re-parse");
}

#[test]
fn catch_after_unreachable() {
    round_trip(
        r#"(module
          (tag $e (param i32))
          (func (result i32)
            unreachable
            try (result i32)
              i32.const 1
            catch $e
            end))"#,
    );
}

#[test]
fn catch_all_after_return() {
    round_trip(
        r#"(module
          (func
            return
            try
              nop
            catch_all
            end))"#,
    );
}

#[test]
fn delegate_after_br() {
    round_trip(
        r#"(module
          (func
            block
              br 0
              try
                nop
              delegate 0
            end))"#,
    );
}

/// Several clauses with a multi-value handler signature, after `throw`.
#[test]
fn multivalue_catches_after_throw() {
    round_trip(
        r#"(module
          (tag $e (param i32))
          (func (result i32 i32)
            i32.const 0
            throw $e
            try (result i32 i32)
              i32.const 1
              i32.const 2
            catch $e
              i32.const 3
            catch_all
              i32.const 4
              i32.const 5
            end))"#,
    );
}

/// A reachable `try` nested inside a dead one keeps its clauses.
#[test]
fn reachable_try_nested_in_dead_try() {
    round_trip(
        r#"(module
          (tag $e)
          (func
            unreachable
            try
              try
                nop
              catch $e
              end
            catch_all
            end))"#,
    );
}
