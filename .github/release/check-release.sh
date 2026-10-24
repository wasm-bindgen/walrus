#!/usr/bin/env bash
# Fails unless the staged changes against $1 are ones a release commit may
# make: the version bumps and the changelog.
set -euo pipefail

base=$1
bad=0
fail() {
  echo "::error::$1"
  bad=1
}

# Fails if the file's diff changes any line besides those matching $2.
only_lines() {
  if git diff --cached -U0 "$base" -- "$1" | grep -E '^[-+]' | grep -vE '^(\+\+\+|---) ' | grep -vE "^[-+]($2)\$"; then
    fail "$1 has changes besides $3"
  fi
}

while IFS= read -r path; do
  case "$path" in
    CHANGELOG.md) ;;
    Cargo.toml)
      only_lines "$path" "version = \"[0-9.]+\"|walrus-macro = \{ path = './crates/macro', version = '=[0-9.]+' \}" 'the versions' ;;
    crates/macro/Cargo.toml)
      only_lines "$path" 'version = "[0-9.]+"' 'the version' ;;
    *) fail "Unexpected change to $path" ;;
  esac
done < <(git diff --cached --name-only --no-renames "$base")
exit $bad
