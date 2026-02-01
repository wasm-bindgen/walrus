//! Fuzz testing for `walrus`.

#![deny(missing_docs)]

use anyhow::Context;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::cmp;
use std::fmt;
use std::fs;
use std::marker::PhantomData;
use std::path::Path;
use std::time;
use walrus_tests_utils::wasm_interp;

/// `Ok(T)` or a `Err(anyhow::Error)`
pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Copy, Clone, Debug)]
enum ValType {
    I32,
}

/// Anything that can generate WAT test cases for fuzzing.
pub trait TestCaseGenerator {
    /// The name of this test case generator.
    const NAME: &'static str;

    /// Generate a string of WAT deterministically using the given RNG and fuel.
    fn generate(rng: &mut impl Rng, fuel: usize) -> String;
}

/// Configuration for fuzzing.
pub struct Config<G, R>
where
    G: TestCaseGenerator,
    R: Rng,
{
    _generator: PhantomData<G>,
    rng: R,
    fuel: usize,
    timeout: u64,
    scratch: tempfile::NamedTempFile,
}

impl<G, R> Config<G, R>
where
    G: TestCaseGenerator,
    R: Rng,
{
    /// The default fuel level.
    pub const DEFAULT_FUEL: usize = 64;

    /// The default timeout (in seconds).
    pub const DEFAULT_TIMEOUT_SECS: u64 = 5;

    /// Construct a new fuzzing configuration.
    pub fn new(rng: R) -> Config<G, R> {
        static INIT_LOGS: std::sync::Once = std::sync::Once::new();
        INIT_LOGS.call_once(|| {
            env_logger::init();
        });

        let fuel = Self::DEFAULT_FUEL;
        let timeout = Self::DEFAULT_TIMEOUT_SECS;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..") // pop `fuzz`
            .join("..") // pop `crates`
            .join("target")
            .join("walrus-fuzz");
        fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("should create directory: {:?}", dir));

        let scratch = tempfile::NamedTempFile::new_in(dir).expect("should create temp file OK");

        Config {
            _generator: PhantomData,
            rng,
            fuel,
            timeout,
            scratch,
        }
    }

    /// Set the fuel level.
    ///
    /// `fuel` must be greater than zero.
    pub fn set_fuel(mut self, fuel: usize) -> Config<G, R> {
        assert!(fuel > 0);
        self.fuel = fuel;
        self
    }

    fn gen_wat(&mut self) -> String {
        G::generate(&mut self.rng, self.fuel)
    }

    fn wat2wasm(&self, wat: &str) -> Result<Vec<u8>> {
        Ok(wat::parse_str(wat)?)
    }

    fn interp(&self, wasm: &[u8]) -> Result<String> {
        fs::write(self.scratch.path(), wasm).context("failed to write to scratch file")?;
        wasm_interp(self.scratch.path())
    }

    fn round_trip_through_walrus(&self, wasm: &[u8]) -> Result<Vec<u8>> {
        let mut module =
            walrus::Module::from_buffer(wasm).context("walrus failed to parse the wasm buffer")?;
        walrus::passes::gc::run(&mut module);
        let buf = module.emit_wasm();
        Ok(buf)
    }

    fn test_wat(&self, wat: &str) -> Result<()> {
        let wasm = self.wat2wasm(wat)?;
        let expected = self.interp(&wasm)?;

        let walrus_wasm = self.round_trip_through_walrus(&wasm)?;
        let actual = self.interp(&walrus_wasm)?;

        if expected == actual {
            return Ok(());
        }

        Err(FailingTestCase {
            generator: G::NAME,
            wat: wat.to_string(),
            expected,
            actual,
        }
        .into())
    }

    /// Generate a single wasm file and then compare its output in the reference
    /// interpreter before and after round tripping it through `walrus`.
    ///
    /// Does not attempt to reduce any failing test cases.
    pub fn run_one(&mut self) -> Result<()> {
        let wat = self.gen_wat();
        self.test_wat(&wat)
            .with_context(|| format!("wat = {}", wat))?;
        Ok(())
    }

    /// Generate and test as many wasm files as we can within the configured
    /// timeout budget.
    ///
    /// Returns the reduced failing test case, if any.
    pub fn run(&mut self) -> Result<()> {
        let start = time::Instant::now();
        let timeout = time::Duration::from_secs(self.timeout);
        let mut failing = Ok(());
        loop {
            // Used all of our time, and didn't find any failing test cases.
            if start.elapsed() > timeout {
                return Ok(());
            }

            match self.run_one() {
                Ok(()) => {
                    // We reduced fuel as far as we could, so return the last
                    // failing test case.
                    #[allow(clippy::question_mark)]
                    if failing.is_err() {
                        return failing;
                    }

                    // This did not produce a failing test case, so generate a
                    // new one.
                    continue;
                }

                Err(e) => {
                    print_err(&e);
                    failing = Err(e);

                    // If we can try and reduce this test case with another
                    // iteration but with smaller fuel, do that. Otherwise
                    // return the failing test case.
                    if self.fuel > 1 {
                        self.fuel -= (self.fuel / 10).max(1);
                    } else {
                        return failing;
                    }
                }
            }
        }
    }
}

/// A failing wasm test case where round tripping the wasm through walrus
/// produces an observably different execution in the reference interpreter.
#[derive(Clone, Debug)]
pub struct FailingTestCase {
    /// The WAT disassembly of the wasm test case.
    pub wat: String,

    /// The reference interpeter's output while interpreting the wasm *before* it
    /// has been round tripped through `walrus`.
    pub expected: String,

    /// The reference interpeter's output while interpreting the wasm *after* it
    /// has been round tripped through `walrus`.
    pub actual: String,

    /// The test case generator that created this failing test case.
    pub generator: &'static str,
}

impl fmt::Display for FailingTestCase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "\
Found a failing test case!

{wat}

BEFORE round tripping through walrus:

{before}

AFTER round tripping through walrus:

{after}

Here is a standalone test case:

----------------8<----------------8<----------------8<----------------
#[test]
fn test_name() {{
    walrus_fuzz::assert_round_trip_execution_is_same(\"\\
{wat}\");
}}
----------------8<----------------8<----------------8<----------------
",
            wat = self.wat,
            before = self.expected,
            after = self.actual,
        )
    }
}

impl std::error::Error for FailingTestCase {}

/// Assert that the given WAT has the same execution trace before and after
/// round tripping it through walrus.
pub fn assert_round_trip_execution_is_same(wat: &str) {
    let config = Config::<WasmOptTtf, SmallRng>::new(SmallRng::seed_from_u64(0));
    if let Err(e) = config.test_wat(wat) {
        print_err(&e);
        panic!("round trip execution is not the same!");
    }
}

/// A simple WAT generator.
pub struct WatGen<R: Rng> {
    rng: R,
    wat: String,
}

impl<R: Rng> TestCaseGenerator for WatGen<R> {
    const NAME: &'static str = "WatGen";

    fn generate(rng: &mut impl Rng, fuel: usize) -> String {
        let wat = String::new();
        let mut g = WatGen { rng, wat };
        g.prefix();
        g.gen_instructions(fuel);
        g.suffix();
        g.wat
    }
}

impl<R: Rng> WatGen<R> {
    fn prefix(&mut self) {
        self.wat.push_str(
            "\
(module
  (import \"host\" \"print\" (func (param i32) (result i32)))
  (func (export \"$f\")
",
        );
    }

    fn suffix(&mut self) {
        self.wat.push_str("  ))");
    }

    fn gen_instructions(&mut self, fuel: usize) {
        assert!(fuel > 0);

        let mut stack = vec![];

        for _ in 0..fuel {
            self.op(&mut stack);
            if !stack.is_empty() {
                self.instr("call 0");
            }
        }

        for _ in stack {
            self.instr("call 0");
            self.instr("drop");
        }
    }

    fn instr_imm<S, I>(&mut self, operator: impl ToString, immediates: I)
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        self.wat.push_str("    ");
        self.wat.push_str(&operator.to_string());

        for op in immediates.into_iter() {
            self.wat.push(' ');
            self.wat.push_str(op.as_ref());
        }

        self.wat.push('\n');
    }

    fn instr(&mut self, operator: impl ToString) {
        self.instr_imm(operator, None::<String>);
    }

    fn op(&mut self, stack: &mut Vec<ValType>) {
        let arity = self.rng.gen_range(0, cmp::min(3, stack.len() + 1));
        match arity {
            0 => self.op_0(stack),
            1 => self.op_1(stack.pop().unwrap(), stack),
            2 => self.op_2(stack.pop().unwrap(), stack.pop().unwrap(), stack),
            _ => unreachable!(),
        }
    }

    fn op_0(&mut self, stack: &mut Vec<ValType>) {
        match self.rng.gen_range(0, 2) {
            0 => {
                let value = self.rng.gen::<i32>().to_string();
                self.instr_imm("i32.const", Some(value));
                stack.push(ValType::I32);
            }
            1 => {
                self.instr("nop");
            }
            _ => unreachable!(),
        }
    }

    fn op_1(&mut self, _operand: ValType, stack: &mut Vec<ValType>) {
        match self.rng.gen_range(0, 2) {
            0 => {
                self.instr("drop");
            }
            1 => {
                self.instr("i32.popcnt");
                stack.push(ValType::I32);
            }
            _ => unreachable!(),
        }
    }

    fn op_2(&mut self, _a: ValType, _b: ValType, stack: &mut Vec<ValType>) {
        match self.rng.gen_range(0, 2) {
            0 => {
                self.instr("i32.add");
                stack.push(ValType::I32);
            }
            1 => {
                self.instr("i32.mul");
                stack.push(ValType::I32);
            }
            _ => unreachable!(),
        }
    }
}

/// Use `wasm-opt -ttf` to generate fuzzing test cases.
pub struct WasmOptTtf;

impl TestCaseGenerator for WasmOptTtf {
    const NAME: &'static str = "WasmOptTtf";

    fn generate(rng: &mut impl Rng, fuel: usize) -> String {
        // The wat we generated in the last iteration of the loop below, if any.
        let mut last_wat = None;

        loop {
            let input: Vec<u8> = (0..fuel).map(|_| rng.gen()).collect();

            let input_tmp = tempfile::NamedTempFile::new().expect("should create temp file OK");
            fs::write(input_tmp.path(), input).expect("should write to temp file OK");

            let wat = match walrus_tests_utils::wasm_opt(
                input_tmp.path(),
                vec!["-ttf", "--emit-text", "--disable-simd", "--disable-threads"],
            ) {
                Ok(ref w) if Some(w) == last_wat.as_ref() => {
                    // We're stuck in a loop generating the same wat that
                    // `wat2wasm` can't handle over and over. This is
                    // typically because we're using an RNG that is derived
                    // from some fuzzer's output, and it is yielding all
                    // zeros or something. Just return the most basic wat
                    // module.
                    return "(module)".to_string();
                }
                Ok(w) => w,
                Err(e) => {
                    // Sometimes `wasm-opt -ttf` fails to generate wasm
                    // modules, so we just try again with the next output
                    // from the RNG.
                    eprintln!("Warning: `wasm-opt -ttf` failed:");
                    print_err(&e);
                    continue;
                }
            };

            // Only generate programs that wat2wasm can handle.
            if let Ok(bytes) = wat::parse_bytes(&wat) {
                if wasmparser::validate(&bytes).is_ok() {
                    return String::from_utf8(wat).unwrap();
                }
            }
            eprintln!(
                "Warning: `wasm-opt -fff` generated wat that wat2wasm cannot handle; \
                 skipping. wat =\n{}",
                String::from_utf8_lossy(&wat)
            );
            last_wat = Some(wat);
        }
    }
}

/// Print a `anyhow::Error` with its chain.
pub fn print_err(e: &anyhow::Error) {
    eprintln!("Error: {:?}", e);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;

    fn get_timeout() -> Option<u64> {
        use std::str::FromStr;
        std::env::var("WALRUS_FUZZ_TIMEOUT")
            .ok()
            .and_then(|t| u64::from_str(&t).ok())
    }

    #[test]
    fn watgen_fuzz() {
        let mut config = Config::<WatGen<SmallRng>, SmallRng>::new(SmallRng::seed_from_u64(
            rand::thread_rng().gen(),
        ));
        if let Some(t) = get_timeout() {
            config.timeout = t;
        }
        if let Err(failing_test_case) = config.run() {
            print_err(&failing_test_case);
            panic!("Found a failing test case");
        }
    }

    #[test]
    fn wasm_opt_ttf_fuzz() {
        let mut config =
            Config::<WasmOptTtf, SmallRng>::new(SmallRng::seed_from_u64(rand::thread_rng().gen()));
        if let Some(t) = get_timeout() {
            config.timeout = t;
        }
        if let Err(failing_test_case) = config.run() {
            print_err(&failing_test_case);
            panic!("Found a failing test case");
        }
    }

    #[test]
    fn fuzz0() {
        super::assert_round_trip_execution_is_same(
            r#"
            (module
             (memory 1)
             (data (i32.const 0))
             (export "" (func $b))
             (func $b
               data.drop 0))
            "#,
        );
    }

    #[test]
    fn fuzz1() {
        super::assert_round_trip_execution_is_same(
            r#"
            (module
              (func (result i32) (local i32)
                local.get 0))
            "#,
        );
    }

    #[test]
    fn fuzz2() {
        // This was causing us to infinite loop in `WasmOptTtf::generate`.
        let rng = bufrng::BufRng::new(&[0]);
        let mut config = Config::<WasmOptTtf, bufrng::BufRng>::new(rng).set_fuel(1);
        if let Err(e) = config.run_one() {
            print_err(&e);
            panic!("Found an error! {}", e);
        }
    }
}
