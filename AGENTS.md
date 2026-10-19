# walrus

walrus is a library for transforming WebAssembly modules. `src/module` parses
each section into the IR, `src/ir` defines function bodies (the `Instr` enum,
which `#[walrus_instr]` in `crates/macro` expands into visitor and builder
support), and `src/emit.rs` with each section's `emit` writes the module back
out. Parsing, the IR and emission must agree: a new instruction, type or
section needs parse, IR, visitor and emit support, plus handling in
`src/passes` (gc and `used`) where it references other items.

## Build & Test

The tests need [WABT](https://github.com/WebAssembly/wabt) and
[wasm-tools](https://github.com/bytecodealliance/wasm-tools) (for
`json-from-wast` in the spec tests) on `$PATH`.

```sh
cargo test --all
cargo test --features parallel
cargo test --features parallel --manifest-path crates/tests/Cargo.toml
```

## Conventions

- User-visible changes need an entry under `Unreleased` in `CHANGELOG.md`, in
  the Added, Changed or Fixed section.
- Behaviour changes should be covered by tests in `crates/tests/tests`: `.wat`
  files in `round_trip` use `CHECK` directives (updated with `WALRUS_BLESS`),
  alongside `valid`, `invalid`, `function_imports` and the spec tests.
  API-level changes are often tested in standalone `.rs` files there.
- walrus is a published library, so changes to public types and functions are
  semver-relevant and worth calling out.
- Changes to emitted bytes should keep output deterministic and valid; the
  fuzz targets in `fuzz/` exercise round-tripping.
