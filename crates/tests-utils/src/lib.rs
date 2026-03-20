use anyhow::{bail, Context};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Once;
use std::time::Duration;
use wait_timeout::ChildExt;

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

/// The maximum time we allow `wasm-interp` to run before killing it.
///
/// Generated wasm modules can contain infinite loops; without this bound the
/// fuzzer hangs indefinitely waiting for the child process to return, which
/// causes libFuzzer to fire its own per-unit timeout and abort the whole run.
const WASM_INTERP_TIMEOUT: Duration = Duration::from_secs(10);

/// Run `wasm-interp` on the given wasm file.
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
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    println!("running: {:?}", cmd);

    let mut child = cmd.spawn().context("could not spawn wasm-interp")?;

    // Take the pipes *before* waiting so that our end is closed (and dropped)
    // on the timeout path.  If we left them open while calling wait_timeout,
    // any grandchild process that inherited the pipe fds would keep them alive
    // and cause a subsequent read_to_string to block indefinitely even after
    // the direct child has been killed.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    // Enforce a wall-clock timeout so that wasm modules containing infinite
    // loops do not block the fuzzer indefinitely.
    let status = child
        .wait_timeout(WASM_INTERP_TIMEOUT)
        .context("could not wait for wasm-interp")?;

    let Some(status) = status else {
        // Timeout elapsed — kill the child and report the timeout.
        // Drop the pipes first so that any grandchild holding the other end
        // of the pipe does not prevent us from reaping.
        drop(stdout_pipe);
        drop(stderr_pipe);
        child
            .kill()
            .context("could not kill timed-out wasm-interp")?;
        child.wait().ok(); // reap the zombie
        bail!("wasm-interp timed out after {:?}", WASM_INTERP_TIMEOUT);
    };

    // Collect stdout/stderr now that the process has exited.
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = stdout_pipe.take() {
        out.read_to_string(&mut stdout).ok();
    }
    if let Some(mut err) = stderr_pipe.take() {
        err.read_to_string(&mut stderr).ok();
    }

    if !status.success() {
        bail!(
            "wasm-interp exited with status {:?}\n\nstderr = '''\n{}\n'''",
            status,
            stderr
        );
    }

    Ok(stdout)
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
