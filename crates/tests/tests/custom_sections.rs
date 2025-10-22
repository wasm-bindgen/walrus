//! Tests for working with custom sections that `walrus` doesn't know about.

use std::borrow::Cow;
use walrus::{CodeTransform, CustomSection, IdsToIndices, Module, ModuleConfig, ValType};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct HelloCustomSection(String);

impl HelloCustomSection {
    fn parse(data: &[u8]) -> Option<Self> {
        let data = std::str::from_utf8(data).ok()?;
        if !data.starts_with("Hello, ") || !data.ends_with("!") {
            return None;
        }
        let who = data["Hello, ".len()..data.len() - 1].to_string();
        Some(HelloCustomSection(who))
    }
}

impl CustomSection for HelloCustomSection {
    fn name(&self) -> &str {
        "hello"
    }

    fn data(&self, _: &IdsToIndices) -> Cow<'_, [u8]> {
        let data = format!("Hello, {}!", self.0);
        data.into_bytes().into()
    }
}

#[test]
fn round_trip_unkown_custom_sections() {
    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);

    let indices = IdsToIndices::default();

    let mut module = Module::with_config(config.clone());

    let world = HelloCustomSection("World".into());
    let world_id = module.customs.add(world.clone());
    assert_eq!(module.customs.get(world_id).unwrap(), &world);

    assert_eq!(
        module
            .customs
            .iter()
            .map(|(id, s)| (id, s.data(&indices)))
            .collect::<Vec<_>>(),
        [(world_id.into(), world.data(&indices))]
    );

    let wasm = module.emit_wasm();
    let mut module = config.parse(&wasm).unwrap();

    let world_round_tripped = module.customs.remove_raw("hello").unwrap();
    assert_eq!(world_round_tripped.data(&indices), world.data(&indices));

    let new_world = HelloCustomSection::parse(&world.data(&indices)).unwrap();
    assert_eq!(new_world.data(&indices), world.data(&indices));
    module.customs.add(new_world);

    let new_wasm = module.emit_wasm();
    assert_eq!(wasm, new_wasm);
}

// Insert a `(drop (i32.const 0))` at the start of the function and assert that
// all instructions are pushed down by the size of a `(drop (i32.const 0))`,
// which is 3.
#[test]
fn smoke_test_code_transform() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static APPLIED_CODE_TRANSFORM: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug)]
    struct CheckCodeTransform;
    impl CustomSection for CheckCodeTransform {
        fn name(&self) -> &str {
            "check-code-transform"
        }

        fn data(&self, _: &IdsToIndices) -> Cow<'_, [u8]> {
            vec![].into()
        }

        fn apply_code_transform(&mut self, transform: &CodeTransform) {
            APPLIED_CODE_TRANSFORM.store(1, Ordering::SeqCst);
            assert!(!transform.instruction_map.is_empty());
            for (input_offset, output_offset) in transform.instruction_map.iter() {
                assert_eq!(input_offset.data() as usize + 3, *output_offset);
            }
        }
    }

    let mut config = ModuleConfig::new();
    config.generate_producers_section(false);

    let wasm = {
        let mut module = Module::with_config(config.clone());

        let mut builder = walrus::FunctionBuilder::new(&mut module.types, &[], &[ValType::I32]);
        builder.func_body().i32_const(1337);
        let locals = vec![];
        let f_id = builder.finish(locals, &mut module.funcs);

        module.exports.add("f", f_id);

        module.emit_wasm()
    };

    config.preserve_code_transform(true);

    let mut module = config.parse(&wasm).unwrap();
    module.customs.add(CheckCodeTransform);

    for (_id, f) in module.funcs.iter_local_mut() {
        let builder = f.builder_mut();
        builder.func_body().const_at(0, walrus::ir::Value::I32(0));
        builder.func_body().drop_at(1);
    }

    // Emit the new, transformed wasm. This should trigger the
    // `apply_code_transform` method to be called.
    let _wasm = module.emit_wasm();

    assert_eq!(APPLIED_CODE_TRANSFORM.load(Ordering::SeqCst), 1);
}
