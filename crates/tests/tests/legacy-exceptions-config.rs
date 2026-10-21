//! `ModuleConfig::legacy_exceptions` controls whether the legacy exception
//! handling instructions are accepted when parsing.

use walrus::{Module, ModuleConfig};

const LEGACY_TRY: &str = r#"(module
  (tag $e (param i32))
  (func (result i32)
    try (result i32)
      i32.const 0
    catch $e
    end))"#;

const TRY_TABLE: &str = r#"(module
  (tag $e (param i32))
  (func (result i32)
    block $h (result i32)
      try_table (catch $e $h)
      end
      i32.const 0
    end))"#;

fn parse(wat: &str, legacy_exceptions: bool) -> anyhow::Result<Module> {
    let wasm = wat::parse_str(wat)?;
    let mut config = ModuleConfig::new();
    config.legacy_exceptions(legacy_exceptions);
    config.parse(&wasm)
}

#[test]
fn legacy_exceptions_on_by_default() {
    let wasm = wat::parse_str(LEGACY_TRY).unwrap();
    Module::from_buffer(&wasm).expect("legacy try should parse by default");
}

#[test]
fn legacy_exceptions_disabled_rejects_try() {
    let err = parse(LEGACY_TRY, false).expect_err("legacy try should fail validation");
    assert!(
        format!("{err:?}").contains("legacy_exceptions feature required"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn legacy_exceptions_disabled_accepts_try_table() {
    let enabled = parse(TRY_TABLE, true).unwrap().emit_wasm();
    let disabled = parse(TRY_TABLE, false).unwrap().emit_wasm();
    assert_eq!(enabled, disabled);
}
