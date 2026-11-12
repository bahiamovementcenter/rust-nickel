//! The formatter, documentation extraction and the library version.
//!
//! The formatter and documentation are Cargo features. Without them the functions still exist
//! and fail with an explanation, so the header is the same for every build.

use std::ffi::c_char;

#[cfg(not(all(feature = "format", feature = "doc")))]
use crate::usage_error;
use crate::{finish, nickel_error, nickel_program, nickel_result, nickel_string, set_string};

/// The format of [`nickel_program_docs`]'s output.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum nickel_doc_format {
    /// Every field with its type, contracts and documentation, as JSON, like
    /// `nickel doc --format json`.
    NICKEL_DOC_FORMAT_JSON = 0,
    /// Markdown, like `nickel doc --format markdown`.
    NICKEL_DOC_FORMAT_MARKDOWN = 1,
}

/// Format Nickel source, like `nickel format`, storing the result in `out_string`.
///
/// `src` is a null-terminated, UTF-8 string. Fails if it doesn't parse, or if this library was
/// built without the formatter (the `format` Cargo feature).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_format(
    src: *const c_char,
    out_string: *mut nickel_string,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = crate::utf8(src, "the source").and_then(format);
        finish(result, out_error, |s| set_string(out_string, s))
    }
}

#[cfg(feature = "format")]
fn format(src: &str) -> Result<String, nickel_embed::Error> {
    // `FormatError` isn't a Nickel error with diagnostics, so its message becomes the error's.
    nickel_embed::format(src).map_err(|e| {
        nickel_embed::Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })
}

#[cfg(not(feature = "format"))]
fn format(_: &str) -> Result<String, nickel_embed::Error> {
    Err(usage_error(
        "this library was built without the formatter (Cargo feature `format`)".to_owned(),
    ))
}

/// Extract the documentation of the program's fields, like `nickel doc`, storing it in
/// `out_string`. Fails if the program isn't a record, or if this library was built without
/// documentation support (the `doc` Cargo feature, on by default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_docs(
    p: *mut nickel_program,
    format: nickel_doc_format,
    out_string: *mut nickel_string,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = crate::program::program_mut(p).and_then(|p| docs(p, format));
        finish(result, out_error, |s| set_string(out_string, s))
    }
}

#[cfg(feature = "doc")]
fn docs(
    p: &mut nickel_embed::Program,
    format: nickel_doc_format,
) -> Result<String, nickel_embed::Error> {
    let docs = p.docs()?;
    Ok(match format {
        nickel_doc_format::NICKEL_DOC_FORMAT_JSON => docs.to_json(),
        nickel_doc_format::NICKEL_DOC_FORMAT_MARKDOWN => docs.to_markdown(),
    }
    .to_owned())
}

#[cfg(not(feature = "doc"))]
fn docs(
    _: &mut nickel_embed::Program,
    _: nickel_doc_format,
) -> Result<String, nickel_embed::Error> {
    Err(usage_error(
        "this library was built without documentation support (Cargo feature `doc`)".to_owned(),
    ))
}

/// The version of this library, as a null-terminated string such as `"0.1.0"`. Its presence
/// tells a program it was linked against this superset of Nickel's C API.
#[unsafe(no_mangle)]
pub extern "C" fn nickel_embed_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}
