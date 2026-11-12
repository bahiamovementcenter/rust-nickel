//! Formatting Nickel source, like `nickel format`.

use std::fmt;

/// Formats Nickel source code the way `nickel format` does.
///
/// ```rust
/// let formatted = nickel_embed::format("{a=1,b=   2}").unwrap();
/// assert_eq!(formatted, "{ a = 1, b = 2 }\n");
/// ```
pub fn format(src: &str) -> Result<String, FormatError> {
    let mut out = Vec::new();
    nickel_lang_core::format::format(src.as_bytes(), &mut out)
        .map_err(|e| FormatError(e.to_string()))?;
    String::from_utf8(out).map_err(|e| FormatError(e.to_string()))
}

/// Why [`format`] failed, usually because the source doesn't parse.
///
/// Unlike [`Error`](crate::Error), this is plain text, so it is `Send` and `Sync`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError(String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}
