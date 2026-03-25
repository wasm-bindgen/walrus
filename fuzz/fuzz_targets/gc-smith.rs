#![no_main]

#[macro_use]
extern crate libfuzzer_sys;

use arbitrary::Unstructured;
use wasm_smith::{Config, Module};

fuzz_target!(|data: &[u8]| {
    // Need enough data for wasm-smith to produce a module.
    if data.len() < 64 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Configure wasm-smith with GC enabled.
    let mut config = Config::default();
    config.gc_enabled = true;
    // Keep modules small for faster iteration.
    config.max_types = 10;
    config.max_funcs = 10;
    config.max_globals = 10;
    config.max_tables = 4;
    config.max_memories = 1;
    // Disable features we don't care about for GC testing.
    config.threads_enabled = false;
    config.simd_enabled = false;
    config.exceptions_enabled = false;

    let module = match Module::new(config, &mut u) {
        Ok(m) => m,
        Err(_) => return,
    };

    let wasm = module.to_bytes();

    // Sanity check: the generated module should validate.
    if wasmparser::validate(&wasm).is_err() {
        return;
    }

    // Round-trip through walrus: parse -> gc -> emit.
    let mut walrus_module = match walrus::Module::from_buffer(&wasm) {
        Ok(m) => m,
        Err(_) => return,
    };
    walrus::passes::gc::run(&mut walrus_module);
    let output = walrus_module.emit_wasm();

    // The output must also validate.
    if let Err(e) = wasmparser::validate(&output) {
        panic!(
            "walrus emitted invalid wasm after round-tripping a GC module: {e}\n\
             Input size: {} bytes, output size: {} bytes",
            wasm.len(),
            output.len()
        );
    }

    // Re-parse and re-emit to check determinism.
    let mut walrus_module2 =
        walrus::Module::from_buffer(&output).expect("walrus should parse its own output");
    let output2 = walrus_module2.emit_wasm();
    assert_eq!(output, output2, "walrus emission should be deterministic");
});
