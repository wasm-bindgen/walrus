//! Modules that rustc and clang compiled with debug info must keep their
//! DWARF correct through walrus, with and without gc. The fixtures and the
//! script that builds them are in `crates/tests/dwarf-fixtures`.
//!
//! With `llvm-dwarfdump` on `$PATH`, the output must also pass its
//! `--verify`.

use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use walrus_tests_utils::dwarf;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dwarf-fixtures")
        .join(name)
}

fn dwarfdump_verify(wasm: &[u8]) -> Result<()> {
    let file = tempfile::NamedTempFile::new()?;
    std::fs::write(file.path(), wasm)?;
    let output = match Command::new("llvm-dwarfdump")
        .arg("--verify")
        .arg(file.path())
        .output()
    {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("llvm-dwarfdump isn't on $PATH; skipping --verify");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    ensure!(
        output.status.success(),
        "llvm-dwarfdump --verify failed:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

/// Runs `name` through walrus and checks the output's DWARF.
fn check(name: &str, gc: bool) -> Result<()> {
    let input = std::fs::read(fixture(name))?;
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(&input)?;
    if gc {
        walrus::passes::gc::run(&mut module);
    }
    let output = module.emit_wasm();
    dwarf::check_readable(&output)?;
    let stats = dwarf::check_preserved(&input, &output)?;
    // The fixtures have debug info for nearly all their code: only the
    // functions the linker writes, such as `__wasm_call_ctors`, have none.
    let most = stats.instructions * 9 / 10;
    ensure!(
        stats.with_line >= most && stats.in_scope >= most,
        "most instructions should have debug info: {stats:?}"
    );
    ensure!(stats.locations > 0, "no variable locations: {stats:?}");
    dwarfdump_verify(&output)
}

macro_rules! fixtures {
    ($($test:ident: $name:literal,)*) => {$(
        #[test]
        fn $test() {
            for gc in [false, true] {
                check($name, gc)
                    .with_context(|| format!("{}, gc: {gc}", $name))
                    .unwrap();
            }
        }
    )*};
}

fixtures! {
    rust_o0_dwarf4: "rust-O0-dwarf4.wasm",
    rust_o1_dwarf4: "rust-O1-dwarf4.wasm",
    rust_o3_dwarf4: "rust-O3-dwarf4.wasm",
    rust_o2_dwarf5: "rust-O2-dwarf5.wasm",
    c_o0_dwarf4: "c-O0-dwarf4.wasm",
    c_o2_dwarf4: "c-O2-dwarf4.wasm",
    c_o2_dwarf5: "c-O2-dwarf5.wasm",
    c_o0_dwarf5_no_gc_sections: "c-O0-dwarf5-no-gc-sections.wasm",
}

/// gc removes `unused_helper`, which the linker kept, and its subprogram
/// no longer covers any code.
#[test]
fn gc_removes_a_function_and_its_code_ranges() {
    let input = std::fs::read(fixture("c-O0-dwarf5-no-gc-sections.wasm")).unwrap();
    let mut config = walrus::ModuleConfig::new();
    config.generate_dwarf(true);
    let mut module = config.parse(&input).unwrap();
    assert!(module.funcs.by_name("unused_helper").is_some());
    walrus::passes::gc::run(&mut module);
    assert!(module.funcs.by_name("unused_helper").is_none());
    let output = module.emit_wasm();
    dwarf::check_readable(&output).unwrap();
    dwarf::check_preserved(&input, &output).unwrap();

    let ranges = dwarf::subprogram_ranges(&output, "unused_helper").unwrap();
    assert_eq!(ranges, Some(vec![]), "its ranges should all be tombstones");
}
