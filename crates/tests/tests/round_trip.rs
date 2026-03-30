use std::env;
use std::path::Path;

/// Strip `(@producers ...)` sections from WAT output so that the walrus
/// version string does not appear in test expectations.
fn strip_producers_section(wat: &str) -> String {
    let mut out = String::with_capacity(wat.len());
    let mut depth: i32 = 0;
    let mut in_producers = false;
    let mut producers_depth: i32 = 0;

    for line in wat.lines() {
        let trimmed = line.trim();

        if !in_producers && trimmed.starts_with("(@producers") {
            in_producers = true;
            producers_depth = depth;
            // count parens on this line
            for ch in trimmed.chars() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= producers_depth {
                in_producers = false;
            }
            continue;
        }

        if in_producers {
            for ch in trimmed.chars() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= producers_depth {
                in_producers = false;
            }
            continue;
        }

        for ch in trimmed.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }

        out.push_str(line);
        out.push('\n');
    }
    out
}

fn run(wat_path: &Path) -> Result<(), anyhow::Error> {
    static INIT_LOGS: std::sync::Once = std::sync::Once::new();
    INIT_LOGS.call_once(|| {
        env_logger::init();
    });

    let wasm = wat::parse_file(wat_path)?;
    let mut module = walrus::Module::from_buffer(&wasm)?;

    if env::var("WALRUS_TESTS_DOT").is_ok() {
        module.write_graphviz_dot(wat_path.with_extension("dot"))?;
    }

    let out_wasm_file = wat_path.with_extension("out.wasm");
    walrus::passes::gc::run(&mut module);
    module.emit_wasm_file(&out_wasm_file)?;

    let out_wat = wasmprinter::print_file(&out_wasm_file)?;
    let out_wat = strip_producers_section(&out_wat);
    let checker = walrus_tests::FileCheck::from_file(wat_path);
    checker.check(&out_wat);
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/round_trip.rs"));
