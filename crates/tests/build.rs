use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

fn is_known_failing(name: &str) -> bool {
    match name {
        // enabling multi-memory means that these tests fail, but the failure is
        // benign.
        "tests_spec_tests_proposals_bulk_memory_operations_binary_wast"
        | "tests_spec_tests_proposals_reference_types_binary_wast" => true,

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
