# nickel-embed-capi

A C library for embedding [Nickel](https://nickel-lang.org), built on `nickel-embed`. Its header,
[`include/nickel_lang.h`](include/nickel_lang.h), is a superset of Nickel's own C API (the
`nickel_lang.h` from `nickel-lang` 2.2 that go-nickel vendors): every declaration in that header is
here unchanged, so existing programs work when linked against this library instead, and new
functions expose what `nickel-embed` adds.

The library is named `nickel_lang` (`libnickel_lang.a`, `libnickel_lang.so`, `nickel_lang.dll`),
like Nickel's, so it can replace the archive go-nickel vendors without changes to go-nickel. All
eight of go-nickel's tests pass that way.

## What it adds

| Area | Functions |
| :-- | :-- |
| Programs, like the `nickel` CLI: several sources (files or in-memory, in Nickel, JSON, YAML, TOML or text) merged, import paths, overrides with priorities, applied contracts, a focused field | `nickel_program_builder_*` |
| Evaluate (deep, for export, shallow, record spine), export to JSON/YAML/YAML documents/TOML/text, query, typecheck (walk or enforce), parse | `nickel_program_*` |
| Field metadata: documentation, type annotation, contracts, merge priority, optional, not_exported | `nickel_program_query`, `nickel_field_info_*`, `nickel_record_field_metadata_by_*`, `nickel_field_metadata_*` |
| Error kinds and structured diagnostics: severity, message, code, notes, labels with source name, byte range, line and column | `nickel_error_get_kind`, `nickel_error_diagnostic*`, `nickel_diagnostic_*`, `nickel_label_*` |
| Package imports (`import foo`): package maps built from directories or from a `Nickel-pkg.ncl` manifest (resolving and fetching dependencies, writing the lock file), on a context or a program | `nickel_package_map_*`, `nickel_context_set_package_map`, `nickel_program_builder_set_package_map` |
| Warnings through a callback | `nickel_context_set_warning_callback`, `nickel_program_builder_set_warning_callback` |
| Import paths on a context | `nickel_context_add_import_path` |
| `nickel doc` and `nickel format` | `nickel_program_docs`, `nickel_format` |
| Library version | `nickel_embed_version` |

The header's comments document each function. Everything follows the conventions of Nickel's C
API: objects are created by `nickel_*_alloc` and freed by `nickel_*_free`, results are written into
objects the caller allocated, fallible functions return `nickel_result` and store the error in an
optional `nickel_error`, and strings come back as a pointer and a length (not null-terminated).

Unlike the original functions, which abort the process when given the wrong kind of value, the new
ones report misuse (an index out of range, a string that isn't UTF-8, a program that wasn't built)
through their return value.

## Building

```
cargo build --release -p nickel-embed-capi                    # target/release/libnickel_lang.*
cargo build --release -p nickel-embed-capi --features format  # with nickel_format
```

Cargo features: `doc` and `packages` (on by default; `packages` pulls in gix to fetch git
dependencies) and `format` (off: it pulls in Topiary and tree-sitter). Without them
`nickel_program_docs`, `nickel_package_map_load_manifest` and `nickel_format` still exist and
return an error, so the header is the same for every build. Package maps built by hand work in
every build.

Linking the static library needs the system libraries Rust's standard library uses (on Linux,
`-lm -lpthread -ldl`; `cargo rustc -p nickel-embed-capi --release --crate-type staticlib -- --print native-static-libs`
lists them for any platform).

## Testing

```
cargo test -p nickel-embed-capi       # the API from Rust, and the header check
nickel-embed-capi/tests/c/run.sh      # the C tests, through the header
```

`tests/header.rs` checks that every declaration in go-nickel's header (copied in
`tests/upstream/`) appears unchanged in ours. `tests/c/upstream_tests.c` is Nickel's own C API
test; `tests/c/superset_tests.c` covers the new functions.

The header is generated with [cbindgen](https://github.com/mozilla/cbindgen) 0.29 and committed; CI
checks it is current. After changing the API:

```
cbindgen --config nickel-embed-capi/cbindgen.toml --crate nickel-embed-capi --output nickel-embed-capi/include/nickel_lang.h
```
