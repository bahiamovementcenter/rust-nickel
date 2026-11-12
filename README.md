# nickel-embed

Embed the [Nickel](https://nickel-lang.org) configuration language in Rust programs.

`nickel-embed` aims to cover Nickel's whole embedding surface with an idiomatic, stable Rust API,
and to be the native core for Nickel bindings in other languages (an Erlang/Gleam NIF is here;
a C API is planned).

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

Phase 4 of 5.

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
  - [`nickel-embed-nif`](nickel-embed-nif): an Erlang NIF, the in-process backend of the Gleam
    package [`nickel`](https://github.com/bahiamovementcenter/gleam-nickel). Each session runs on
    its own thread, owning one `Program` and the expressions taken from it, and replies to the
    calling Erlang process with a message, so schedulers are never blocked. Expressions cross into
    Erlang as resources, which gives lazy, go-nickel-style evaluation; values convert to Gleam
    terms with exact integers. Build it with `cargo build --release -p nickel-embed-nif` or
    `nix build .#nickel-embed-nif`; `nickel-embed-nif/test/run.sh` runs its Erlang tests.
  - [`nickel-embed-capi`](nickel-embed-capi): a C library whose header is a superset of Nickel's
    own `nickel_lang.h`. Programs built for Nickel's C API, go-nickel included, work with it
    unchanged, and new functions expose programs, field metadata, structured diagnostics,
    warnings, docs and formatting.

`Context`, `Expr`, `Program` and `Error` are not `Send`: use one per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
