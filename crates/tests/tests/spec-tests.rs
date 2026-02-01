#![allow(dead_code)]
use anyhow::{bail, Context};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(serde::Deserialize, serde::Serialize)]
struct Test {
    source_filename: String,
    commands: Vec<serde_json::Value>,
}

fn run(wast: &Path) -> Result<(), anyhow::Error> {
    static INIT_LOGS: std::sync::Once = std::sync::Once::new();
    INIT_LOGS.call_once(|| {
        env_logger::init();
    });

    let proposal = wast
        .iter()
        .skip_while(|part| *part != "proposals")
        .nth(1)
        .map(|s| s.to_str().unwrap());

    let extra_args: &[&str] = match proposal {
        None => &[],
        Some("annotations") => return Ok(()),
        Some("custom-descriptors") => return Ok(()),
        Some("custom-page-sizes") => return Ok(()),
        Some("exception-handling") => &[],
        Some("extended-const") => &[],
        Some("function-references") => &[],
        Some("gc") => return Ok(()),
        Some("relaxed-simd") => &[],
        Some("tail-call") => &[],
        Some("threads") => return Ok(()),
        Some("wide-arithmetic") => return Ok(()),
        Some(other) => bail!("unknown wasm proposal: {}", other),
    };

    let tempdir = TempDir::new()?;
    let json = tempdir.path().join("foo.json");
    // Using `wasm-tools json-from-wast` instead of wabt's `wast2json`
    // because the latter is slow to support new proposals.
    let output = Command::new("wasm-tools")
        .arg("json-from-wast")
        .arg("--pretty")
        .arg(wast)
        .arg("--output")
        .arg(&json)
        .arg("--wasm-dir")
        .arg(tempdir.path())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to run `wasm-tools json-from-wast`\nstderr: {stderr}");
    }

    let contents = fs::read_to_string(&json).context("failed to read file")?;
    let test: Test = serde_json::from_str(&contents).context("failed to parse file")?;
    let mut files = Vec::new();

    let mut config = walrus::ModuleConfig::new();
    if proposal.is_none() {
        // For non-proposals tests, we only enable the stable features.
        // For proposals tests, we enable all supported features.
        config.only_stable_features(true);
    }

    let wabt_ok = run_spectest_interp(tempdir.path(), extra_args).is_ok();

    let mut should_not_parse = vec![];
    let mut non_deterministic = vec![];
    for command in test.commands {
        let filename = match command.get("filename") {
            Some(name) => name.as_str().unwrap().to_string(),
            None => continue,
        };
        // walrus only process .wasm binary files
        if filename.ends_with(".wat") {
            continue;
        }
        let line = command["line"].as_u64().unwrap();
        let path = tempdir.path().join(filename);
        match command["type"].as_str().unwrap() {
            "assert_invalid" | "assert_malformed" => {
                if proposal.is_some()
                    && ["zero byte expected", "multiple memories"]
                        .contains(&command["text"].as_str().unwrap())
                {
                    // The multi-memory proposal is enabled for all proprosals tests
                    // but some proposals tests still expect them to fail.
                    continue;
                }

                let wasm = fs::read(&path)?;
                if config.parse(&wasm).is_ok() {
                    should_not_parse.push(line);
                }
            }
            cmd => {
                // The bytes read from the original spec test case
                let bytes0 = fs::read(&path)?;
                // The module parsed from bytes0
                let mut wasm1 = config.parse(&bytes0).with_context(|| {
                    format!(
                        "error parsing wasm ({}, line {})",
                        path.to_string_lossy(),
                        line
                    )
                })?;
                // The bytes emitted from wasm1
                let bytes1 = wasm1.emit_wasm();
                fs::write(&path, &bytes1)?;
                // The module parsed from bytes1
                let mut wasm2 = config.parse(&bytes1).with_context(|| {
                    format!(
                        "error re-parsing wasm ({}, line {})",
                        path.to_string_lossy(),
                        line
                    )
                })?;
                // The bytes emitted from wasm2
                let bytes2 = wasm2.emit_wasm();

                if bytes1 != bytes2 {
                    non_deterministic.push(line);
                }
                files.push((cmd.to_string(), path.to_path_buf()));
                continue;
            }
        }
    }

    let mut message = String::new();
    if !should_not_parse.is_empty() {
        message.push_str(&format!(
            "wasm parsed when it shouldn't at line: {:?}",
            should_not_parse
        ));
    }
    if !non_deterministic.is_empty() {
        message.push_str(&format!(
            "wasm isn't deterministic at line: {:?}",
            non_deterministic
        ));
    }
    if !message.is_empty() {
        panic!("{}", message);
    }

    // If wabt didn't succeed before we ran walrus there's no hope of it passing
    // after we run walrus.
    if !wabt_ok {
        return Ok(());
    }

    // First up run the spec-tests as-is after we round-tripped through walrus.
    // This should for sure work correctly
    run_spectest_interp(tempdir.path(), extra_args)?;

    // Next run the same spec tests with semantics-preserving passes implemented
    // in walrus. Everything should continue to pass.
    for (cmd, file) in files.iter() {
        let wasm = fs::read(file)?;
        let mut module = config.parse(&wasm)?;

        // Tests which assert that they're not linkable tend to not work with
        // the gc pass because it removes things which would cause a module to
        // become unlinkable. This doesn't matter too much in the real world
        // (hopefully), so just don't gc assert_unlinkable modules. The same
        // applies to assert_uninstantiable modules due to removal of unused
        // elements and tables.
        if !matches!(cmd.as_str(), "assert_unlinkable" | "assert_uninstantiable") {
            walrus::passes::gc::run(&mut module);
        }

        let wasm = module.emit_wasm();
        fs::write(file, wasm)?;
    }

    run_spectest_interp(tempdir.path(), extra_args)?;

    Ok(())
}

fn run_spectest_interp(cwd: &Path, extra_args: &[&str]) -> Result<(), anyhow::Error> {
    let output = Command::new("spectest-interp")
        .current_dir(cwd)
        .arg("foo.json")
        .args(extra_args)
        .output()
        .context("executing `spectest-interp`")?;

    // If the interpreter exits with success it may still have failed some
    // tests. Check the output for `X/Y tests passed.` and make sure `X` equals
    // `Y`.
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().find(|l| l.ends_with("tests passed.")) {
            let part = line.split_whitespace().next().unwrap();
            let mut parts = part.split("/");
            let a = parts.next().unwrap().parse::<u32>();
            let b = parts.next().unwrap().parse::<u32>();
            if a == b {
                return Ok(());
            }
        }
    }
    println!("status: {}", output.status);
    println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    bail!("failed");
}

include!(concat!(env!("OUT_DIR"), "/spec-tests.rs"));
