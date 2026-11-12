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

Phase 1 of 5: parity with `nickel-lang` 2.2 (deep, shallow and export evaluation, typed accessors,
JSON/YAML/TOML output, `serde` decoding, trace output, import paths, error formats), plus
`Expr::kind()` for matching, `Debug` for expressions, and `Display`/`std::error::Error` for errors.

`Context`, `Expr` and `Error` are not `Send`: use one `Context` per thread.

## Development

```
nix develop      # or direnv allow
cargo test
```
