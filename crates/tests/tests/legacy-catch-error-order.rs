//! The scan for legacy `catch` handler block types decodes every function
//! body, and with the `parallel` feature it runs in parallel. When several
//! bodies are malformed, the error reported must still be the one for the
//! first of them in function order, not whichever a thread hits first.

use wasm_encoder::{
    CodeSection, FunctionSection, Module, TagKind, TagSection, TagType, TypeSection, ValType,
};

const FUNCS: u32 = 2000;

fn module(malformed: &[u32]) -> Vec<u8> {
    let mut module = Module::new();

    let mut types = TypeSection::new();
    types.ty().function([], []);
    types.ty().function([ValType::I32], []);
    module.section(&types);

    let mut funcs = FunctionSection::new();
    for _ in 0..FUNCS {
        funcs.function(0);
    }
    module.section(&funcs);

    // A tag with params is what makes the parser scan the bodies.
    let mut tags = TagSection::new();
    tags.tag(TagType {
        kind: TagKind::Exception,
        func_type_idx: 1,
    });
    module.section(&tags);

    let mut code = CodeSection::new();
    for i in 0..FUNCS {
        // No locals, then `nop`s.
        let mut body = vec![0x00];
        body.extend([0x01; 100]);
        if malformed.contains(&i) {
            // An illegal opcode. Two bytes, so that every body is the same
            // size whichever of them are malformed.
            body.extend([0xff, 0xff]);
        } else {
            body.extend([0x01, 0x01]);
        }
        body.push(0x0b);
        code.raw(&body);
    }
    module.section(&code);

    module.finish()
}

fn error(wasm: &[u8]) -> String {
    match walrus::Module::from_buffer(wasm) {
        Ok(_) => panic!("expected the module to be invalid"),
        Err(e) => format!("{e:#}"),
    }
}

#[test]
fn first_malformed_body_is_reported() {
    let malformed = [150, FUNCS / 4, FUNCS / 2, 3 * FUNCS / 4];
    let first = error(&module(&malformed[..1]));
    let wasm = module(&malformed);
    for _ in 0..100 {
        assert_eq!(error(&wasm), first);
    }
}
