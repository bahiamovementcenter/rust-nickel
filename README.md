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

## Nickel packages (feature `package`)

`import name` of a library declared in a `Nickel-pkg.ncl` manifest, as `nickel` built with
`package-experimental` does it: the dependencies are locked (`Nickel-pkg.lock`, next to the
manifest) and fetched into Nickel's package cache, shared with the `nickel` tool.

```rust
use nickel_embed::{Context, PackageCache, Packages};

let packages = Packages::from_manifest("Nickel-pkg.ncl", &PackageCache::new()?)?;
let mut ctx = Context::new().with_packages(packages);      // or ProgramBuilder::packages
```

Package management is experimental in Nickel, and so is this feature. It pulls in `gix` and an
HTTP client to fetch git packages.

## xD's primitives (feature `xd`)

[xD](https://github.com/bahiamovementcenter/xD) publishes its Nickel library for numbering
systems such as Johnny.Decimal as the Nickel package `xd`. The `xd` feature pins one version
of it (`xd::VERSION` at `xd::COMMIT`), fetches it into Nickel's package cache on first use, and
adds typed wrappers for its `render`, `name`, `parse` and `write`:

```rust
use nickel_embed::xd::Pattern;

let mut jd = Pattern::jd()?;
assert_eq!(jd.name(&[1, 1, 11], "Passport")?, "11.11 Passport");
let entry = jd.parse("11.11 Passport")?.unwrap();       // Entry { address: [1, 1, 11], label: Some("Passport"), .. }
assert_eq!(jd.write(&entry)?, "11.11 Passport");

let mut boards = Pattern::new(r#"xd.pattern [
    xd.slot { name = "category", width = 2 }, xd.sep.within,
    xd.slot { name = "board", width = 2, alphabet = xd.alphabets.hex },
]"#)?;
assert_eq!(boards.render(&[11, 255])?, "11.FF");
```

`Context::new().with_xd()?` makes `import xd` work in any Nickel code. Units stack with `&`, as
in xD: `xd.units.jd & xd.units.work_packages`. No copy of xD's files lives in this repository:
moving to a new xD version means changing `xd::COMMIT`, `xd::TREE` and `xd::VERSION`, which the
tests check against a real fetch. To run those tests without GitHub, point
`NICKEL_EMBED_XD_REPO` at a local clone of xD that has the pinned commit.

`Context`, `Expr`, `Program` and `Error` are not `Send`: use one per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
