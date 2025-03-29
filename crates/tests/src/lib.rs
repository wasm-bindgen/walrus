//! A simple `FileCheck` clone from LLVM for our purposes
//!
//! This will parse a file looking for directives to match against some expected
//! output. Typically we'll be running tooling like `wasm2wat` and then
//! executing `CHECK` matches against the output of `wasm2wat`.
//!
//! Currently there's two possible types of checks:
//!
//! * `;; CHECK:` - start of a block of text to check for in the output. This can
//!   be optionally followed with a number of `;; NEXT:` lines which must show
//!   up in the output after the original line:
//!
//!   ```wat
//!   (module)
//!
//!   ;; CHECK: (module)
//!   ```
//!
//!   or ...
//!
//!   ```wat
//!   (module
//!     (func (export "a")))
//!
//!   ;; CHECK: (func (;0;))
//!   ```
//!
//!   or ...
//!
//!   ```wat
//!   (module
//!     (func (export "a")))
//!
//!   ;; CHECK: (func (;0;))
//!   ;; NEXT:  (export "a" (func 0))
//!   ```
//!
//! * `(; CHECK-ALL:` can be used (and terminated at the end with `;)` to match
//!   the output exhaustively. This is typically used in conjunction with
//!   `WALRUS_BLESS` below to automatically update the output of tests.
//!
//! The above two directives are typically written in-line with tests to have
//! everything in one nice location.
//!
//! It is an error to have a test file with no directives in it at all.
//!
//! ## Automatically updating tests
//!
//! If the `CHECK-ALL` directive is used, or if no directive is used in a file,
//! then the expected output can be automatically updated. By running tests with
//! `WALRUS_BLESS` in the environment:
//!
//! ```bash
//! WALRUS_BLESS=1 cargo test --all
//! ```
//!
//! the expected output of each test will be updated with the actual output of
//! the tool at hand. If a `CHECK-ALL` directive is present we'll update the
//! text after it. If a no test directive is present one will be added to the
//! end of the file.
//!
//! Note that this is somewhat experimental, so it's recommended to make liberal
//! use of git to prevent destructive edits.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub enum FileCheck {
    Exhaustive(Vec<String>, PathBuf),
    Patterns(Vec<Vec<String>>),
    None(PathBuf),
}

impl FileCheck {
    pub fn from_file(path: &Path) -> FileCheck {
        let contents = fs::read_to_string(path).expect("should read file to string OK");
        let mut patterns = vec![];
        let mut iter = contents.lines().map(str::trim);
        while let Some(line) = iter.next() {
            if line.starts_with("(; CHECK-ALL:") {
                if !patterns.is_empty() {
                    panic!("CHECK cannot be used with CHECK-ALL");
                }
                let mut pattern = Vec::new();
                for line in iter.by_ref() {
                    if line == ";)" {
                        break;
                    }
                    pattern.push(line.to_string());
                }
                if iter.next().is_some() {
                    panic!("CHECK-ALL must be at the end of the file");
                }
                return FileCheck::Exhaustive(pattern, path.to_path_buf());
            }

            if let Some(p) = line.strip_prefix(";; CHECK:") {
                patterns.push(vec![p.to_string()]);
            }
            if let Some(n) = line.strip_prefix(";; NEXT:") {
                let p = patterns
                    .last_mut()
                    .expect("NEXT should never come before CHECK");
                p.push(n.to_string());
            }
        }
        if patterns.is_empty() {
            FileCheck::None(path.to_path_buf())
        } else {
            FileCheck::Patterns(patterns)
        }
    }

    pub fn check(&self, output: &str) {
        let output_lines = output.lines().collect::<Vec<_>>();
        let bless = env::var("WALRUS_BLESS").is_ok();
        match self {
            FileCheck::Patterns(patterns) => {
                'outer: for pattern in patterns {
                    let first_line = &pattern[0];

                    let mut start = 0;

                    'inner: while let Some(pos) = output_lines[start..]
                        .iter()
                        .position(|l| matches(l, first_line))
                    {
                        start = pos + 1;
                        if output_lines[pos..].len() + 1 < pattern.len() {
                            break;
                        }
                        for (out_line, pat_line) in
                            output_lines[pos + 1..].iter().zip(&pattern[1..])
                        {
                            if !matches(out_line, pat_line) {
                                continue 'inner;
                            }
                        }

                        continue 'outer;
                    }
                    self.missing_pattern(pattern, output);
                }
            }
            FileCheck::Exhaustive(_, path) | FileCheck::None(path) if bless => {
                update_output(path, output)
            }
            FileCheck::Exhaustive(pattern, _) => {
                for (out_line, pat_line) in output_lines.iter().zip(pattern) {
                    if !matches(out_line, pat_line) {
                        self.missing_pattern(pattern, output)
                    }
                }
            }
            FileCheck::None(_) => {
                println!();
                println!("no test assertions were found in this file, but");
                println!("you can rerun tests with `WALRUS_BLESS=1` to");
                println!("automatically add assertions to this file");
                println!();
                panic!("no tests to run")
            }
        }
    }

    fn missing_pattern(&self, pattern: &[String], output: &str) -> ! {
        let pattern = pattern
            .iter()
            .enumerate()
            .map(|(i, l)| format!("    {}: {}", if i == 0 { "CHECK" } else { "NEXT" }, l,))
            .collect::<Vec<_>>()
            .join("\n");

        let output = output
            .lines()
            .map(|l| format!("    {}", l.trim_end()))
            .collect::<Vec<_>>()
            .join("\n");

        panic!(
            "\
             CHECK failed!\n\n\
             Did not find pattern\n\n\
             {}\n\n\
             in output\n\n\
             {}\n\n",
            pattern, output
        );
    }
}

fn matches(mut actual: &str, expected: &str) -> bool {
    actual = actual.trim();
    // skip a leading comment
    if actual.starts_with("(;") {
        actual = actual[actual.find(";)").unwrap() + 2..].trim();
    }
    actual.starts_with(expected.trim())
}

fn update_output(path: &Path, output: &str) {
    let contents = fs::read_to_string(path).unwrap();
    let start = contents.find("(; CHECK-ALL:").unwrap_or(contents.len());

    let mut new_output = String::new();
    for line in output.lines() {
        if !line.is_empty() {
            new_output.push_str("  ");
            new_output.push_str(line.trim_end());
        }
        new_output.push('\n');
    }
    let new = format!(
        "{}\n\n(; CHECK-ALL:\n{}\n;)\n",
        contents[..start].trim(),
        new_output.trim_end()
    );
    fs::write(path, new).unwrap();
}
