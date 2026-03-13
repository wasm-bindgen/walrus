use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

fn is_known_failing(name: &str) -> bool {
    match name {
        // Tests that use legacy syntax that is not supported by wasm-tools.
        "tests_spec_tests_legacy_rethrow_wast"
        | "tests_spec_tests_legacy_throw_wast"
        | "tests_spec_tests_legacy_try_catch_wast"
        | "tests_spec_tests_legacy_try_delegate_wast"
        // 64-bit memory/table offsets not yet supported in walrus.
        | "tests_spec_tests_data_wast"  // active data with non-i32 offset (memory64)
        | "tests_spec_tests_elem_wast"  // active elem with non-i32 offset (table64)
        => true,

        _ => false,
    }
}

fn for_each_wat_file<P, F>(dir: P, mut f: F)
where
    P: AsRef<Path>,
    F: FnMut(&Path),
{
    println!("cargo:rerun-if-changed={}", dir.as_ref().display());
    for entry in WalkDir::new(dir) {
        let entry = entry.unwrap();
        if entry.path().extension() == Some(OsStr::new("wat"))
            || entry.path().extension() == Some(OsStr::new("wast"))
        {
            println!("cargo:rerun-if-changed={}", entry.path().display());
            f(entry.path());
        }
    }
}

fn path_to_ident(p: &Path) -> String {
    p.display()
        .to_string()
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => c,
            _ => '_',
        })
        .collect()
}

fn generate_tests(name: &str) {
    let mut tests = String::new();

    for_each_wat_file(Path::new("tests").join(name), |path| {
        let test_name = path_to_ident(path);
        let ignore_test = if is_known_failing(&test_name) {
            "#[ignore]"
        } else {
            ""
        };
        tests.push_str(&format!(
            "#[test] {} fn {}() {{ walrus_tests_utils::handle(run({:?}.as_ref())); }}\n",
            ignore_test,
            test_name,
            path.display(),
        ));
    });

    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join(name).with_extension("rs"), &tests)
        .expect("should write generated valid.rs file OK");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=WALRUS_TESTS_DOT");

    generate_tests("valid");
    generate_tests("round_trip");
    generate_tests("spec-tests");
    generate_tests("function_imports");
    generate_tests("invalid");
}
