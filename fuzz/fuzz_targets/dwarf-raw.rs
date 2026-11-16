//! Any module walrus parses with `generate_dwarf`, whatever its DWARF, must
//! emit without panicking, with DWARF that can be read, the same way every
//! time. Seed it with the compiled fixtures, which it mutates:
//!
//! ```sh
//! cargo fuzz run dwarf-raw fuzz/corpus/dwarf-raw crates/tests/dwarf-fixtures
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use walrus_tests_utils::dwarf;

fn emit(wasm: &[u8]) -> Option<Vec<u8>> {
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(wasm).ok()?;
    walrus::passes::gc::run(&mut module);
    Some(module.emit_wasm())
}

fuzz_target!(|data: &[u8]| {
    let Some(output) = emit(data) else {
        return;
    };
    // walrus parses more than wasmparser's default features, such as legacy
    // exceptions.
    let mut validator = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all());
    if let Err(e) = validator.validate_all(&output) {
        panic!("walrus emitted invalid wasm: {}", e);
    }
    if let Err(e) = dwarf::check_readable(&output) {
        panic!("walrus emitted unreadable DWARF: {:?}", e);
    }
    assert_eq!(emit(data), Some(output), "emitting should be deterministic");
});
