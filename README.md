# nickel-embed

Embed the [Nickel](https://nickel-lang.org) configuration language in Rust programs.

`nickel-embed` aims to cover Nickel's whole embedding surface with an idiomatic, stable Rust API,
and to be the native core for Nickel bindings in other languages (a C API and an Erlang/Gleam
NIF are planned).

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

Phase 2 of 5.

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

`Context`, `Expr`, `Program` and `Error` are not `Send`: use one per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
