# Contributing to `walrus`

## Build

To build `walrus`, run:

```
cargo build
```

## Test

The tests rely on [WABT][] being installed on your system's `$PATH`, so make
sure you have that first.

Then run:

```
cargo test --all
```

[WABT]: https://github.com/WebAssembly/wabt

## Release

Releases are automated, with maintainers reviewing each step:

1. Run the [Release PR](https://github.com/wasm-bindgen/walrus/actions/workflows/release-pr.yml)
  workflow on `main`, choosing a patch or minor bump. It bumps the version in
  `Cargo.toml`, and `walrus-macro`'s version and pin when it changed since the
  last release. Claude then finalizes the changelog, adding entries for merged
  PRs that lack them, and the workflow opens a `Release #.#.#` PR from
  `wasm-bindgen-bot` whose description is the release's changelog section.
  Claude's notes for reviewers are posted as a comment.
1. Review the PR, pushing fixes to it as needed. If more PRs land on `main`
  first, run the workflow again, which regenerates the branch from `main` and
  discards changes pushed to it.
1. Squash-merge the PR. Once CI passes on the merge commit, the
  [Release tag](https://github.com/wasm-bindgen/walrus/actions/workflows/release-tag.yml)
  workflow tags it `#.#.#`. If CI fails, re-running the failed jobs tags it
  once it passes.
1. The tag runs the Publish workflow, which waits for approval of the
  `publish` environment, publishes `walrus-macro` if its version is new and
  then `walrus` to crates.io, and creates the GitHub release with the
  changelog section as its notes.

To release by hand, make the same changes in a PR, merge it, and push the
`#.#.#` tag once CI passes, if the Release tag workflow didn't.
