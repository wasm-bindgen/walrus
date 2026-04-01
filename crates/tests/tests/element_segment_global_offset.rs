use walrus::{
    ir::Value, ConstExpr, ConstOp, ElementItems, ElementKind, FunctionBuilder, Module,
    ModuleConfig, RefType, ValType,
};

#[test]
fn element_segment_with_global_offset() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let base_global =
        module
            .globals
            .add_local(ValType::I32, false, false, ConstExpr::Value(Value::I32(1)));
    module.exports.add("__table_base", base_global);

    let builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("f", func_id);

    let table_id = module.tables.add_local(false, 2, None, RefType::FUNCREF);

    let elem_id = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Global(base_global),
        },
        ElementItems::Functions(vec![func_id]),
    );
    module
        .tables
        .get_mut(table_id)
        .elem_segments
        .insert(elem_id);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let table2 = module2.tables.main_function_table().unwrap().unwrap();
    let segments: Vec<_> = module2.tables.get(table2).elem_segments.iter().collect();
    assert_eq!(segments.len(), 1);

    let seg = module2.elements.get(*segments[0]);
    assert!(matches!(
        seg.kind,
        ElementKind::Active {
            offset: ConstExpr::Global(_),
            ..
        }
    ));
}

#[test]
fn element_segment_with_extended_const_offset() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let base_global =
        module
            .globals
            .add_local(ValType::I32, false, false, ConstExpr::Value(Value::I32(0)));
    module.exports.add("__table_base", base_global);

    let builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add("f", func_id);

    let table_id = module.tables.add_local(false, 8, None, RefType::FUNCREF);

    let elem_id = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Extended(vec![
                ConstOp::GlobalGet(base_global),
                ConstOp::I32Const(4),
                ConstOp::I32Add,
            ]),
        },
        ElementItems::Functions(vec![func_id]),
    );
    module
        .tables
        .get_mut(table_id)
        .elem_segments
        .insert(elem_id);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let table2 = module2.tables.main_function_table().unwrap().unwrap();
    let segments: Vec<_> = module2.tables.get(table2).elem_segments.iter().collect();
    assert_eq!(segments.len(), 1);

    let seg = module2.elements.get(*segments[0]);
    assert!(matches!(
        seg.kind,
        ElementKind::Active {
            offset: ConstExpr::Extended(_),
            ..
        }
    ));
}

#[test]
fn multi_segment_table_index_no_underflow() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);
    let mut module = Module::with_config(config.clone());

    let func_a = {
        let b = FunctionBuilder::new(&mut module.types, &[], &[]);
        let id = b.finish(vec![], &mut module.funcs);
        module.exports.add("func_a", id);
        id
    };
    let func_b = {
        let b = FunctionBuilder::new(&mut module.types, &[], &[]);
        let id = b.finish(vec![], &mut module.funcs);
        module.exports.add("func_b", id);
        id
    };

    let table_id = module.tables.add_local(false, 256, None, RefType::FUNCREF);

    let seg_a = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Value(Value::I32(0)),
        },
        ElementItems::Functions(vec![func_a]),
    );
    module.tables.get_mut(table_id).elem_segments.insert(seg_a);

    let seg_b = module.elements.add(
        ElementKind::Active {
            table: table_id,
            offset: ConstExpr::Value(Value::I32(128)),
        },
        ElementItems::Functions(vec![func_b]),
    );
    module.tables.get_mut(table_id).elem_segments.insert(seg_b);

    let wasm = module.emit_wasm();
    let module2 = config.parse(&wasm).expect("round-trip parse failed");

    let found_0 = module2.get_function_table_entry(0).unwrap();
    let found_128 = module2.get_function_table_entry(128).unwrap();
    assert_ne!(found_0, found_128);
}
