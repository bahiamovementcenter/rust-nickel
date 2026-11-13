//! xD's primitives, bundled: patterns for numbering systems such as Johnny.Decimal, written in
//! Nickel, with typed Rust wrappers for rendering and parsing numbers.
//!
//! [xD](https://github.com/bahiamovementcenter/xD) describes a numbering system as a *pattern*:
//! an array of slots (fixed-width digits from an alphabet), separators, literals, an optional
//! link to another pattern's numbers, and an optional label. Its Nickel library compiles a
//! pattern into `render`, `name`, `parse` and `write` functions, the `Numeral` and `Name`
//! contracts, and the `scheme` xD's core loads.
//!
//! This module ships that library with nickel-embed, so no `.ncl` files need to be copied
//! around. The files are copies of xD's `nickel/` folder; xD's are the canonical ones (see
//! `nickel-embed/xd/SOURCE` for the commit they come from).
//!
//! - [`Context::with_xd`] makes `import "xd.ncl"`, `import "jd.ncl"` and
//!   `import "work-packages.ncl"` work in any program the context evaluates.
//! - [`Pattern`] wraps one compiled pattern with typed `render`, `name`, `parse` and `write`.
//!
//! ```rust
//! use nickel_embed::xd::{Entry, Pattern};
//!
//! let mut jd = Pattern::jd();
//! assert_eq!(jd.render(&[1, 1, 11])?, "11.11");
//! assert_eq!(jd.name(&[1, 1, 11], "Passport")?, "11.11 Passport");
//! assert_eq!(
//!     jd.parse("11.11 Passport")?,
//!     Some(Entry { address: vec![1, 1, 11], link: None, label: Some("Passport".into()) }),
//! );
//! assert_eq!(jd.parse("11.1")?, None);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Units stack with `&`, as in Nickel. Work packages linked to JD numbers:
//!
//! ```rust
//! use nickel_embed::xd::Pattern;
//!
//! let mut wp = Pattern::new(r#"((import "jd.ncl") & (import "work-packages.ncl")).work_packages"#);
//! let entry = wp.parse("W0012~11.13 Tax return")?.unwrap();
//! assert_eq!(entry.address, [12]);
//! assert_eq!(entry.link, Some(vec![1, 1, 13]));
//! assert_eq!(wp.write(&entry)?, "W0012~11.13 Tax return");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    fmt::Write as _,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf},
};

use nickel_lang_core::cache::{InputFormat as CoreInputFormat, SourcePath, normalize_path};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::Context;

/// The bundled library: each file's name, as imported, and its contents.
///
/// For a [`crate::Program`], which reads imports from disk, write these files to a folder and
/// add it with [`crate::ProgramBuilder::import_path`].
pub const FILES: &[(&str, &str)] = &[
    ("xd.ncl", include_str!("../xd/xd.ncl")),
    ("jd.ncl", include_str!("../xd/jd.ncl")),
    ("work-packages.ncl", include_str!("../xd/work-packages.ncl")),
];

/// The folder the bundled files appear to live in. Nothing is read from or written to it: the
/// files are registered in memory under these paths, and Nickel finds them there first.
fn library_dir() -> PathBuf {
    let dir = Path::new(MAIN_SEPARATOR_STR)
        .join("nickel-embed")
        .join("xd");
    // Registered under the same normalized form Nickel computes for an import, so that the
    // lookup hits the in-memory entry. Normalizing only fails when the current directory
    // can't be read, and then the root-relative path is used as is.
    normalize_path(&dir).unwrap_or(dir)
}

impl Context {
    /// Makes xD's primitives importable: `import "xd.ncl"`, `import "jd.ncl"` and
    /// `import "work-packages.ncl"`. See [`crate::xd`].
    ///
    /// The files are held in memory and searched after the importing file's own folder, like
    /// any import path, so a file of the same name next to the importing one takes precedence.
    pub fn with_xd(mut self) -> Self {
        let dir = library_dir();
        let sources = &mut self.vm_ctxt.import_resolver.sources;
        for (name, contents) in FILES {
            sources.add_string(
                SourcePath::Path(dir.join(name), CoreInputFormat::Nickel),
                (*contents).to_owned(),
            );
        }
        sources.add_import_paths(std::iter::once(dir));
        self
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
    /// A pattern from Nickel code. `xd` (the primitives) is in scope, and the bundled files can
    /// be imported:
    ///
    /// ```rust
    /// use nickel_embed::xd::Pattern;
    ///
    /// let mut boards = Pattern::new(
    ///     r#"xd.pattern [
    ///          xd.slot { name = "category", width = 2 },
    ///          xd.sep.within,
    ///          xd.slot { name = "board", width = 2, alphabet = xd.alphabets.hex },
    ///        ]"#,
    /// );
    /// assert_eq!(boards.render(&[11, 255])?, "11.FF");
    /// # Ok::<(), nickel_embed::xd::Error>(())
    /// ```
    pub fn new(nickel: impl Into<String>) -> Self {
        Self::with_context(Context::new(), nickel)
    }

    /// Like [`Self::new`], in a context of your own: with a trace output, a warning callback,
    /// or import paths for patterns kept in your own files. The bundled files are added to it.
    pub fn with_context(ctx: Context, nickel: impl Into<String>) -> Self {
        Self {
            ctx: ctx.with_xd(),
            source: nickel.into(),
        }
    }

    /// Johnny.Decimal: `10-19` for an area, `11` for a category, `11.11 Label` for an ID.
    pub fn jd() -> Self {
        Self::new(r#"(import "jd.ncl").pattern"#)
    }

    /// Work packages on their own: `W0012 Label`, a flat number space. Stack them on a system
    /// to link them to its numbers; see the [module documentation](crate::xd).
    pub fn work_packages() -> Self {
        Self::new(r#"(import "work-packages.ncl").work_packages"#)
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
            "let xd = import \"xd.ncl\" in\nlet pattern = (\n{}\n) in\n{body}",
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
