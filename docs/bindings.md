# Bindings for other languages

nickel-embed is the one Nickel engine behind every language binding. This repository holds the
engine and what every language can share; each language's binding is its own package, in its own
repository, released to that language's registry.

| Repository | Holds | Published as |
| --- | --- | --- |
| [rust-nickel](https://github.com/bahiamovementcenter/rust-nickel) | `nickel-embed` (the engine) and `nickel-embed-capi` (its C library) | version tags, with prebuilt C libraries attached to each release |
| [go-ncl](https://github.com/bahiamovementcenter/go-ncl) | Go package over the C library, with the prebuilt libraries in `lib/` | Go module `github.com/bahiamovementcenter/go-ncl` |
| [gleam-ncl](https://github.com/bahiamovementcenter/gleam-ncl) | Gleam package, and its Erlang NIF (a Rust crate) | Hex package `ncl`, prebuilt NIFs on its releases |
| [py-ncl](https://github.com/bahiamovementcenter/py-ncl) | Python package, and its PyO3 extension (a Rust crate) | PyPI package `py-ncl` (`import ncl`) |

## What goes where

- **Here:** anything about Nickel itself, and anything every language can use. The C library is
  here because it's language-neutral: Go uses it through cgo, and any other language with a C
  foreign function interface can use it the same way.
- **In a language's repository:** code written for that language only, even when it's Rust. An
  Erlang NIF or a PyO3 module is shaped by its language's runtime and changes with that
  language's package, so it lives and is released there. It depends on `nickel-embed` by git tag.
- **Nowhere but here:** Nickel logic. A binding loads the engine, converts values to its
  language's types and offers idiomatic wrappers. When a binding needs something the engine
  doesn't do, it's added to `nickel-embed` (and the C library, if Go needs it) first.

## Versions

rust-nickel is released with tags `vX.Y.Z`, one version for the whole workspace (while the
crates aren't on crates.io). Each binding pins one tag:

- go-ncl records it in `RUST_NICKEL_VERSION` and commits that release's libraries to `lib/`.
- gleam-ncl and py-ncl name it in their Rust crates' `Cargo.toml` (`tag = "vX.Y.Z"`).

A binding's own version is independent: it moves when its API moves. A new rust-nickel release is
picked up by a pull request in each binding that bumps the pin.

## Release artifacts

Pushing a tag `vX.Y.Z` runs `.github/workflows/release.yml`, which builds the C library as a
static archive for each platform Go supports through cgo and attaches it to a GitHub release:

```
nickel-embed-capi-vX.Y.Z-linux_amd64.tar.gz
nickel-embed-capi-vX.Y.Z-linux_arm64.tar.gz
nickel-embed-capi-vX.Y.Z-darwin_arm64.tar.gz
nickel-embed-capi-vX.Y.Z-windows_amd64.tar.gz
SHA256SUMS
```

Each archive holds `libnickel_lang.a`, `nickel_lang.h` and `link-flags.txt` (the system libraries
the archive needs, as `rustc --print native-static-libs` lists them). Platform names follow Go's
`GOOS_GOARCH`. The Windows archive is built for the GNU toolchain, which is what cgo uses.

## xD's package

xD's `xd` Nickel package is imported through a package map in every binding, never by copying its
files: the C library's `nickel_package_map_*` functions, the NIF's package list or manifest, and
the Python package's `PackageMap`. A binding's tests import a package through a map, so this keeps
working.
