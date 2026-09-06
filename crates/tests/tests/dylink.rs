use walrus::{ModuleConfig, RawCustomSection};
use wasmparser::{Parser, Payload, Validator};

fn payloads(wasm: &[u8]) -> Vec<(String, Vec<u8>)> {
    Parser::new(0)
        .parse_all(wasm)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CustomSection(section) => {
                Some((section.name().to_owned(), section.data().to_vec()))
            }
            _ => None,
        })
        .collect()
}

fn assert_dylink_first(wasm: &[u8], name: &str, data: &[u8]) {
    Validator::new().validate_all(wasm).unwrap();
    let first = Parser::new(0).parse_all(wasm).nth(1).unwrap().unwrap();
    match first {
        Payload::CustomSection(section) => {
            assert_eq!(section.name(), name);
            assert_eq!(section.data(), data);
        }
        _ => panic!("dynamic-link metadata must precede standard sections"),
    }
    assert_eq!(payloads(wasm).iter().filter(|(n, _)| n == name).count(), 1);
}

#[test]
fn dylink_metadata_survives_code_changes_and_round_trips() {
    // Memory and table reservations deliberately exceed the active segments.
    // Retain BSS/padding, alignment, needed libraries, symbol flags, runtime
    // paths, and an unknown subsection when moving the metadata.
    let data = vec![
        1, 5, 0x80, 0x40, 4, 8, 1, // memory=8192, align=16; table=8, align=2
        2, 5, 1, 3, b'd', b'e', b'p', // needed library
        3, 4, 1, 1, b'f', 1, // weak export
        4, 8, 1, 3, b'e', b'n', b'v', 1, b'g', 0x10, // undefined import
        5, 5, 1, 3, b'l', b'i', b'b', // runtime path
        127, 3, 4, 5, 6, // future/unknown subsection
    ];
    let original = wat::parse_str(
        r#"(module
            (import "env" "memory" (memory 1))
            (import "env" "__indirect_function_table" (table 8 funcref))
            (import "env" "__memory_base" (global $memory_base i32))
            (import "env" "__table_base" (global $table_base i32))
            (import "env" "g" (func))
            (func $f (export "f") (result i32) i32.const 42)
            (elem (global.get $table_base) $f)
            (data (global.get $memory_base) "data")
        )"#,
    )
    .unwrap();
    let mut module = ModuleConfig::new().parse(&original).unwrap();
    module.customs.add(RawCustomSection {
        name: "unrelated".to_owned(),
        data: vec![7, 8, 9],
    });
    module.customs.add(RawCustomSection {
        name: "dylink.0".to_owned(),
        data: data.clone(),
    });
    for (_, func) in module.funcs.iter_local_mut() {
        func.builder_mut()
            .func_body()
            .const_at(0, walrus::ir::Value::I32(0))
            .drop_at(1);
    }
    let emitted = module.emit_wasm();
    assert_dylink_first(&emitted, "dylink.0", &data);
    assert!(payloads(&emitted).contains(&("unrelated".to_owned(), vec![7, 8, 9])));

    let mut reparsed = ModuleConfig::new().parse(&emitted).unwrap();
    assert_dylink_first(&reparsed.emit_wasm(), "dylink.0", &data);
}

#[test]
fn legacy_dylink_is_also_first() {
    let original = wat::parse_str("(module (func (export \"f\")))").unwrap();
    let mut module = ModuleConfig::new().parse(&original).unwrap();
    let data = vec![0x80, 0x40, 4, 8, 1, 1, 3, b'd', b'e', b'p'];
    module.customs.add(RawCustomSection {
        name: "dylink".to_owned(),
        data: data.clone(),
    });
    assert_dylink_first(&module.emit_wasm(), "dylink", &data);
}
