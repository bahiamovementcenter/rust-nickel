//! C API for `nickel-embed`.
//!
//! The first part of this API is Nickel's own C API (`nickel_lang.h` from `nickel-lang` 2.2,
//! which go-nickel vendors): every function, type and constant in that header is declared here
//! identically and behaves the same, so programs built against it (go-nickel included) work
//! unchanged when linked against this library instead.
//!
//! The rest is new, and exposes what `nickel-embed` adds:
//!
//! - programs built from several sources (files, in-memory code, JSON, YAML, TOML, text) with
//!   import paths, overrides, applied contracts and a focused field, which can be evaluated,
//!   exported, queried and typechecked like the `nickel` command line tool
//!   (`nickel_program_builder_*`, `nickel_program_*`);
//! - field metadata on records and from queries (`nickel_field_metadata_*`);
//! - structured errors: an error kind, and diagnostics with severities, notes and labelled
//!   source locations (`nickel_error_get_kind`, `nickel_diagnostic_*`, `nickel_label_*`);
//! - package imports (`import foo`) through a package map, filled by hand or from a
//!   `Nickel-pkg.ncl` manifest (`nickel_package_map_*`);
//! - warnings through a callback, import paths on a context, the formatter, documentation
//!   extraction and the library version.
//!
//! Unlike the original functions, which panic (and so abort) when given the wrong kind of value,
//! the new functions report misuse through their return value.
//!
//! # Ownership and lifetimes
//!
//! All the owning types (`nickel_context`, `nickel_expr`, `nickel_error`, `nickel_string`, and
//! the new `nickel_program_builder`, `nickel_program`, `nickel_package_map`, `nickel_field_info`
//! and `nickel_field_metadata`) are only made available to C via opaque pointers, created by a
//! `nickel_*_alloc` function and destroyed by the matching `nickel_*_free`. Functions that
//! produce one of these write into an object the caller allocated.
//!
//! `nickel_record`, `nickel_array`, `nickel_number`, `nickel_diagnostic` and `nickel_label`
//! pointers borrow from the object they came from, and are invalidated when it is overwritten
//! or freed.
//!
//! # Threads
//!
//! None of these objects may be used from two threads at once, and objects from one context or
//! program must stay on the thread that created them.

// Parts of this file are derived from nickel-lang 2.2.0's C API (src/capi.rs,
// https://github.com/tweag/nickel, MIT License, Copyright The Nickel Team).

// Note to future maintainers: pointers to the owning types should only come from
// `nickel_something_alloc` functions, never be temporary pointers to things owned on the
// Rust side.

#![allow(clippy::missing_safety_doc)]
#![allow(non_camel_case_types)]
#![deny(missing_docs)]

mod diagnostics;
mod packages;
mod program;
mod tooling;

pub use diagnostics::*;
pub use packages::*;
pub use program::*;
pub use tooling::*;

use std::{
    cell::OnceCell,
    ffi::{CStr, c_char, c_int, c_void},
    io::Write,
};

use nickel_embed::{Array, Context, Diagnostic, Error, ErrorFormat, Expr, Number, Record};

/// The main entry point.
pub struct nickel_context {
    inner: Context,
}

/// A Nickel error.
///
/// If you want to collect an error message from a fallible function
/// (like `nickel_context_eval_deep`), first allocate an error using
/// `nickel_error_alloc`, and then pass the resulting pointer to your fallible
/// function. If that function fails, it will save the error data in your
/// `nickel_error`.
pub struct nickel_error {
    inner: Option<Error>,
    // Computed on first use by the diagnostics functions, and reset whenever `inner` changes.
    diagnostics: OnceCell<Vec<Diagnostic>>,
}

impl nickel_error {
    fn set(&mut self, error: Error) {
        self.inner = Some(error);
        self.diagnostics = OnceCell::new();
    }
}

/// A Nickel expression.
///
/// This might be fully evaluated (for example, if you got it from [`nickel_context_eval_deep`])
/// or might have unevaluated sub-expressions (if you got it from [`nickel_context_eval_shallow`]).
pub struct nickel_expr {
    // `None` is the `null` a fresh expression holds: `nickel-embed` has no public way to make a
    // null `Expr` without evaluating one.
    inner: Option<Expr>,
}

/// A Nickel array.
///
/// See [`nickel_expr_is_array`] and [`nickel_expr_as_array`].
// An opaque type: a `*const nickel_array` is secretly the `*const nickel_expr` it came from,
// since nickel-embed's `Array` can't be reached through a stable pointer. A null pointer is
// an empty array, as in Nickel's own C API.
pub struct nickel_array {}

/// A Nickel record.
///
/// See [`nickel_expr_is_record`] and [`nickel_expr_as_record`].
// Secretly a `*const nickel_expr`, like `nickel_array`. A null pointer is an empty record.
pub struct nickel_record {}

/// A Nickel string.
pub struct nickel_string {
    inner: String,
}

/// A Nickel number.
///
/// See [`nickel_expr_is_number`] and [`nickel_expr_as_number`].
// Secretly a `*const nickel_expr`, like `nickel_array`.
pub struct nickel_number {}

// Pointer helpers. Each assumes its pointer came from the matching `nickel_*_alloc` (or, for
// the borrowed types, from the accessor that returns them) and is still valid.

unsafe fn context<'a>(ctx: *mut nickel_context) -> &'a mut Context {
    // Safety: pre-condition of this function.
    unsafe { &mut ctx.as_mut().expect("null nickel_context").inner }
}

unsafe fn expr<'a>(expr: *const nickel_expr) -> Option<&'a Expr> {
    // Safety: pre-condition of this function.
    unsafe { expr.as_ref().expect("null nickel_expr").inner.as_ref() }
}

unsafe fn set_expr(out: *mut nickel_expr, value: Expr) {
    // Safety: `out` is null or valid, a pre-condition of this function.
    if let Some(out) = unsafe { out.as_mut() } {
        out.inner = Some(value);
    }
}

unsafe fn set_error(out: *mut nickel_error, error: Error) {
    // Safety: `out` is null or valid, a pre-condition of this function.
    if let Some(out) = unsafe { out.as_mut() } {
        out.set(error);
    }
}

unsafe fn set_string(out: *mut nickel_string, value: String) {
    // Safety: `out` is null or valid, a pre-condition of this function.
    if let Some(out) = unsafe { out.as_mut() } {
        out.inner = value;
    }
}

/// Writes a successful value with `ok`, or an error to `out_error`, and returns the result code.
unsafe fn finish<T>(
    result: Result<T, Error>,
    out_error: *mut nickel_error,
    ok: impl FnOnce(T),
) -> nickel_result {
    match result {
        Ok(value) => {
            ok(value);
            nickel_result::NICKEL_RESULT_OK
        }
        Err(e) => {
            // Safety: pre-condition of this function.
            unsafe { set_error(out_error, e) };
            nickel_result::NICKEL_RESULT_ERR
        }
    }
}

/// Reads a NUL-terminated string that must be UTF-8.
unsafe fn utf8<'a>(s: *const c_char, what: &str) -> Result<&'a str, Error> {
    if s.is_null() {
        return Err(usage_error(format!("{what} is NULL")));
    }
    // Safety: `s` is a valid C string, a pre-condition of this function.
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|e| usage_error(format!("{what} is not valid UTF-8: {e}")))
}

/// An error for a bad argument from C. `nickel-embed` errors can only be made from Nickel
/// failures or I/O errors, so these have the kind `NICKEL_ERROR_KIND_IO`.
fn usage_error(message: String) -> Error {
    Error::from(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    ))
}

/// Writes a borrowed string out as a pointer and length.
unsafe fn write_str(s: &str, out_str: *mut *const c_char, out_len: *mut usize) {
    // Safety: the out-pointers are null or valid, a pre-condition of this function.
    unsafe {
        if !out_str.is_null() {
            *out_str = s.as_ptr() as *const c_char;
        }
        if !out_len.is_null() {
            *out_len = s.len();
        }
    }
}

thread_local! {
    // A `null` expression, for passing a fresh `nickel_expr` to functions that need an `Expr`.
    static NULL: Expr = Context::new().eval_deep("null").expect("`null` evaluates");
}

fn null_expr() -> Expr {
    NULL.with(Clone::clone)
}

unsafe fn expr_or_null(e: *const nickel_expr) -> Expr {
    // Safety: pre-condition of this function.
    unsafe { expr(e) }.cloned().unwrap_or_else(null_expr)
}

unsafe fn record<'a>(rec: *const nickel_record) -> Option<Record<'a>> {
    // Safety: a non-null `rec` is a `nickel_expr` pointer, a pre-condition of this function.
    unsafe { expr(rec as *const nickel_expr) }.map(|e| {
        e.as_record()
            .expect("nickel_record no longer points to a record")
    })
}

unsafe fn array<'a>(arr: *const nickel_array) -> Option<Array<'a>> {
    if arr.is_null() {
        return None;
    }
    // Safety: a non-null `arr` is a `nickel_expr` pointer, a pre-condition of this function.
    unsafe { expr(arr as *const nickel_expr) }.map(|e| {
        e.as_array()
            .expect("nickel_array no longer points to an array")
    })
}

unsafe fn number<'a>(num: *const nickel_number) -> Number<'a> {
    // Safety: `num` is a `nickel_expr` pointer, a pre-condition of this function.
    unsafe { expr(num as *const nickel_expr) }
        .and_then(Expr::as_number)
        .expect("nickel_number no longer points to a number")
}

/// Allocate a new [`nickel_context`], which can be used to evaluate Nickel expressions.
///
/// Returns a newly-allocated [`nickel_context`] that can be freed with [`nickel_context_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_alloc() -> *mut nickel_context {
    Box::into_raw(Box::new(nickel_context {
        inner: Context::default(),
    }))
}

/// Free a [`nickel_context`] that was created with [`nickel_context_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_free(ctx: *mut nickel_context) {
    if !ctx.is_null() {
        // Safety: `ctx` was returned by `nickel_context_alloc`.
        drop(unsafe { Box::from_raw(ctx) });
    }
}

/// A callback function for writing data.
///
/// This function will be called with a buffer (`buf`) of data, having length
/// `len`. It need not consume the entire buffer, and should return the number
/// of bytes consumed.
// This Option<fn> pattern seems to be cbindgen's preferred way of encoding
// a nullable function pointer (since rust fns are never null).
// https://github.com/mozilla/cbindgen/issues/326#issuecomment-584288686
pub type nickel_write_callback =
    Option<extern "C" fn(context: *mut c_void, buf: *const u8, len: usize) -> usize>;

/// A callback function for flushing data that was written by a write callback.
pub type nickel_flush_callback = Option<extern "C" fn(context: *const c_void)>;

/// For functions that can fail, these are the interpretations of the return value.
#[repr(C)]
pub enum nickel_error_format {
    /// Format an error as human-readable text.
    NICKEL_ERROR_FORMAT_TEXT = 0,
    /// Format an error as human-readable text, with ANSI color codes.
    NICKEL_ERROR_FORMAT_ANSI_TEXT = 1,
    /// Format an error as JSON.
    NICKEL_ERROR_FORMAT_JSON = 2,
    /// Format an error as YAML.
    NICKEL_ERROR_FORMAT_YAML = 3,
    /// Format an error as TOML.
    NICKEL_ERROR_FORMAT_TOML = 4,
}

/// For functions that can fail, these are the interpretations of the return value.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum nickel_result {
    /// A successful result.
    NICKEL_RESULT_OK = 0,
    /// A bad result.
    NICKEL_RESULT_ERR = 1,
}

impl From<nickel_error_format> for ErrorFormat {
    fn from(e: nickel_error_format) -> Self {
        match e {
            nickel_error_format::NICKEL_ERROR_FORMAT_TEXT => ErrorFormat::Text,
            nickel_error_format::NICKEL_ERROR_FORMAT_ANSI_TEXT => ErrorFormat::AnsiText,
            nickel_error_format::NICKEL_ERROR_FORMAT_JSON => ErrorFormat::Json,
            nickel_error_format::NICKEL_ERROR_FORMAT_YAML => ErrorFormat::Yaml,
            nickel_error_format::NICKEL_ERROR_FORMAT_TOML => ErrorFormat::Toml,
        }
    }
}

/// A `Write` that forwards to C callbacks.
struct CWriter {
    write: nickel_write_callback,
    flush: nickel_flush_callback,
    context: *mut c_void,
}

impl Write for CWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let count = if let Some(w) = self.write {
            w(self.context, buf.as_ptr(), buf.len())
        } else {
            buf.len()
        };
        if count == usize::MAX {
            Err(std::io::Error::other("trace failed to write"))
        } else {
            Ok(count)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(flush) = self.flush {
            flush(self.context);
        }
        Ok(())
    }
}

/// Provide a callback that will be called when evaluating Nickel
/// code that uses `std.trace`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_set_trace_callback(
    ctx: *mut nickel_context,
    write: nickel_write_callback,
    flush: nickel_flush_callback,
    user_data: *mut c_void,
) {
    // Safety: `ctx` was returned by `nickel_context_alloc`.
    let ctx = unsafe { context(ctx) };
    *ctx = std::mem::take(ctx).with_trace(CWriter {
        write,
        flush,
        context: user_data,
    });
}

/// Provide a name for the main input program.
///
/// This is used to format error messages. If you read the main input
/// program from a file, its path is a good choice.
///
/// `name` should be a UTF-8-encoded, null-terminated string. It is only
/// borrowed temporarily; the pointer need not remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_set_source_name(
    ctx: *mut nickel_context,
    name: *const c_char,
) {
    // Safety: `name` is a valid C string.
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    // Safety: `ctx` was returned by `nickel_context_alloc`.
    let ctx = unsafe { context(ctx) };
    *ctx = std::mem::take(ctx).with_source_name(name);
}

/// Runs an evaluation of source code and stores its result.
unsafe fn do_eval(
    f: impl FnOnce(&mut Context, &str) -> Result<Expr, Error>,
    ctx: *mut nickel_context,
    src: *const c_char,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of the public functions that call this one.
    unsafe {
        let ctx = context(ctx);
        let result = utf8(src, "the source").and_then(|src| f(ctx, src));
        finish(result, out_error, |e| set_expr(out_expr, e))
    }
}

/// Evaluate a Nickel program deeply.
///
/// "Deeply" means that we recursively evaluate records and arrays. For
/// an alternative, see [`nickel_context_eval_shallow`].
///
/// - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
/// - `out_expr` either NULL or something that was created with [`nickel_expr_alloc`]
/// - `out_error` can be NULL if you aren't interested in getting detailed
///   error messages
///
/// If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces
/// the value at `out_expr` (if non-NULL) with the newly-evaluated Nickel expression.
///
/// If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the
/// value at `out_error` (if non-NULL) by a pointer to a newly-allocated Nickel error.
/// That error should be freed with `nickel_error_free` when you are
/// done with it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_eval_deep(
    ctx: *mut nickel_context,
    src: *const c_char,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: the safety requirements of `do_eval` are expected from a C call.
    unsafe { do_eval(|ctx, src| ctx.eval_deep(src), ctx, src, out_expr, out_error) }
}

/// Evaluate a Nickel program deeply.
///
/// This differs from [`nickel_context_eval_deep`] in that it ignores
/// fields marked as `not_exported`.
///
/// - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
/// - `out_expr` either NULL or something that was created with [`nickel_expr_alloc`]
/// - `out_error` can be NULL if you aren't interested in getting detailed
///   error messages
///
/// If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces
/// the value at `out_expr` (if non-NULL) with the newly-evaluated Nickel expression.
///
/// If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the
/// value at `out_error` (if non-NULL) by a pointer to a newly-allocated Nickel error.
/// That error should be freed with `nickel_error_free` when you are
/// done with it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_eval_deep_for_export(
    ctx: *mut nickel_context,
    src: *const c_char,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: the safety requirements of `do_eval` are expected from a C call.
    unsafe {
        do_eval(
            |ctx, src| ctx.eval_deep_for_export(src),
            ctx,
            src,
            out_expr,
            out_error,
        )
    }
}

/// Evaluate a Nickel program to weak head normal form (WHNF).
///
/// The result of this evaluation is a null, bool, number, string,
/// enum, record, or array. In case it's a record, array, or enum
/// variant, the payload (record values, array elements, or enum
/// payloads) will be left unevaluated.
///
/// Sub-expressions of the result can be evaluated further by [nickel_context_eval_expr_shallow].
///
/// - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
/// - `out_expr` is either NULL or something that was created with [`nickel_expr_alloc`]
/// - `out_error` can be NULL if you aren't interested in getting detailed
///   error messages
///
/// If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces the value at `out_expr`
/// (if non-NULL) with the newly-evaluated Nickel expression.
///
/// If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the value at `out_error` (if
/// non-NULL) by a pointer to a newly-allocated Nickel error. That error should be freed with
/// `nickel_error_free` when you are done with it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_eval_shallow(
    ctx: *mut nickel_context,
    src: *const c_char,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: the safety requirements of `do_eval` are expected from a C call.
    unsafe {
        do_eval(
            |ctx, src| ctx.eval_shallow(src),
            ctx,
            src,
            out_expr,
            out_error,
        )
    }
}

/// Allocate a new Nickel expression.
///
/// The returned expression pointer can be used to store the results of
/// evaluation, for example by passing it as the `out_expr` location of
/// `nickel_context_eval_deep`.
///
/// Each call to `nickel_expr_alloc` should be paired with a call to
/// `nickel_expr_free`. The various functions (like `nickel_context_eval_deep`)
/// that take an `out_expr` parameter overwrite the existing expression
/// contents, and do not affect the pairing of `nickel_expr_alloc` and
/// `nickel_expr_free`.
///
/// For example:
///
/// ```c
/// nickel_context *ctx = nickel_context_alloc();
/// nickel_context *expr = nickel_expr_alloc();
///
/// nickel_context_eval_deep(ctx, "{ foo = 1 }", expr, NULL);
///
/// // now expr is a record
/// printf("record: %d\n", nickel_expr_is_record(expr));
///
/// nickel_context_eval_deep(ctx, "[1, 2, 3]", expr, NULL);
///
/// // now expr is an array
/// printf("array: %d\n", nickel_expr_is_array(expr));
///
/// // the calls to nickel_context_eval_deep haven't created any new exprs:
/// // we only need to free it once
/// nickel_expr_free(expr);
/// nickel_context_free(ctx);
/// ```
///
/// An `Expr` owns its data. There are various ways to get a reference to
/// data owned by an expression, which are then invalidated when the expression
/// is freed (by `nickel_expr_free`) or overwritten (for example, by
/// `nickel_context_deep_eval`).
///
/// ```c
/// nickel_context *ctx = nickel_context_alloc();
/// nickel_expr *expr = nickel_expr_alloc();
///
/// nickel_context_eval_deep(ctx, "{ foo = 1 }", expr, NULL);
///
/// nickel_record *rec = nickel_expr_as_record(expr);
/// nickel_expr *field = nickel_expr_alloc();
/// nickel_record_value_by_name(rec, "foo", field);
///
/// // Now `rec` points to data owned by `expr`, but `field`
/// // owns its own data. The following deallocation invalidates
/// // `rec`, but not `field`.
/// nickel_expr_free(expr);
/// printf("number: %d\n", nickel_expr_is_number(field));
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_alloc() -> *mut nickel_expr {
    Box::into_raw(Box::new(nickel_expr { inner: None }))
}

/// Free a Nickel expression.
///
/// See [`nickel_expr_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_free(expr: *mut nickel_expr) {
    if !expr.is_null() {
        // Safety: `expr` was returned by `nickel_expr_alloc`.
        drop(unsafe { Box::from_raw(expr) });
    }
}

/// Is this expression a boolean?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_bool(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_bool) as c_int
}

/// Is this expression a number?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_number(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_number) as c_int
}

/// Is this expression a string?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_str(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_string) as c_int
}

/// Is this expression an enum tag?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_enum_tag(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_enum_tag) as c_int
}

/// Is this expression an enum variant?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_enum_variant(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_enum_variant) as c_int
}

/// Is this expression a record?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_record(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_record) as c_int
}

/// Is this expression an array?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_array(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_some_and(Expr::is_array) as c_int
}

/// Has this expression been evaluated?
///
/// An evaluated expression is either null, or it's a number, bool, string, record, array, or enum.
/// If this expression is not a value, you probably got it from looking inside the result of
/// [`nickel_context_eval_shallow`], and you can use the [`nickel_context_eval_expr_shallow`] to
/// evaluate this expression further.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_value(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_none_or(Expr::is_value) as c_int
}

/// Is this expression null?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_is_null(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }.is_none_or(Expr::is_null) as c_int
}

/// If this expression is a boolean, returns that boolean.
///
/// # Panics
///
/// Panics if `expr` is not a boolean.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_bool(expr: *const nickel_expr) -> c_int {
    // Safety: `expr` is expected to be a valid pointer to an expression
    unsafe { self::expr(expr) }
        .and_then(Expr::as_bool)
        .expect("not a boolean") as c_int
}

/// If this expression is a string, returns that string.
///
/// A pointer to the string contents, which are UTF-8 encoded, is returned in
/// `out_str`. These contents are *not* null-terminated. The return value of this
/// function is the length of these contents.
///
/// The returned string contents are owned by this `Expr`, and will be invalidated
/// when the `Expr` is freed with [`nickel_expr_free`].
///
/// # Panics
///
/// Panics if `expr` is not a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_str(
    expr: *const nickel_expr,
    out_str: *mut *const c_char,
) -> usize {
    // Safety: `expr` is expected to be a valid pointer to an expression, `out_str` to be valid.
    unsafe {
        let s = self::expr(expr)
            .and_then(Expr::as_str)
            .expect("not a string");
        write_str(s, out_str, std::ptr::null_mut());
        s.len()
    }
}

/// If this expression is a number, returns the number.
///
/// The returned number pointer borrows from `expr`, and will be invalidated
/// when `expr` is overwritten or freed.
///
/// # Panics
///
/// Panics if `expr` is not an number.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_number(expr: *const nickel_expr) -> *const nickel_number {
    // Safety: `expr` is expected to be a valid pointer to an expression
    assert!(
        unsafe { self::expr(expr) }.is_some_and(Expr::is_number),
        "not a number"
    );
    expr as *const nickel_number
}

/// If this expression is an enum tag, returns its string value.
///
/// A pointer to the string contents, which are UTF-8 encoded, is returned in
/// `out_str`. These contents are *not* null-terminated. The return value of this
/// function is the length of these contents.
///
/// The returned string contents point to an interned string and will never be
/// invalidated.
///
/// # Panics
///
/// Panics if `expr` is null or is not an enum tag.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_enum_tag(
    expr: *const nickel_expr,
    out_str: *mut *const c_char,
) -> usize {
    // Safety: `expr` is expected to be a valid pointer to an expression, `out_str` to be valid.
    unsafe {
        let s = self::expr(expr)
            .and_then(Expr::as_enum_tag)
            .expect("not an enum tag");
        write_str(s, out_str, std::ptr::null_mut());
        s.len()
    }
}

/// If this expression is an enum variant, returns its string value and its payload.
///
/// A pointer to the string contents, which are UTF-8 encoded, is returned in
/// `out_str`. These contents are *not* null-terminated. The return value of this
/// function is the length of these contents.
///
/// The returned string contents point to an interned string and will never be
/// invalidated.
///
/// # Panics
///
/// Panics if `expr` is not an enum tag.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_enum_variant(
    expr: *const nickel_expr,
    out_str: *mut *const c_char,
    out_expr: *mut nickel_expr,
) -> usize {
    // Safety: pre-conditions of this function. The tag is interned, so it stays valid when
    // `out_expr` is `expr`.
    unsafe {
        let (tag, payload) = self::expr(expr)
            .and_then(Expr::as_enum_variant)
            .expect("not an enum variant");
        let (ptr, len) = (tag.as_ptr(), tag.len());
        *out_str = ptr as *const c_char;
        set_expr(out_expr, payload);
        len
    }
}

/// If this expression is a record, returns the record.
///
/// The returned record pointer borrows from `expr`, and will be invalidated
/// when `expr` is overwritten or freed.
///
/// # Panics
///
/// Panics if `expr` is not an record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_record(expr: *const nickel_expr) -> *const nickel_record {
    // Safety: `expr` is expected to be a valid pointer to an expression
    assert!(
        unsafe { self::expr(expr) }.is_some_and(Expr::is_record),
        "not a record"
    );
    expr as *const nickel_record
}

/// If this expression is an array, returns the array.
///
/// The returned array pointer borrows from `expr`, and will be invalidated
/// when `expr` is overwritten or freed.
///
/// # Panics
///
/// Panics if `expr` is not an array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_expr_as_array(expr: *const nickel_expr) -> *const nickel_array {
    // Safety: `expr` is expected to be a valid pointer to an expression
    assert!(
        unsafe { self::expr(expr) }.is_some_and(Expr::is_array),
        "not an array"
    );
    expr as *const nickel_array
}

/// Converts an expression with `f` and stores the string or the error.
unsafe fn export_expr(
    ctx: *mut nickel_context,
    expr: *const nickel_expr,
    out_string: *mut nickel_string,
    out_err: *mut nickel_error,
    f: impl FnOnce(&Context, &Expr) -> Result<String, Error>,
) -> nickel_result {
    // Safety: pre-conditions of the public functions that call this one.
    unsafe {
        let result = f(context(ctx), &expr_or_null(expr));
        finish(result, out_err, |s| set_string(out_string, s))
    }
}

/// Converts an expression to JSON.
///
/// This is fallible because enum variants have no canonical conversion to
/// JSON: if the expression contains any enum variants, this will fail.
/// This also fails if the expression contains any unevaluated sub-expressions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_expr_to_json(
    ctx: *mut nickel_context,
    expr: *const nickel_expr,
    out_string: *mut nickel_string,
    out_err: *mut nickel_error,
) -> nickel_result {
    // Safety: function pre-conditions
    unsafe { export_expr(ctx, expr, out_string, out_err, Context::expr_to_json) }
}

/// Converts an expression to YAML.
///
/// This is fallible because enum variants have no canonical conversion to
/// YAML: if the expression contains any enum variants, this will fail.
/// This also fails if the expression contains any unevaluated sub-expressions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_expr_to_yaml(
    ctx: *mut nickel_context,
    expr: *const nickel_expr,
    out_string: *mut nickel_string,
    out_err: *mut nickel_error,
) -> nickel_result {
    // Safety: function pre-conditions
    unsafe { export_expr(ctx, expr, out_string, out_err, Context::expr_to_yaml) }
}

/// Converts an expression to TOML.
///
/// This is fallible because enum variants have no canonical conversion to
/// TOML: if the expression contains any enum variants, this will fail.
/// This also fails if the expression contains any unevaluated sub-expressions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_expr_to_toml(
    ctx: *mut nickel_context,
    expr: *const nickel_expr,
    out_string: *mut nickel_string,
    out_err: *mut nickel_error,
) -> nickel_result {
    // Safety: function pre-conditions
    unsafe { export_expr(ctx, expr, out_string, out_err, Context::expr_to_toml) }
}

/// Is this number an integer within the range of an `int64_t`?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_number_is_i64(num: *const nickel_number) -> c_int {
    // Safety: `num` is expected to be a valid pointer to `nickel_number`
    unsafe { number(num) }.as_i64().is_some() as c_int
}

/// If this number is an integer within the range of an `int64_t`, returns it.
///
/// # Panics
///
/// Panics if this number is not an integer in the appropriate range (you should
/// check with [`nickel_number_is_i64`] first).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_number_as_i64(num: *const nickel_number) -> i64 {
    // Safety: `num` is expected to be a valid pointer to `nickel_number`
    unsafe { number(num) }
        .as_i64()
        .expect("not an integer in range")
}

/// The value of this number, rounded to the nearest `double`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_number_as_f64(num: *const nickel_number) -> f64 {
    // Safety: `num` is expected to be a valid pointer to `nickel_number`
    unsafe { number(num) }.as_f64()
}

/// The value of this number, as an exact rational number.
///
/// - `out_numerator` must have been allocated with [`nickel_string_alloc`]. It
///   will be overwritten with the numerator, as a decimal string.
/// - `out_denominator` must have been allocated with [`nickel_string_alloc`].
///   It will be overwritten with the denominator, as a decimal string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_number_as_rational(
    num: *const nickel_number,
    out_numerator: *mut nickel_string,
    out_denominator: *mut nickel_string,
) {
    // Safety: pre-conditions of this function
    unsafe {
        let (numerator, denominator) = number(num).as_rational();
        set_string(out_numerator, numerator);
        set_string(out_denominator, denominator);
    }
}

/// The number of elements of this Nickel array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_array_len(arr: *const nickel_array) -> usize {
    // Safety: `arr` is expected to be a valid pointer to a nickel array, or null.
    unsafe { array(arr) }.map_or(0, |a| a.len())
}

/// Retrieve the element at the given array index.
///
/// The retrieved element will be written to `out_expr`, which must have been allocated with
/// [`nickel_expr_alloc`].
///
/// # Panics
///
/// Panics if the given index is out of bounds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_array_get(
    arr: *const nickel_array,
    idx: usize,
    out_expr: *mut nickel_expr,
) {
    // Safety: `arr` is expected to be a valid pointer to a nickel array, `out_expr` to an
    // allocated `nickel_expr`.
    unsafe {
        let elem = array(arr)
            .and_then(|a| a.get(idx))
            .expect("array index out of bounds");
        set_expr(out_expr, elem);
    }
}

/// The number of keys in this Nickel record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_record_len(rec: *const nickel_record) -> usize {
    if rec.is_null() {
        return 0;
    }
    // Safety: `rec` is expected to be a valid pointer to a `nickel_record`.
    unsafe { record(rec) }.map_or(0, |r| r.len())
}

/// Retrieve the key and value at the given index.
///
/// If this record was deeply evaluated, every key will come with a value.
/// However, shallowly evaluated records may have fields with no value.
///
/// Returns 1 if the key came with a value, and 0 if it didn't. The value
/// will be written to `out_expr` if it is non-NULL.
///
/// # Panics
///
/// Panics if `idx` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_record_key_value_by_index(
    rec: *const nickel_record,
    idx: usize,
    out_key: *mut *const c_char,
    out_key_len: *mut usize,
    out_expr: *mut nickel_expr,
) -> c_int {
    // Safety: pre-conditions of this function
    unsafe {
        assert!(!rec.is_null(), "record index out of range");
        let rec = record(rec).expect("not a record");
        let (key, val) = rec
            .key_value_by_index(idx)
            .expect("record index out of range");
        write_str(key, out_key, out_key_len);
        match val {
            Some(val) => {
                set_expr(out_expr, val);
                1
            }
            None => 0,
        }
    }
}

/// Look up a key in this record and return its value, if there is one.
///
/// Returns 1 if the key has a value, and 0 if it didn't. The value is
/// written to `out_expr` if it is non-NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_record_value_by_name(
    rec: *const nickel_record,
    key: *const c_char,
    out_expr: *mut nickel_expr,
) -> c_int {
    if rec.is_null() {
        return 0;
    }
    // Safety: pre-conditions of this function
    unsafe {
        let Ok(key) = CStr::from_ptr(key).to_str() else {
            return 0;
        };
        match record(rec).and_then(|r| r.value_by_name(key)) {
            Some(val) => {
                set_expr(out_expr, val);
                1
            }
            None => 0,
        }
    }
}

/// Allocates a new string.
///
/// The lifecycle management of a string is much like that of an expression
/// (see `nickel_expr_alloc`). It gets allocated here, modified by various other
/// functions, and finally is freed by a call to `nickel_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_string_alloc() -> *mut nickel_string {
    Box::into_raw(Box::new(nickel_string {
        inner: String::new(),
    }))
}

/// Frees a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_string_free(s: *mut nickel_string) {
    if !s.is_null() {
        // Safety: `s` was returned by `nickel_string_alloc`.
        drop(unsafe { Box::from_raw(s) });
    }
}

/// Retrieve the data inside a string.
///
/// A pointer to the string contents, which are UTF-8 encoded, is written to
/// `data`. These contents are *not* null-terminated, but their length (in bytes)
/// is written to `len`. The string contents will be invalidated when `s` is
/// freed or overwritten.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_string_data(
    s: *const nickel_string,
    data: *mut *const c_char,
    len: *mut usize,
) {
    // Safety: pre-conditions of this function
    unsafe {
        let s = s.as_ref().expect("null nickel_string");
        write_str(&s.inner, data, len);
    }
}

/// Evaluate an expression to weak head normal form (WHNF).
///
/// This has no effect if the expression is already evaluated (see
/// [`nickel_expr_is_value`]).
///
/// The result of this evaluation is a null, bool, number, string,
/// enum, record, or array. In case it's a record, array, or enum
/// variant, the payload (record values, array elements, or enum
/// payloads) will be left unevaluated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_eval_expr_shallow(
    ctx: *mut nickel_context,
    expr: *const nickel_expr,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // We clone `expr` instead of consuming it (as the Rust API does). The clone is
    // cheap (it's only a refcount bump) and this makes the allocation/free pairing
    // easier to get right: only `nickel_expr_alloc` creates an expr and only `nickel_expr_free`
    // frees one.
    // Safety: pre-conditions of this function
    unsafe {
        let result = context(ctx).eval_expr_shallow(expr_or_null(expr));
        finish(result, out_error, |e| set_expr(out_expr, e))
    }
}

/// Allocate a new `nickel_error`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_alloc() -> *mut nickel_error {
    Box::into_raw(Box::new(nickel_error {
        inner: None,
        diagnostics: OnceCell::new(),
    }))
}

/// Frees a `nickel_error`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_free(err: *mut nickel_error) {
    if !err.is_null() {
        // Safety: `err` was returned by `nickel_error_alloc`.
        drop(unsafe { Box::from_raw(err) });
    }
}

unsafe fn stored_error<'a>(err: *const nickel_error) -> &'a Error {
    // Safety: `err` is a valid pointer to a `nickel_error`.
    unsafe { err.as_ref() }
        .expect("null nickel_error")
        .inner
        .as_ref()
        .expect("uninitialized error")
}

/// Write out an error as a user- or machine-readable diagnostic.
///
/// - `err` must have been allocated by `nickel_error_alloc` and initialized by some failing
///   function (like `nickel_context_eval_deep`).
/// - `write` is a callback function that will be invoked with UTF-8 encoded data.
/// - `write_payload` is optional extra data to pass to `write`
/// - `format` selects the error-rendering format.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_display(
    err: *const nickel_error,
    write: nickel_write_callback,
    write_payload: *mut c_void,
    format: nickel_error_format,
) -> nickel_result {
    // Safety: `err` is required to be valid pointer to `nickel_error`.
    let err = unsafe { stored_error(err) };
    let mut write = CWriter {
        write,
        flush: None,
        context: write_payload,
    };
    if err.format(&mut write, format.into()).is_err() {
        nickel_result::NICKEL_RESULT_ERR
    } else {
        nickel_result::NICKEL_RESULT_OK
    }
}

/// Write out an error as a user- or machine-readable diagnostic.
///
/// This is like `nickel_error_format`, but writes the error to a string instead
/// of via a callback function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_error_format_as_string(
    err: *const nickel_error,
    out_string: *mut nickel_string,
    format: nickel_error_format,
) -> nickel_result {
    // Safety: `err` is required to be valid pointer to `nickel_error`.
    let err = unsafe { stored_error(err) };
    let mut out = Vec::new();
    if err.format(&mut out, format.into()).is_err() {
        nickel_result::NICKEL_RESULT_ERR
    } else {
        // Nickel's formatting always produces UTF-8.
        // Safety: `out_string` is required to be null or a valid pointer to `nickel_string`.
        unsafe { set_string(out_string, String::from_utf8_lossy(&out).into_owned()) };
        nickel_result::NICKEL_RESULT_OK
    }
}
