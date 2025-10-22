use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

fn is_known_failing(name: &str) -> bool {
    match name {
        // Tests that require the "gc" feature.
        "tests_spec_tests_array_fill_wast"
        | "tests_spec_tests_br_on_cast_wast"
        | "tests_spec_tests_br_on_cast_fail_wast"
        | "tests_spec_tests_i31_wast"
        | "tests_spec_tests_array_new_elem_wast"
        | "tests_spec_tests_array_new_data_wast"
        | "tests_spec_tests_array_init_elem_wast"
        | "tests_spec_tests_array_copy_wast"
        | "tests_spec_tests_array_wast"
        | "tests_spec_tests_data_wast"
        | "tests_spec_tests_extern_wast"
        | "tests_spec_tests_array_init_data_wast"
        | "tests_spec_tests_ref_cast_wast"
        | "tests_spec_tests_ref_null_wast"
        | "tests_spec_tests_ref_test_wast"
        | "tests_spec_tests_ref_eq_wast"
        | "tests_spec_tests_struct_wast"
        | "tests_spec_tests_type_canon_wast"
        | "tests_spec_tests_type_subtyping_wast"
        // Tests that require typed references (ref $typeidx) from function-references proposal
        | "tests_spec_tests_br_on_non_null_wast"
        | "tests_spec_tests_br_on_null_wast"
        | "tests_spec_tests_br_table_wast"
        | "tests_spec_tests_call_ref_wast"
        | "tests_spec_tests_elem_wast"
        | "tests_spec_tests_instance_wast"
        | "tests_spec_tests_linking_wast"
        | "tests_spec_tests_local_init_wast"
        | "tests_spec_tests_ref_as_non_null_wast"
        | "tests_spec_tests_ref_is_null_wast"
        | "tests_spec_tests_ref_wast"
        | "tests_spec_tests_return_call_ref_wast"
        | "tests_spec_tests_table_sub_wast"
        | "tests_spec_tests_table_wast"
        | "tests_spec_tests_type_equivalence_wast"
        | "tests_spec_tests_type_rec_wast"
        | "tests_spec_tests_unreached_valid_wast"
        // Tests that use legacy syntax that is not supported by wasm-tools.
        | "tests_spec_tests_legacy_rethrow_wast"
        | "tests_spec_tests_legacy_throw_wast"
        | "tests_spec_tests_legacy_try_catch_wast"
        | "tests_spec_tests_legacy_try_delegate_wast"
        // Tests that require GC proposal features not yet supported.
        | "tests_spec_tests_tag_wast"  // Uses recursive types (rec)
        | "tests_spec_tests_try_table_wast"  // Uses typed refs like (ref (module 0))
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
