#!/usr/bin/env bash
# Builds the C library, then compiles and runs the C tests against it:
# - upstream_tests.c: Nickel's own C API test (from nickel-lang 2.2), unchanged
# - superset_tests.c: the functions nickel-embed adds
#
# Usage: nickel-embed-capi/tests/c/run.sh [extra cargo build flags, e.g. --features format]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
crate="$(dirname "$(dirname "$here")")"
root="$(dirname "$crate")"
cc="${CC:-cc}"

cargo build --manifest-path "$root/Cargo.toml" -p nickel-embed-capi "$@"
lib="$root/target/debug"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT

# Link the shared library: the static one needs per-platform system libraries.
for test in upstream_tests superset_tests; do
  "$cc" -std=c11 -Wall -Werror -Wno-unused-but-set-variable \
    -I "$crate/include" -DFIXTURES="\"$root/nickel-embed/tests/fixtures\"" \
    "$here/$test.c" -L "$lib" -lnickel_lang -Wl,-rpath,"$lib" -o "$out/$test"
  echo "== $test"
  "$out/$test"
done
