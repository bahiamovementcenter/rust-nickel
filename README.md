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

Phase 3 of 5.

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

## xD's primitives (feature `xd`)

The Cargo feature `xd` bundles [xD](https://github.com/bahiamovementcenter/xD)'s Nickel library
for numbering systems such as Johnny.Decimal, with typed wrappers for its `render`, `name`,
`parse` and `write`:

```rust
use nickel_embed::xd::Pattern;

let mut jd = Pattern::jd();
assert_eq!(jd.name(&[1, 1, 11], "Passport")?, "11.11 Passport");
let entry = jd.parse("11.11 Passport")?.unwrap();       // Entry { address: [1, 1, 11], label: Some("Passport"), .. }
assert_eq!(jd.write(&entry)?, "11.11 Passport");

let mut boards = Pattern::new(r#"xd.pattern [
    xd.slot { name = "category", width = 2 }, xd.sep.within,
    xd.slot { name = "board", width = 2, alphabet = xd.alphabets.hex },
]"#);
assert_eq!(boards.render(&[11, 255])?, "11.FF");
```

`Context::new().with_xd()` makes `import "xd.ncl"`, `import "jd.ncl"` and
`import "work-packages.ncl"` work in any Nickel code, without copying files: they are held in
memory. Units stack with `&`, as in xD: `(import "jd.ncl") & (import "work-packages.ncl")`.

The `.ncl` files in `nickel-embed/xd/` are copies; xD's are canonical. `nickel-embed/xd/SOURCE`
names the xD commit they come from, and `scripts/sync-xd.sh <xD clone> <commit>` copies them
again. Their own examples (`xd/tests/primitives.ncl`) run as part of `cargo test --features xd`.

`Context`, `Expr`, `Program` and `Error` are not `Send`: use one per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
