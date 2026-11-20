//! Every `round_trip` and `valid` module, given synthesized DWARF, must keep
//! that DWARF correct through walrus, with and without gc: the default
//! DWARF (a line for every instruction, a subprogram for every function, a
//! lexical block for every block and variables for every local) and DWARF
//! from a few fixed seeds, which vary the forms and ranges as the `dwarf`
//! fuzz target does.

use anyhow::Context;
use arbitrary::Unstructured;
use std::path::Path;
use walrus_tests_utils::dwarf;

/// The seeds tried for each module, besides the default DWARF.
const SEEDS: u64 = 32;

/// Deterministic bytes for a seed.
fn bytes(seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..4096)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn check(input: &[u8], gc: bool) -> anyhow::Result<dwarf::Stats> {
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(input)?;
    if gc {
        walrus::passes::gc::run(&mut module);
    }
    let output = module.emit_wasm();
    dwarf::check_readable(&output)?;
    dwarf::check_preserved(input, &output)
}

fn run(wat: &Path) -> anyhow::Result<()> {
    let wasm = wat::parse_file(wat)?;
    let input = dwarf::synthesize(&wasm, &mut Unstructured::new(&[]))?;
    for gc in [false, true] {
        let stats = check(&input, gc).with_context(|| format!("default DWARF, gc: {gc}"))?;
        anyhow::ensure!(
            stats.with_line == stats.instructions,
            "only {} of {} instructions have a line",
            stats.with_line,
            stats.instructions
        );
    }
    for seed in 0..SEEDS {
        let data = bytes(seed);
        let mut u = Unstructured::new(&data);
        let gc = u.arbitrary::<bool>()?;
        let input = dwarf::synthesize(&wasm, &mut u)?;
        check(&input, gc).with_context(|| format!("seed {seed}, gc: {gc}"))?;
    }
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/round_trip.rs"));
include!(concat!(env!("OUT_DIR"), "/valid.rs"));
