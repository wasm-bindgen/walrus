use anyhow::{bail, Context};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Once;

pub type Result<T> = std::result::Result<T, anyhow::Error>;

fn require_tool(tool: &str, repo: &str) {
    let diagnostic = format!("Could not spawn {}; do you have {} installed?", tool, repo);
    let status = Command::new(tool)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect(&diagnostic);
    assert!(status.success(), "{}", diagnostic)
}

fn require_wasm_interp() {
    require_tool("wasm-interp", "https://github.com/WebAssembly/wabt");
}

/// Run `wasm-interp` on the given wat file.
pub fn wasm_interp(path: &Path) -> Result<String> {
    static CHECK: Once = Once::new();
    CHECK.call_once(require_wasm_interp);

    let mut cmd = Command::new("wasm-interp");
    cmd.arg(path);
    cmd.arg("--run-all-exports");
    // This requires a build of WABT at least as new as `41adcbfb` to get
    // `wasm-interp`'s `--dummy-import-func`.
    cmd.arg("--dummy-import-func");
    cmd.arg("--enable-all");
    println!("running: {:?}", cmd);
    let output = cmd.output().context("could notrun wasm-interp")?;
    if !output.status.success() {
        bail!(
            "wasm-interp exited with status {:?}\n\nstderr = '''\n{}\n'''",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn require_wasm_opt() {
    require_tool("wasm-opt", "https://github.com/WebAssembly/binaryen");
}

/// Run `wasm-opt` on the given input file with optional extra arguments, and
/// return the resulting wasm binary as an in-memory buffer.
pub fn wasm_opt<A, S>(input: &Path, args: A) -> Result<Vec<u8>>
where
    A: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    static CHECK: Once = Once::new();
    CHECK.call_once(require_wasm_opt);

    let tmp = tempfile::NamedTempFile::new().unwrap();

    let mut cmd = Command::new("wasm-opt");
    cmd.arg(input);
    cmd.arg("-o");
    cmd.arg(tmp.path());
    cmd.args([
        "--enable-threads",
        "--enable-bulk-memory",
        // "--enable-reference-types",
        "--enable-simd",
    ]);
    cmd.args(args);
    println!("running: {:?}", cmd);
    let output = cmd.output().context("could not run wasm-opt")?;
    if !output.status.success() {
        bail!(
            "wasm-opt exited with status {:?}\n\nstderr = '''\n{}\n'''",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let buf = fs::read(tmp.path())?;
    Ok(buf)
}

pub fn handle<T: TestResult>(result: T) {
    result.handle();
}

pub trait TestResult {
    fn handle(self);
}

impl TestResult for () {
    fn handle(self) {}
}

impl TestResult for Result<()> {
    fn handle(self) {
        match self {
            Ok(()) => {}
            Err(e) => panic!("got an error: {:?}", e),
        }
    }
}
