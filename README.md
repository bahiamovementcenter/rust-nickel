# nickel-embed

Embed the [Nickel](https://nickel-lang.org) configuration language in Rust programs.

`nickel-embed` aims to cover Nickel's whole embedding surface with an idiomatic, stable Rust API,
and to be the native core for Nickel bindings in other languages: its C library is here, and
each language's binding is its own package in its own repository (see
[docs/bindings.md](docs/bindings.md)).

Its core API keeps the names of Nickel's official [`nickel-lang`](https://crates.io/crates/nickel-lang)
crate, from which it was derived, and runs on the latest `nickel-lang-core` (0.19). Later releases
add what `nickel-lang` lacks: evaluating files and other input formats, overrides, contracts,
queries, typechecking and structured diagnostics. Proven pieces will be offered upstream.

```rust
use nickel_embed::Context;

#[derive(serde::Deserialize)]
struct Server { port: u16, name: String }

let mut ctx = Context::new();
let server: Server = ctx
    .eval_deep(r#"{ port = 80, name = "my" ++ "server" } | { port | Number, name | String }"#)?
    .to_serde()?;
```

## Status

All five phases are done.

- **Phase 1:** parity with `nickel-lang` 2.2 (`Context`, `Expr`, typed accessors, JSON/YAML/TOML
  output, `serde` decoding, trace output, import paths, error formats), plus `Expr::kind()`,
  `Debug` and `Display` for expressions, and `std::error::Error` for errors.
- **Phase 2:** `Program`, the embedded equivalent of the `nickel` command line tool:

```rust
use nickel_embed::{ExportFormat, Priority, Program, Source};

let mut program = Program::builder()
    .source(Source::file("server.ncl"))
    .source(Source::file("overrides.yaml"))       // several inputs are merged
    .import_paths_from_env()
    .apply_contract_file("schema.ncl")
    .override_with_priority("server.port".parse()?, "8080", Priority::Force)
    .build()?;

let json = program.export(ExportFormat::Json)?;   // nickel export
let info = program.query()?;                      // nickel query: doc, contracts, subfields
```

  Inputs can be files, in-memory code or any reader, in Nickel, JSON, YAML, TOML or plain text.
  Records expose each field's metadata (`Record::field`, `Record::fields`).

- **Phase 3:** structured errors and tooling:
  - `Error::kind()` (`Parse`, `Import`, `Typecheck`, `Contract`, `Eval`, `Export`, `Io`) and
    `Error::diagnostics()`, which gives each message with its severity, notes and labelled
    source locations (file, byte range, 1-based line and column) as plain, serializable data.
  - Warnings through a callback: `Context::with_warnings`, `ProgramBuilder::on_warning`.
  - `format(src)`, like `nickel format` (Cargo feature `format`, off by default: it pulls in
    Topiary and tree-sitter).
  - `Program::docs()`, like `nickel doc`: documented fields, plus JSON and Markdown renderings
    (Cargo feature `doc`).

- **Phase 4:** bindings for other languages.
  - [`nickel-embed-capi`](nickel-embed-capi): a C library whose header is a superset of Nickel's
    own `nickel_lang.h`. Programs built for Nickel's C API work with it unchanged, and new
    functions expose programs, field metadata, structured diagnostics, warnings, docs and
    formatting. Version tags release it prebuilt for four platforms.
  - The bindings over it: [go-ncl](https://github.com/bahiamovementcenter/go-ncl) (Go, through
    the C library), [gleam-ncl](https://github.com/bahiamovementcenter/gleam-ncl) (Gleam, with
    its own Erlang NIF) and [py-ncl](https://github.com/bahiamovementcenter/py-ncl) (Python,
    with its own PyO3 module). Each pins a rust-nickel release.

- **Phase 5:** package imports (`import foo`).
  - `PackageMap` maps package names to directories, for the program and for each package's own
    dependencies. Pass it to `ProgramBuilder::package_map` or `Context::with_package_map`. This
    needs no extra dependencies, so packages fetched by other means (Nix, vendoring) work as is.
  - With the Cargo feature `packages` (off by default: it pulls in `gix`),
    `PackageMap::from_manifest` reads a `Nickel-pkg.ncl` manifest like `nickel` does: it resolves
    versions, fetches git and index dependencies into the cache and writes `Nickel-pkg.lock`.
    `PackageMap::find_manifest` (in every build) finds the nearest manifest. Failures have
    `ErrorKind::Package`.
  - The C library has the same as `nickel_package_map_*` (with `packages` on by default there),
    and the bindings expose it in their own terms.
    Nickel's package manager is still experimental (`package-experimental` in the `nickel`
    command), so manifest and lock formats may change; it also prints a few status lines to
    standard error that can't be turned off.

```rust
use nickel_embed::{PackageMap, Program, Source};

let manifest = PackageMap::find_manifest(".").ok_or("no Nickel-pkg.ncl")?;
let mut program = Program::builder()
    .source(Source::file("main.ncl"))
    .package_map(PackageMap::from_manifest(manifest)?)
    .build()?;
```

`Context`, `Expr`, `Program` and `Error` are not `Send`: use one per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
