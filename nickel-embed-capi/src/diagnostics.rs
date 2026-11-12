//! Structured errors and warnings: error kinds, diagnostics and their labelled locations.

use std::ffi::{c_char, c_int, c_void};

use nickel_embed::{Diagnostic, ErrorKind, Label, Severity};

use crate::{context, nickel_context, nickel_error, write_str};

/// What kind of failure a [`nickel_error`] is.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum nickel_error_kind {
    /// The `nickel_error` holds no error.
    NICKEL_ERROR_KIND_NONE = 0,
    /// The source isn't valid syntax.
    NICKEL_ERROR_KIND_PARSE = 1,
    /// An import couldn't be found or read.
    NICKEL_ERROR_KIND_IMPORT = 2,
    /// A static type error.
    NICKEL_ERROR_KIND_TYPECHECK = 3,
    /// A value broke a contract.
    NICKEL_ERROR_KIND_CONTRACT = 4,
    /// Any other evaluation failure, such as a missing field or a failed assertion.
    NICKEL_ERROR_KIND_EVAL = 5,
    /// The value can't be serialized in the requested format.
    NICKEL_ERROR_KIND_EXPORT = 6,
    /// Reading a source failed, or a function was given a bad argument (such as a string that
    /// isn't UTF-8, or a program that hasn't been built).
    NICKEL_ERROR_KIND_IO = 7,
    /// A failure with no more specific kind.
    NICKEL_ERROR_KIND_OTHER = 8,
    /// A package manifest couldn't be read or its dependencies resolved or fetched.
    NICKEL_ERROR_KIND_PACKAGE = 9,
}

/// How serious a [`nickel_diagnostic`] is.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum nickel_severity {
    /// A suggestion.
    NICKEL_SEVERITY_HELP = 0,
    /// Extra information.
    NICKEL_SEVERITY_NOTE = 1,
    /// A warning.
    NICKEL_SEVERITY_WARNING = 2,
    /// An error.
    NICKEL_SEVERITY_ERROR = 3,
    /// An internal error in Nickel itself.
    NICKEL_SEVERITY_BUG = 4,
}

/// One message about a problem, with the places in the source it points at.
///
/// Get one from [`nickel_error_diagnostic`], or in a [`nickel_warning_callback`]. It borrows
/// from where it came from: the error (until it is overwritten or freed), or the callback
/// (until it returns).
// Secretly a `nickel_embed::Diagnostic`.
pub struct nickel_diagnostic {}

/// A place in a source that a [`nickel_diagnostic`] points at. It borrows from the diagnostic.
// Secretly a `nickel_embed::Label`.
pub struct nickel_label {}

/// A callback for warnings: called with `user_data` and the warning, which is only valid during
/// the call.
pub type nickel_warning_callback =
    Option<extern "C" fn(user_data: *mut c_void, diagnostic: *const nickel_diagnostic)>;

pub(crate) fn warning_reporter(
    callback: nickel_warning_callback,
    user_data: *mut c_void,
) -> impl FnMut(Diagnostic) + 'static {
    move |d: Diagnostic| {
        if let Some(callback) = callback {
            callback(
                user_data,
                &d as *const Diagnostic as *const nickel_diagnostic,
            );
        }
    }
}

/// Call `callback` with each warning Nickel reports while evaluating in this context. Warnings
/// are discarded otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_set_warning_callback(
    ctx: *mut nickel_context,
    callback: nickel_warning_callback,
    user_data: *mut c_void,
) {
    // Safety: `ctx` came from `nickel_context_alloc`.
    let ctx = unsafe { context(ctx) };
    *ctx = std::mem::take(ctx).with_warnings(warning_reporter(callback, user_data));
}

/// Add a directory to search for imports that aren't found relative to the importing file.
///
/// `path` is a null-terminated string, copied. Like Nickel's own C API, contexts don't read
/// the `NICKEL_IMPORT_PATH` environment variable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_add_import_path(
    ctx: *mut nickel_context,
    path: *const c_char,
) {
    // Safety: `path` is a valid C string and `ctx` came from `nickel_context_alloc`.
    unsafe {
        let path = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        let ctx = context(ctx);
        *ctx = std::mem::take(ctx).with_added_import_paths(vec![path.into()]);
    }
}

unsafe fn error_ref<'a>(err: *const nickel_error) -> &'a nickel_error {
    // Safety: `err` came from `nickel_error_alloc`.
    unsafe { err.as_ref() }.expect("null nickel_error")
}

unsafe fn diagnostic<'a>(d: *const nickel_diagnostic) -> &'a Diagnostic {
    // Safety: `d` came from `nickel_error_diagnostic` or a warning callback.
    unsafe { (d as *const Diagnostic).as_ref() }.expect("null nickel_diagnostic")
}

unsafe fn label<'a>(l: *const nickel_label) -> &'a Label {
    // Safety: `l` came from `nickel_diagnostic_label`.
    unsafe { (l as *const Label).as_ref() }.expect("null nickel_label")
}

/// What kind of failure this error is, or `NICKEL_ERROR_KIND_NONE` if it holds no error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_get_kind(err: *const nickel_error) -> nickel_error_kind {
    // Safety: pre-conditions of this function.
    let Some(error) = &unsafe { error_ref(err) }.inner else {
        return nickel_error_kind::NICKEL_ERROR_KIND_NONE;
    };
    match error.kind() {
        ErrorKind::Parse => nickel_error_kind::NICKEL_ERROR_KIND_PARSE,
        ErrorKind::Import => nickel_error_kind::NICKEL_ERROR_KIND_IMPORT,
        ErrorKind::Typecheck => nickel_error_kind::NICKEL_ERROR_KIND_TYPECHECK,
        ErrorKind::Contract => nickel_error_kind::NICKEL_ERROR_KIND_CONTRACT,
        ErrorKind::Eval => nickel_error_kind::NICKEL_ERROR_KIND_EVAL,
        ErrorKind::Export => nickel_error_kind::NICKEL_ERROR_KIND_EXPORT,
        ErrorKind::Io => nickel_error_kind::NICKEL_ERROR_KIND_IO,
        ErrorKind::Package => nickel_error_kind::NICKEL_ERROR_KIND_PACKAGE,
        _ => nickel_error_kind::NICKEL_ERROR_KIND_OTHER,
    }
}

unsafe fn diagnostics<'a>(err: *const nickel_error) -> &'a [Diagnostic] {
    // Safety: pre-condition of the public functions calling this one.
    let err = unsafe { error_ref(err) };
    match &err.inner {
        Some(error) => err.diagnostics.get_or_init(|| error.diagnostics()),
        None => &[],
    }
}

/// The number of diagnostics in this error: usually one, but a parse error, for example, has
/// one per problem found. Zero if it holds no error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_diagnostic_count(err: *const nickel_error) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { diagnostics(err) }.len()
}

/// The diagnostic at `idx`, or NULL if `idx` is out of range. It is valid until `err` is
/// overwritten or freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_diagnostic(
    err: *const nickel_error,
    idx: usize,
) -> *const nickel_diagnostic {
    // Safety: pre-conditions of this function.
    match unsafe { diagnostics(err) }.get(idx) {
        Some(d) => d as *const Diagnostic as *const nickel_diagnostic,
        None => std::ptr::null(),
    }
}

/// How serious this diagnostic is.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_severity(
    d: *const nickel_diagnostic,
) -> nickel_severity {
    // Safety: pre-conditions of this function.
    match unsafe { diagnostic(d) }.severity {
        Severity::Help => nickel_severity::NICKEL_SEVERITY_HELP,
        Severity::Note => nickel_severity::NICKEL_SEVERITY_NOTE,
        Severity::Warning => nickel_severity::NICKEL_SEVERITY_WARNING,
        Severity::Error => nickel_severity::NICKEL_SEVERITY_ERROR,
        Severity::Bug => nickel_severity::NICKEL_SEVERITY_BUG,
        _ => nickel_severity::NICKEL_SEVERITY_ERROR,
    }
}

/// The diagnostic's message. A pointer to UTF-8 (not null-terminated) is written to `out_str`
/// and its length returned; it is valid as long as the diagnostic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_message(
    d: *const nickel_diagnostic,
    out_str: *mut *const c_char,
) -> usize {
    // Safety: pre-conditions of this function.
    unsafe {
        let message = &diagnostic(d).message;
        write_str(message, out_str, std::ptr::null_mut());
        message.len()
    }
}

/// The diagnostic's code, a short identifier for the problem, written as in
/// [`nickel_field_metadata_doc`]. Returns 1, or 0 if Nickel gave none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_code(
    d: *const nickel_diagnostic,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match &diagnostic(d).code {
            Some(code) => {
                write_str(code, out_str, out_len);
                1
            }
            None => 0,
        }
    }
}

/// The number of notes: extra explanation not tied to a place in the source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_note_count(d: *const nickel_diagnostic) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { diagnostic(d) }.notes.len()
}

/// The note at `idx`, written as in [`nickel_field_metadata_doc`]. Returns 1, or 0 if
/// `idx` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_note(
    d: *const nickel_diagnostic,
    idx: usize,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match diagnostic(d).notes.get(idx) {
            Some(note) => {
                write_str(note, out_str, out_len);
                1
            }
            None => 0,
        }
    }
}

/// The number of places in the source this diagnostic points at.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_label_count(d: *const nickel_diagnostic) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { diagnostic(d) }.labels.len()
}

/// The label at `idx`, or NULL if `idx` is out of range. It is valid as long as the diagnostic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_diagnostic_label(
    d: *const nickel_diagnostic,
    idx: usize,
) -> *const nickel_label {
    // Safety: pre-conditions of this function.
    match unsafe { diagnostic(d) }.labels.get(idx) {
        Some(l) => l as *const Label as *const nickel_label,
        None => std::ptr::null(),
    }
}

/// Is this where the problem is (1), or context for it (0)?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_label_is_primary(l: *const nickel_label) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe { label(l) }.primary as c_int
}

/// The label's message, which may be empty, returned like [`nickel_diagnostic_message`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_label_message(
    l: *const nickel_label,
    out_str: *mut *const c_char,
) -> usize {
    // Safety: pre-conditions of this function.
    unsafe {
        let message = &label(l).message;
        write_str(message, out_str, std::ptr::null_mut());
        message.len()
    }
}

/// The name of the source the label points into (a file path, or the name given to in-memory
/// code), returned like [`nickel_diagnostic_message`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_label_source(
    l: *const nickel_label,
    out_str: *mut *const c_char,
) -> usize {
    // Safety: pre-conditions of this function.
    unsafe {
        let source = &label(l).source;
        write_str(source, out_str, std::ptr::null_mut());
        source.len()
    }
}

/// The label's position: the byte range `[start, end)` in the source, and the 1-based line and
/// column where it starts and ends. Each out-pointer may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_label_position(
    l: *const nickel_label,
    out_start_byte: *mut usize,
    out_end_byte: *mut usize,
    out_start_line: *mut usize,
    out_start_column: *mut usize,
    out_end_line: *mut usize,
    out_end_column: *mut usize,
) {
    // Safety: pre-conditions of this function.
    unsafe {
        let l = label(l);
        for (out, value) in [
            (out_start_byte, l.range.start),
            (out_end_byte, l.range.end),
            (out_start_line, l.start.line),
            (out_start_column, l.start.column),
            (out_end_line, l.end.line),
            (out_end_column, l.end.column),
        ] {
            if !out.is_null() {
                *out = value;
            }
        }
    }
}
