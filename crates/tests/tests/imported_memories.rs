//! Tests for round-tripping imported memories, ensuring that flags like
//! `memory64` and `page_size_log2` are preserved through emit and re-parse.

use walrus::{Module, ModuleConfig};

#[test]
fn imported_memory64_round_trip() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    module.add_import_memory(
        "env",
        "memory",
        false, // shared
        true,  // memory64
        1,     // initial
        Some(65536),
        None, // page_size_log2
    );

    let wasm = module.emit_wasm();
    let mut module2 = config.parse(&wasm).expect("should parse emitted wasm");

    let mem = module2
        .memories
        .iter()
        .next()
        .expect("should have a memory");
    assert!(mem.memory64, "memory64 flag must survive round-trip");
    assert!(mem.import.is_some(), "memory should still be an import");
    assert_eq!(mem.initial, 1);
    assert_eq!(mem.maximum, Some(65536));

    let wasm2 = module2.emit_wasm();
    assert_eq!(wasm, wasm2, "round-trip should produce identical bytes");
}
