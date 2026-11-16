# Fuzzing Walrus with `cargo fuzz`!

## Prerequisites

```
cargo install cargo-fuzz
```

## Fuzzing

```
cargo fuzz run watgen
cargo fuzz run wasm-opt-ttf
cargo fuzz run raw
cargo fuzz run gc-smith
```

### DWARF

With `generate_dwarf`, walrus rewrites a module's DWARF to match the code it
emits. Two targets check it:

* `dwarf` gives a wasm-smith module DWARF that the rest of the input chooses
  (`walrus_tests_utils::dwarf::synthesize`), and fails if walrus's output
  tells a debugger anything different at any instruction: the line, the
  scopes covering it or where each variable is.
* `dwarf-raw` mutates modules with real DWARF, and fails if walrus panics,
  emits DWARF that can't be read or has code ranges outside functions, or
  emits differently twice. Seed it with the compiled fixtures:

```
cargo fuzz run dwarf
cargo fuzz run dwarf-raw corpus/dwarf-raw ../crates/tests/dwarf-fixtures
```

`crates/tests/tests/dwarf_synthetic.rs` runs the `dwarf` checks on every
`round_trip` and `valid` module with fixed seeds, so `cargo test` covers them
too.

## Learn More

[Learn more about `cargo fuzz` from the `rust-fuzz`
book.](https://rust-fuzz.github.io/book/cargo-fuzz.html)
