#!/usr/bin/env bash
# Rebuilds the DWARF fixtures from `fixture.rs` and `fixture.c`.
#
# Needs rustc with the wasm32-unknown-unknown target, and clang with the
# wasm32 target. Both link with the rust-lld in rustc's sysroot.
set -euo pipefail
cd "$(dirname "$0")"

sysroot="$(rustc --print sysroot)"
lld="$sysroot/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/rust-lld"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Rust: only the fixture's own object, so core's debug info stays out; the
# core functions it calls are imports.
rust() {
    local opt=$1 dwarf=$2
    rustc --target wasm32-unknown-unknown --crate-type lib --emit obj -g \
        -C opt-level="$opt" -C panic=abort -C dwarf-version="$dwarf" \
        --remap-path-prefix="$PWD=." --remap-path-prefix="$sysroot=/sysroot" \
        fixture.rs -o "$tmp/rust.o"
    "$lld" -flavor wasm --no-entry --allow-undefined --export=entry --export=select \
        "$tmp/rust.o" -o "rust-O$opt-dwarf$dwarf.wasm"
}

c() {
    local opt=$1 dwarf=$2
    shift 2
    clang --target=wasm32 -nostdlib -g -gdwarf-"$dwarf" -O"$opt" \
        -fdebug-prefix-map="$PWD=." -c fixture.c -o "$tmp/c.o"
    "$lld" -flavor wasm --no-entry "$@" "$tmp/c.o" -o "c-O$opt-dwarf$dwarf$suffix.wasm"
}

rust 0 4
rust 1 4
rust 3 4
rust 2 5
suffix= c 0 4
suffix= c 2 4
suffix= c 2 5
# Keeps `unused_helper`, for walrus's gc to remove.
suffix=-no-gc-sections c 0 5 --no-gc-sections
