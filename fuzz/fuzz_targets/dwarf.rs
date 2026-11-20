//! A wasm-smith module, given DWARF the rest of the input chooses, must keep
//! that DWARF correct through walrus: the same lines, scopes and variable
//! locations at every instruction.

#![no_main]

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use walrus_tests_utils::dwarf;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(mut config) = u.arbitrary::<wasm_smith::Config>() else {
        return;
    };
    config.max_funcs = config.max_funcs.min(20);
    config.max_instructions = config.max_instructions.min(200);
    // Features walrus doesn't parse.
    config.threads_enabled = false;
    config.custom_page_sizes_enabled = false;
    config.memory64_enabled = false;
    let Ok(module) = wasm_smith::Module::new(config, &mut u) else {
        return;
    };
    let wasm = module.to_bytes();
    let gc = u.arbitrary::<bool>().unwrap_or(false);
    let input = dwarf::synthesize(&wasm, &mut u).expect("synthesizing DWARF");

    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let Ok(mut module) = config.parse(&input) else {
        return;
    };
    if gc {
        walrus::passes::gc::run(&mut module);
    }
    let output = module.emit_wasm();
    if let Err(e) = dwarf::check_readable(&output) {
        panic!("walrus emitted unreadable DWARF (gc: {}): {:?}", gc, e);
    }
    if let Err(e) = dwarf::check_preserved(&input, &output) {
        panic!("walrus changed what the DWARF says (gc: {}): {:?}", gc, e);
    }
});
