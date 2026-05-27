//! Repro for an emit bug: walrus picks the MVP element-segment form
//! (variant `0x00`, implicit table 0, i32 offset) for active segments
//! targeting a `table64` table, which V8 rejects with `invalid table
//! elements limits flags`. `wasmparser` accepts the malformed binary,
//! so this test asserts at the byte level on the segment-kind tag.

use walrus::{
    ConstExpr, ElementItems, ElementKind, FunctionBuilder, Module, ModuleConfig, RefType,
};

#[test]
fn table64_active_segment_round_trips() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let func_id =
        FunctionBuilder::new(&mut module.types, &[], &[]).finish(vec![], &mut module.funcs);
    module.exports.add("f", func_id);

    let table_id = module
        .tables
        .add_local(/* table64 */ true, 1, Some(1), RefType::FUNCREF);

    let elem_id = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Value(walrus::ir::Value::I64(0)),
        },
        ElementItems::Functions(vec![func_id]),
    );
    module
        .tables
        .get_mut(table_id)
        .elem_segments
        .insert(elem_id);

    let wasm = module.emit_wasm();

    let kind = first_element_segment_kind(&wasm).expect("module must have an element section");
    assert_ne!(
        kind, 0x00,
        "walrus emitted MVP element-segment form for a table64 segment; \
         engines like V8 reject this. Use the explicit-table-index form."
    );

    let module2 = config.parse(&wasm).expect("must round-trip");
    let table = module2.tables.iter().next().expect("should have a table");
    assert!(table.table64, "table64 flag must survive round-trip");
}

fn first_element_segment_kind(wasm: &[u8]) -> Option<u8> {
    let mut parser = wasmparser::Parser::new(0);
    let mut cur = wasm;
    loop {
        match parser.parse(cur, true).ok()? {
            wasmparser::Chunk::Parsed { consumed, payload } => {
                if let wasmparser::Payload::ElementSection(reader) = &payload {
                    let range = reader.range();
                    let bytes = &wasm[range.start..range.end];
                    let (_count, n) = leb128_u32(bytes);
                    return Some(bytes[n]);
                }
                if matches!(payload, wasmparser::Payload::End(_)) {
                    return None;
                }
                cur = &cur[consumed..];
            }
            _ => return None,
        }
    }
}

fn leb128_u32(bytes: &[u8]) -> (u32, usize) {
    let mut result = 0u32;
    let mut shift = 0;
    let mut i = 0;
    loop {
        let b = bytes[i];
        result |= ((b & 0x7f) as u32) << shift;
        i += 1;
        if b & 0x80 == 0 {
            return (result, i);
        }
        shift += 7;
    }
}
