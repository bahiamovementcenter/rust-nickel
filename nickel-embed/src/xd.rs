//! xD's primitives as a pinned Nickel package: patterns for numbering systems such as
//! Johnny.Decimal, with typed Rust wrappers for rendering and parsing numbers.
//!
//! [xD](https://github.com/bahiamovementcenter/xD) describes a numbering system as a *pattern*:
//! an array of slots (fixed-width digits from an alphabet), separators, literals, an optional
//! link to another pattern's numbers, and an optional label. Its Nickel library compiles a
//! pattern into `render`, `name`, `parse` and `write` functions, the `Numeral` and `Name`
//! contracts, and the `scheme` xD's core loads.
//!
//! xD publishes that library as the Nickel package `xd` (the `nickel/` folder of its
//! repository). This module pins one version of it, [`VERSION`] at commit [`COMMIT`], and
//! fetches it like any other Nickel git package, into Nickel's
//! package cache. It's fetched once, on first use, and read from the cache after that; no copy
//! of it lives in this crate.
//!
//! - [`Context::with_xd`] makes `import xd` work in any program the context evaluates.
//! - [`Pattern`] wraps one compiled pattern with typed `render`, `name`, `parse` and `write`.
//!
//! ```no_run
//! use nickel_embed::xd::{Entry, Pattern};
//!
//! let mut jd = Pattern::jd()?;
//! assert_eq!(jd.render(&[1, 1, 11])?, "11.11");
//! assert_eq!(jd.name(&[1, 1, 11], "Passport")?, "11.11 Passport");
//! assert_eq!(
//!     jd.parse("11.11 Passport")?,
//!     Some(Entry { address: vec![1, 1, 11], link: None, label: Some("Passport".into()) }),
//! );
//! assert_eq!(jd.parse("11.1")?, None);
//! # Ok::<(), nickel_embed::xd::Error>(())
//! ```
//!
//! Units stack with `&`, as in Nickel. Work packages linked to JD numbers:
//!
//! ```no_run
//! use nickel_embed::xd::Pattern;
//!
//! let mut wp = Pattern::new("(xd.units.jd & xd.units.work_packages).work_packages")?;
//! let entry = wp.parse("W0012~11.13 Tax return")?.unwrap();
//! assert_eq!(entry.address, [12]);
//! assert_eq!(entry.link, Some(vec![1, 1, 13]));
//! assert_eq!(wp.write(&entry)?, "W0012~11.13 Tax return");
//! # Ok::<(), nickel_embed::xd::Error>(())
//! ```
//!
//! A program with a `Nickel-pkg.ncl` manifest of its own can depend on the same version
//! directly, and use [`crate::PackageMap::from_manifest`] instead:
//!
//! ```nickel
//! dependencies = {
//!   xd = 'Git {
//!     url = "https://github.com/bahiamovementcenter/xD.git",
//!     ref = 'Commit "<COMMIT>",
//!     path = "nickel",
//!   },
//! }
//! ```

use std::fmt::Write as _;

use std::path::{Path, PathBuf};

use nickel_lang_core::{error::IOError, files::Files, identifier::Ident};
use nickel_lang_package::{
    Dependency, GitDependency, ManifestFile, config::Config, resolve, snapshot::Snapshot,
    version::SemVer,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{Context, ErrorKind, PackageMap, package::manifest::package_error};

/// The repository xD's package is fetched from.
pub const GIT_URL: &str = "https://github.com/bahiamovementcenter/xD.git";
/// The commit of [`GIT_URL`] this crate pins.
pub const COMMIT: &str = "084237c00f782e21662e87bde811f4b7421bbeda";
/// The package's folder in the repository.
pub const PATH: &str = "nickel";
/// The git tree of [`COMMIT`]: Nickel's package cache is keyed by content, so this is the
/// folder the package is found in once fetched. Checked against a real fetch by the tests.
pub const TREE: &str = "343fdc183f3b645f38c06ec807384c1c60df9863";
/// The package version at [`COMMIT`], from its `Nickel-pkg.ncl`.
pub const VERSION: &str = "0.1.0";

/// A [`PackageMap`] in which `import xd` loads xD's package at the pinned version, from
/// Nickel's package cache (such as `~/.cache/nickel`), fetching it first if it isn't there.
/// Add your own packages to it with [`PackageMap::package`].
pub fn package_map() -> Result<PackageMap, crate::Error> {
    resolve_pin(Config::new().map_err(package_error)?)
}

/// Like [`package_map`], with packages cached in `cache_dir`, like `--package-cache-dir`.
pub fn package_map_with_cache(cache_dir: impl Into<PathBuf>) -> Result<PackageMap, crate::Error> {
    resolve_pin(
        Config::new()
            .map_err(package_error)?
            .with_cache_dir(cache_dir.into()),
    )
}

/// Like [`package_map_with_cache`], fetching from `mirror` (a URL or a path to a clone of xD
/// that has [`COMMIT`]) instead of [`GIT_URL`]: offline, or behind a proxy.
pub fn package_map_from_mirror(
    cache_dir: impl Into<PathBuf>,
    mirror: &str,
) -> Result<PackageMap, crate::Error> {
    let mut config = Config::new()
        .map_err(package_error)?
        .with_cache_dir(cache_dir.into());
    config.git_replacements.insert(url(GIT_URL)?, url(mirror)?);
    resolve_pin(config)
}

fn resolve_pin(config: Config) -> Result<PackageMap, crate::Error> {
    let dir = config.git_package_dir.join(TREE).join(PATH);
    // The cache folder is named after the content's hash, so whatever is there is the pinned
    // version. Without dependencies of its own it needs nothing else, and no network.
    if dir.join("main.ncl").is_file() {
        let own = ManifestFile::from_path(dir.join("Nickel-pkg.ncl")).map_err(package_error)?;
        if own.dependencies.is_empty() {
            return Ok(PackageMap::new().package("xd", &dir));
        }
    }
    fetch(config)
}

/// Resolves the pin as a manifest with this one dependency would: fetching it, and anything
/// it depends on, without writing a lock file anywhere.
fn fetch(config: Config) -> Result<PackageMap, crate::Error> {
    let commit = nickel_lang_package::ObjectId::from_hex(COMMIT.as_bytes())
        .map_err(|e| message(format!("invalid commit `{COMMIT}`: {e}")))?;
    let dependency = Dependency::Git(GitDependency {
        url: url(GIT_URL)?,
        target: nickel_lang_git::Target::Commit(commit),
        path: PATH.into(),
    });
    let root = ManifestFile {
        parent_dir: config.cache_dir.clone(),
        name: Ident::new("nickel-embed-xd"),
        version: SemVer::new(0, 0, 0),
        minimal_nickel_version: SemVer::new(0, 0, 0),
        dependencies: [(Ident::new("xd"), dependency)].into(),
        authors: Vec::new(),
        description: String::new(),
        keywords: Vec::new(),
        license: String::new(),
    };
    let snapshot = Snapshot::new(&config, &root.parent_dir, &root).map_err(package_error)?;
    let index =
        nickel_lang_package::index::PackageIndex::shared(config.clone()).map_err(package_error)?;
    let resolution = resolve::resolve(&root, snapshot, index, config).map_err(package_error)?;
    nickel_lang_package::index::ensure_index_packages_downloaded(&resolution)
        .map_err(package_error)?;
    let map = resolution.package_map(&root).map_err(package_error)?;
    Ok(PackageMap::from_core(map))
}

fn url(u: &str) -> Result<gix::Url, crate::Error> {
    let path = Path::new(u);
    let parsed = if !u.contains("://") && path.exists() {
        gix::Url::try_from(std::path::absolute(path)?.as_path())
    } else {
        gix::Url::try_from(u)
    };
    parsed.map_err(|e| message(format!("invalid git url `{u}`: {e}")))
}

fn message(text: String) -> crate::Error {
    crate::Error {
        error: Box::new(IOError(text).into()),
        files: Files::empty(),
        kind: Some(ErrorKind::Package),
    }
}

impl Context {
    /// Makes xD's primitives importable as `import xd`: [`package_map`] in a context. It
    /// replaces any package map the context had; to have both, add yours to
    /// [`package_map`]'s.
    pub fn with_xd(self) -> Result<Self, crate::Error> {
        Ok(self.with_package_map(package_map()?))
    }
}

/// Why a [`Pattern`] call failed.
#[derive(Debug)]
pub enum Error {
    /// Nickel reported an error: a mistake in the pattern, an address that doesn't fit it, an
    /// entry its `write` refuses.
    Nickel(crate::Error),
    /// Nickel's result didn't have the expected shape, e.g. a scheme that doesn't fit the
    /// type it's read into.
    Decode(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Nickel(e) => e.fmt(f),
            Error::Decode(e) => write!(f, "unexpected result from xD's Nickel library: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Nickel(e) => Some(e),
            Error::Decode(_) => None,
        }
    }
}

impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Error::Nickel(e)
    }
}

/// A number and what goes with it, as a pattern's `parse` returns it and its `write` takes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Entry {
    /// One value per slot, from the top: `11.11` in JD is `[1, 1, 11]` (area, category, ID).
    pub address: Vec<u64>,
    /// The address of the number this one links to, in the linked pattern (`W0012~11.13` links
    /// to `[1, 1, 13]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Vec<u64>>,
    /// The free text after the number, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// One compiled xD pattern, with typed wrappers for its Nickel functions.
///
/// A pattern is a Nickel expression that evaluates to the result of `xd.pattern`. Each call
/// evaluates a small Nickel program in the pattern's own [`Context`], which keeps the bundled
/// library parsed between calls. Like [`Context`], a pattern is not `Send`.
///
/// Mistakes in the pattern itself (a slot without a name, a link that isn't after the last
/// slot) are reported as an [`Error::Nickel`] by the first call that needs that part of it.
pub struct Pattern {
    ctx: Context,
    source: String,
}

impl Pattern {
    /// A pattern from Nickel code, with `xd` (the package) in scope:
    ///
    /// ```no_run
    /// use nickel_embed::xd::Pattern;
    ///
    /// let mut boards = Pattern::new(
    ///     r#"xd.pattern [
    ///          xd.slot { name = "category", width = 2 },
    ///          xd.sep.within,
    ///          xd.slot { name = "board", width = 2, alphabet = xd.alphabets.hex },
    ///        ]"#,
    /// )?;
    /// assert_eq!(boards.render(&[11, 255])?, "11.FF");
    /// # Ok::<(), nickel_embed::xd::Error>(())
    /// ```
    ///
    /// Fetches the package into Nickel's cache if it isn't there yet.
    pub fn new(nickel: impl Into<String>) -> Result<Self, Error> {
        Ok(Self::with_context(Context::new().with_xd()?, nickel))
    }

    /// Like [`Self::new`], in a context of your own: with a trace output, a warning callback,
    /// import paths for patterns kept in your own files, or the package from another cache
    /// (`Context::new().with_package_map(xd::package_map_with_cache(dir)?)`). The context must
    /// be able to `import xd`.
    pub fn with_context(ctx: Context, nickel: impl Into<String>) -> Self {
        Self {
            ctx,
            source: nickel.into(),
        }
    }

    /// Johnny.Decimal: `10-19` for an area, `11` for a category, `11.11 Label` for an ID.
    pub fn jd() -> Result<Self, Error> {
        Self::new("xd.units.jd.pattern")
    }

    /// Work packages on their own: `W0012 Label`, a flat number space. Stack them on a system
    /// to link them to its numbers; see the [module documentation](crate::xd).
    pub fn work_packages() -> Result<Self, Error> {
        Self::new("xd.units.work_packages.work_packages")
    }

    /// The number for an address of depth 1 to the pattern's depth: `[1, 1, 11]` → `11.11`.
    /// A level written as a range renders as one: `[1]` → `10-19` in JD.
    pub fn render(&mut self, address: &[u64]) -> Result<String, Error> {
        self.call_string(&format!("render {}", array(address)))
    }

    /// The number and its label: `11.11 Passport`. An empty label gives the number alone.
    pub fn name(&mut self, address: &[u64], label: &str) -> Result<String, Error> {
        self.call_string(&format!("name {} {}", array(address), string(label)))
    }

    /// Reads a number or a name. `None` for text outside the pattern's number space.
    pub fn parse(&mut self, text: &str) -> Result<Option<Entry>, Error> {
        let expr = self.call(&format!("parse {}", string(text)))?;
        if expr.is_null() {
            return Ok(None);
        }
        Ok(Some(
            expr.to_serde().map_err(|e| Error::Decode(e.to_string()))?,
        ))
    }

    /// Writes an entry back: the inverse of [`Self::parse`].
    pub fn write(&mut self, entry: &Entry) -> Result<String, Error> {
        let mut record = format!("{{ address = {}", array(&entry.address));
        if let Some(link) = &entry.link {
            let _ = write!(record, ", link = {}", array(link));
        }
        if let Some(label) = &entry.label {
            let _ = write!(record, ", label = {}", string(label));
        }
        record.push_str(" }");
        self.call_string(&format!("write {record}"))
    }

    /// The scheme for xD's core, deserialized into any type that mirrors it (such as
    /// `xd_core::Scheme`, or [`serde_json::Value`]). Fails for patterns the core can't hold
    /// yet, such as mixed alphabets or links.
    pub fn scheme<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let expr = self.call("scheme")?;
        expr.to_serde().map_err(|e| Error::Decode(e.to_string()))
    }

    fn program(&self, body: &str) -> String {
        format!(
            "let xd = import xd in\nlet pattern = (\n{}\n) in\n{body}",
            self.source
        )
    }

    fn call(&mut self, member: &str) -> Result<crate::Expr, Error> {
        let src = self.program(&format!("pattern.{member}"));
        Ok(self.ctx.eval_deep_for_export(&src)?)
    }

    fn call_string(&mut self, member: &str) -> Result<String, Error> {
        let expr = self.call(member)?;
        expr.to_serde().map_err(|e| Error::Decode(e.to_string()))
    }
}

/// A Nickel array of integers.
fn array(values: &[u64]) -> String {
    let items: Vec<String> = values.iter().map(u64::to_string).collect();
    format!("[{}]", items.join(", "))
}

/// A Nickel string literal holding exactly `s`: no interpolation, every control character
/// escaped.
fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '%' => out.push_str("\\%"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::string;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string(r#"a"b\c%{d}"#), r#""a\"b\\c\%{d}""#);
        assert_eq!(string("x\ny\u{7}"), r#""x\ny\u{7}""#);
    }
}
