//! Programs: what the `nickel` command line tool does, from C.

use std::{
    ffi::{c_char, c_int, c_void},
    mem,
};

use nickel_embed::{
    ExportFormat, FieldInfo, FieldMetadata, FieldPath, InputFormat, MergePriority, Priority,
    Program, ProgramBuilder, Source, TypecheckMode,
};

use crate::{
    CWriter, diagnostics::warning_reporter, expr_or_null, finish, nickel_error, nickel_expr,
    nickel_flush_callback, nickel_record, nickel_result, nickel_string, nickel_write_callback,
    record, set_expr, set_string, usage_error, utf8, write_str,
};

/// Configures a [`nickel_program`]: its sources, import paths, overrides, applied contracts,
/// focused field, trace output and warnings. Nothing is read until
/// [`nickel_program_builder_build`].
pub struct nickel_program_builder {
    inner: ProgramBuilder,
}

impl nickel_program_builder {
    pub(crate) fn update(&mut self, f: impl FnOnce(ProgramBuilder) -> ProgramBuilder) {
        self.inner = f(mem::take(&mut self.inner));
    }
}

/// A Nickel program built from one or more sources, ready to evaluate, export, query or
/// typecheck, like the `nickel` command line tool.
///
/// Allocate one with [`nickel_program_alloc`] and fill it with [`nickel_program_builder_build`].
/// A program keeps its sources and evaluation state, so operations can be repeated, and
/// expressions from [`nickel_program_eval_shallow`] evaluated further with
/// [`nickel_program_eval_expr_shallow`].
pub struct nickel_program {
    inner: Option<Program>,
}

/// What [`nickel_program_query`] found: a field's metadata, its value if it has one, and the
/// names of its sub-fields if its value is a record.
pub struct nickel_field_info {
    inner: Option<FieldInfo>,
    subfields: Vec<String>,
}

/// The metadata of a record field: its documentation, type annotation, contracts, merge
/// priority, and whether it is optional or `not_exported`.
///
/// This is a copy, so it stays valid when the record it came from is freed.
pub struct nickel_field_metadata {
    doc: Option<String>,
    optional: bool,
    not_exported: bool,
    type_annotation: Option<String>,
    contracts: Vec<String>,
    priority: nickel_merge_priority,
    priority_value: f64,
}

impl nickel_field_metadata {
    fn fill(&mut self, m: FieldMetadata<'_>) {
        let (priority, priority_value) = match m.merge_priority() {
            None => (nickel_merge_priority::NICKEL_MERGE_PRIORITY_NEUTRAL, 0.0),
            Some(MergePriority::Default) => {
                (nickel_merge_priority::NICKEL_MERGE_PRIORITY_DEFAULT, 0.0)
            }
            Some(MergePriority::Priority(n)) => (
                nickel_merge_priority::NICKEL_MERGE_PRIORITY_NUMERAL,
                n.as_f64(),
            ),
            Some(MergePriority::Force) => (nickel_merge_priority::NICKEL_MERGE_PRIORITY_FORCE, 0.0),
        };
        *self = Self {
            doc: m.doc().map(str::to_owned),
            optional: m.optional(),
            not_exported: m.not_exported(),
            type_annotation: m.type_annotation(),
            contracts: m.contracts(),
            priority,
            priority_value,
        };
    }
}

/// The format of a source.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum nickel_input_format {
    /// Nickel for in-memory code; for a file, the format its extension names (`.json`,
    /// `.yaml`/`.yml`, `.toml`, `.txt`), or Nickel.
    NICKEL_INPUT_FORMAT_AUTO = 0,
    /// Nickel source.
    NICKEL_INPUT_FORMAT_NICKEL = 1,
    /// JSON.
    NICKEL_INPUT_FORMAT_JSON = 2,
    /// YAML.
    NICKEL_INPUT_FORMAT_YAML = 3,
    /// TOML.
    NICKEL_INPUT_FORMAT_TOML = 4,
    /// Plain text, read as a single string.
    NICKEL_INPUT_FORMAT_TEXT = 5,
}

impl nickel_input_format {
    fn to_rust(self) -> Option<InputFormat> {
        match self {
            Self::NICKEL_INPUT_FORMAT_AUTO => None,
            Self::NICKEL_INPUT_FORMAT_NICKEL => Some(InputFormat::Nickel),
            Self::NICKEL_INPUT_FORMAT_JSON => Some(InputFormat::Json),
            Self::NICKEL_INPUT_FORMAT_YAML => Some(InputFormat::Yaml),
            Self::NICKEL_INPUT_FORMAT_TOML => Some(InputFormat::Toml),
            Self::NICKEL_INPUT_FORMAT_TEXT => Some(InputFormat::Text),
        }
    }
}

/// The format of [`nickel_program_export`]'s output.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum nickel_export_format {
    /// JSON.
    NICKEL_EXPORT_FORMAT_JSON = 0,
    /// YAML.
    NICKEL_EXPORT_FORMAT_YAML = 1,
    /// A stream of YAML documents, one per element of the exported array.
    NICKEL_EXPORT_FORMAT_YAML_DOCUMENTS = 2,
    /// TOML.
    NICKEL_EXPORT_FORMAT_TOML = 3,
    /// A string, as is.
    NICKEL_EXPORT_FORMAT_TEXT = 4,
}

impl From<nickel_export_format> for ExportFormat {
    fn from(f: nickel_export_format) -> Self {
        match f {
            nickel_export_format::NICKEL_EXPORT_FORMAT_JSON => ExportFormat::Json,
            nickel_export_format::NICKEL_EXPORT_FORMAT_YAML => ExportFormat::Yaml,
            nickel_export_format::NICKEL_EXPORT_FORMAT_YAML_DOCUMENTS => {
                ExportFormat::YamlDocuments
            }
            nickel_export_format::NICKEL_EXPORT_FORMAT_TOML => ExportFormat::Toml,
            nickel_export_format::NICKEL_EXPORT_FORMAT_TEXT => ExportFormat::Text,
        }
    }
}

/// How strictly [`nickel_program_typecheck`] checks.
#[repr(C)]
#[derive(Copy, Clone)]
pub enum nickel_typecheck_mode {
    /// Check statically typed code, and only walk the rest, as `nickel typecheck` does.
    NICKEL_TYPECHECK_MODE_WALK = 0,
    /// Check everything as if the whole program were statically typed.
    NICKEL_TYPECHECK_MODE_ENFORCE = 1,
}

/// A merge priority, as in Nickel's `default`, `priority n` and `force`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum nickel_merge_priority {
    /// No annotation, the same as `priority 0`.
    NICKEL_MERGE_PRIORITY_NEUTRAL = 0,
    /// `default`: the lowest priority.
    NICKEL_MERGE_PRIORITY_DEFAULT = 1,
    /// `priority n`.
    NICKEL_MERGE_PRIORITY_NUMERAL = 2,
    /// `force`: the highest priority.
    NICKEL_MERGE_PRIORITY_FORCE = 3,
}

pub(crate) unsafe fn builder<'a>(b: *mut nickel_program_builder) -> &'a mut nickel_program_builder {
    // Safety: `b` came from `nickel_program_builder_alloc`.
    unsafe { b.as_mut().expect("null nickel_program_builder") }
}

pub(crate) unsafe fn program_mut<'a>(
    p: *mut nickel_program,
) -> Result<&'a mut Program, nickel_embed::Error> {
    // Safety: `p` came from `nickel_program_alloc`.
    unsafe { p.as_mut() }
        .expect("null nickel_program")
        .inner
        .as_mut()
        .ok_or_else(|| usage_error("the program has not been built".to_owned()))
}

/// Allocate a new, empty [`nickel_program_builder`]. Free it with
/// [`nickel_program_builder_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_alloc() -> *mut nickel_program_builder {
    Box::into_raw(Box::new(nickel_program_builder {
        inner: Program::builder(),
    }))
}

/// Free a [`nickel_program_builder`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_free(b: *mut nickel_program_builder) {
    if !b.is_null() {
        // Safety: `b` came from `nickel_program_builder_alloc`.
        drop(unsafe { Box::from_raw(b) });
    }
}

/// Add in-memory source code. With several sources, the program is their merge, like
/// `nickel eval a.ncl b.ncl`.
///
/// - `src` is a null-terminated, UTF-8 string. It is copied.
/// - `name`, if not NULL, names the source in error messages.
/// - `format` is the source's format; `NICKEL_INPUT_FORMAT_AUTO` means Nickel.
///
/// Relative imports in the source resolve against the current directory, then the import
/// paths. Fails only if `src` or `name` isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_add_source(
    b: *mut nickel_program_builder,
    src: *const c_char,
    name: *const c_char,
    format: nickel_input_format,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(src, "the source").and_then(|src| {
            let mut source = Source::code(src);
            if !name.is_null() {
                source = source.with_name(utf8(name, "the source name")?);
            }
            if let Some(format) = format.to_rust() {
                source = source.with_format(format);
            }
            Ok(source)
        });
        finish(result, out_error, |s| builder(b).update(|b| b.source(s)))
    }
}

/// Add a file, read when the program is built. With several sources, the program is their
/// merge.
///
/// - `path` is a null-terminated, UTF-8 path.
/// - `format` is the file's format; `NICKEL_INPUT_FORMAT_AUTO` infers it from the extension.
///
/// Relative imports in the file resolve against its directory, then the import paths. Fails
/// only if `path` isn't UTF-8; a missing file is reported by [`nickel_program_builder_build`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_add_file(
    b: *mut nickel_program_builder,
    path: *const c_char,
    format: nickel_input_format,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(path, "the path").map(|path| match format.to_rust() {
            Some(format) => Source::file(path).with_format(format),
            None => Source::file(path),
        });
        finish(result, out_error, |s| builder(b).update(|b| b.source(s)))
    }
}

/// Add a directory to search for imports that aren't found relative to the importing file.
/// `path` is a null-terminated, UTF-8 path. Fails only if it isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_add_import_path(
    b: *mut nickel_program_builder,
    path: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(path, "the import path").map(str::to_owned);
        finish(result, out_error, |p| {
            builder(b).update(|b| b.import_path(p))
        })
    }
}

/// Add the directories in the `NICKEL_IMPORT_PATH` environment variable as import paths, as
/// the `nickel` command does. Embedded programs ignore that variable unless you call this.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_add_import_paths_from_env(
    b: *mut nickel_program_builder,
) {
    // Safety: pre-conditions of this function.
    unsafe { builder(b) }.update(ProgramBuilder::import_paths_from_env);
}

/// Apply the contract in a Nickel file to the whole program, like `--apply-contract`.
/// `path` is a null-terminated, UTF-8 path. Fails only if it isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_apply_contract_file(
    b: *mut nickel_program_builder,
    path: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(path, "the contract path").map(str::to_owned);
        finish(result, out_error, |p| {
            builder(b).update(|b| b.apply_contract_file(p))
        })
    }
}

/// Override a field with a value written in Nickel, like `--override path=value`.
///
/// - `path` is a field path such as `server.port`; segments that aren't plain identifiers go
///   in double quotes (`labels."app.kubernetes.io/name"`).
/// - `value` is Nickel code, evaluated on its own, so it can't refer to other fields.
/// - `priority` is the override's merge priority, and `numeral` the number for
///   `NICKEL_MERGE_PRIORITY_NUMERAL` (ignored otherwise). The command line uses
///   `NICKEL_MERGE_PRIORITY_NEUTRAL`, which replaces `default` values but conflicts with other
///   plain definitions; `NICKEL_MERGE_PRIORITY_FORCE` replaces a field however it was defined.
///
/// Fails if `path` doesn't parse or a string isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_add_override(
    b: *mut nickel_program_builder,
    path: *const c_char,
    value: *const c_char,
    priority: nickel_merge_priority,
    numeral: i64,
    out_error: *mut nickel_error,
) -> nickel_result {
    let priority = match priority {
        nickel_merge_priority::NICKEL_MERGE_PRIORITY_NEUTRAL => Priority::Neutral,
        nickel_merge_priority::NICKEL_MERGE_PRIORITY_DEFAULT => Priority::Default,
        nickel_merge_priority::NICKEL_MERGE_PRIORITY_NUMERAL => Priority::Numeral(numeral),
        nickel_merge_priority::NICKEL_MERGE_PRIORITY_FORCE => Priority::Force,
    };
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(path, "the field path").and_then(|path| {
            let path: FieldPath = path.parse()?;
            Ok((path, utf8(value, "the value")?.to_owned()))
        });
        finish(result, out_error, |(path, value)| {
            builder(b).update(|b| b.override_with_priority(path, value, priority))
        })
    }
}

/// Evaluate, export or query only this field, like `--field`. `path` is a field path, as in
/// [`nickel_program_builder_add_override`]. Fails if it doesn't parse.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_set_field(
    b: *mut nickel_program_builder,
    path: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(path, "the field path").and_then(|p| p.parse::<FieldPath>());
        finish(result, out_error, |p| builder(b).update(|b| b.field(p)))
    }
}

/// Send the output of `std.trace` to `write` (and flush with `flush`, which may be NULL),
/// as [`nickel_context_set_trace_callback`] does for a context. It is discarded
/// otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_set_trace_callback(
    b: *mut nickel_program_builder,
    write: nickel_write_callback,
    flush: nickel_flush_callback,
    user_data: *mut c_void,
) {
    let sink = CWriter {
        write,
        flush,
        context: user_data,
    };
    // Safety: pre-conditions of this function.
    unsafe { builder(b) }.update(|b| b.trace(sink));
}

/// Call `callback` with each warning Nickel reports while evaluating the program. Warnings are
/// discarded otherwise. See [`nickel_warning_callback`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_set_warning_callback(
    b: *mut nickel_program_builder,
    callback: crate::nickel_warning_callback,
    user_data: *mut c_void,
) {
    let reporter = warning_reporter(callback, user_data);
    // Safety: pre-conditions of this function.
    unsafe { builder(b) }.update(|b| b.on_warning(reporter));
}

/// Read the sources and prepare the program, storing it in `out_program` (allocated with
/// [`nickel_program_alloc`], replacing any program it held).
///
/// The builder's configuration is used up: afterwards it is empty, as if newly allocated, and
/// still needs [`nickel_program_builder_free`].
///
/// Fails if there are no sources or one can't be read. Parse and type errors are reported by
/// the operations on the program.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_build(
    b: *mut nickel_program_builder,
    out_program: *mut nickel_program,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let config = mem::take(&mut builder(b).inner);
        finish(config.build(), out_error, |p| {
            if let Some(out) = out_program.as_mut() {
                out.inner = Some(p);
            }
        })
    }
}

/// Allocate a [`nickel_program`] to build into. Free it with [`nickel_program_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_alloc() -> *mut nickel_program {
    Box::into_raw(Box::new(nickel_program { inner: None }))
}

/// Free a [`nickel_program`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_free(p: *mut nickel_program) {
    if !p.is_null() {
        // Safety: `p` came from `nickel_program_alloc`.
        drop(unsafe { Box::from_raw(p) });
    }
}

unsafe fn eval_program(
    p: *mut nickel_program,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
    f: impl FnOnce(&mut Program) -> Result<nickel_embed::Expr, nickel_embed::Error>,
) -> nickel_result {
    // Safety: pre-conditions of the public functions calling this one.
    unsafe {
        let result = program_mut(p).and_then(f);
        finish(result, out_error, |e| set_expr(out_expr, e))
    }
}

/// Evaluate the program fully, like `nickel eval`, storing the result in `out_expr` (if not
/// NULL). On failure, stores the error in `out_error` (if not NULL). All the program functions
/// fail if the program hasn't been built.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_eval_deep(
    p: *mut nickel_program,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe { eval_program(p, out_expr, out_error, Program::eval_deep) }
}

/// Like [`nickel_program_eval_deep`], leaving out fields marked `not_exported`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_eval_deep_for_export(
    p: *mut nickel_program,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe { eval_program(p, out_expr, out_error, Program::eval_deep_for_export) }
}

/// Evaluate the program to weak head normal form, like
/// [`nickel_context_eval_shallow`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_eval_shallow(
    p: *mut nickel_program,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe { eval_program(p, out_expr, out_error, Program::eval_shallow) }
}

/// Evaluate the records of the program but not the other values in it. Fields that can't be
/// evaluated because something they need is missing are left unevaluated rather than failing,
/// so this works on incomplete configurations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_eval_record_spine(
    p: *mut nickel_program,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe { eval_program(p, out_expr, out_error, Program::eval_record_spine) }
}

/// Evaluate an expression from this program a step further, like
/// [`nickel_context_eval_expr_shallow`]. `out_expr` may be `expr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_eval_expr_shallow(
    p: *mut nickel_program,
    expr: *const nickel_expr,
    out_expr: *mut nickel_expr,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let e = expr_or_null(expr);
        eval_program(p, out_expr, out_error, |p| p.eval_expr_shallow(e))
    }
}

/// Evaluate the program and serialize it, like `nickel export`, storing the text in
/// `out_string` (allocated with [`nickel_string_alloc`]). Fields marked `not_exported`
/// are left out.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_export(
    p: *mut nickel_program,
    format: nickel_export_format,
    out_string: *mut nickel_string,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = program_mut(p).and_then(|p| p.export(format.into()));
        finish(result, out_error, |s| set_string(out_string, s))
    }
}

/// Look up the focused field (see [`nickel_program_builder_set_field`]), or the whole program,
/// and store its metadata, value and sub-fields in `out_info` (allocated with
/// [`nickel_field_info_alloc`]) without evaluating it fully, like `nickel query`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_query(
    p: *mut nickel_program,
    out_info: *mut nickel_field_info,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = program_mut(p).and_then(Program::query);
        finish(result, out_error, |info| {
            if let Some(out) = out_info.as_mut() {
                out.subfields = info.subfields();
                out.inner = Some(info);
            }
        })
    }
}

/// Typecheck the program and any applied contracts, like `nickel typecheck`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_typecheck(
    p: *mut nickel_program,
    mode: nickel_typecheck_mode,
    out_error: *mut nickel_error,
) -> nickel_result {
    let mode = match mode {
        nickel_typecheck_mode::NICKEL_TYPECHECK_MODE_WALK => TypecheckMode::Walk,
        nickel_typecheck_mode::NICKEL_TYPECHECK_MODE_ENFORCE => TypecheckMode::Enforce,
    };
    // Safety: pre-conditions of this function.
    unsafe {
        let result = program_mut(p).and_then(|p| p.typecheck(mode));
        finish(result, out_error, |()| ())
    }
}

/// Parse the program without typechecking or evaluating it. Undefined variables and type
/// errors are not reported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_parse(
    p: *mut nickel_program,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = program_mut(p).and_then(Program::parse);
        finish(result, out_error, |()| ())
    }
}

/// Allocate a [`nickel_field_info`] for [`nickel_program_query`] to fill. Free it with
/// [`nickel_field_info_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_alloc() -> *mut nickel_field_info {
    Box::into_raw(Box::new(nickel_field_info {
        inner: None,
        subfields: Vec::new(),
    }))
}

/// Free a [`nickel_field_info`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_free(info: *mut nickel_field_info) {
    if !info.is_null() {
        // Safety: `info` came from `nickel_field_info_alloc`.
        drop(unsafe { Box::from_raw(info) });
    }
}

unsafe fn field_info<'a>(info: *const nickel_field_info) -> &'a nickel_field_info {
    // Safety: `info` came from `nickel_field_info_alloc`.
    unsafe { info.as_ref() }.expect("null nickel_field_info")
}

/// Copy the queried field's metadata into `out_meta` (allocated with
/// [`nickel_field_metadata_alloc`]). Returns 0, leaving `out_meta` alone, if nothing has been
/// queried into `info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_metadata(
    info: *const nickel_field_info,
    out_meta: *mut nickel_field_metadata,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match (&field_info(info).inner, out_meta.as_mut()) {
            (Some(info), Some(out)) => {
                out.fill(info.metadata());
                1
            }
            (Some(_), None) => 1,
            (None, _) => 0,
        }
    }
}

/// Store the queried field's value in `out_expr`, if it has one. It may be unevaluated.
/// Returns 1 if the field has a value, 0 if not.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_value(
    info: *const nickel_field_info,
    out_expr: *mut nickel_expr,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match field_info(info).inner.as_ref().and_then(FieldInfo::value) {
            Some(value) => {
                set_expr(out_expr, value);
                1
            }
            None => 0,
        }
    }
}

/// The number of sub-fields of the queried field, if its value is a record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_subfield_count(info: *const nickel_field_info) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { field_info(info) }.subfields.len()
}

/// The name of the sub-field at `idx`, in source order: written as a pointer to UTF-8 (not
/// null-terminated) and a length, valid until `info` is overwritten or freed. Returns 1, or 0
/// if `idx` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_info_subfield(
    info: *const nickel_field_info,
    idx: usize,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match field_info(info).subfields.get(idx) {
            Some(name) => {
                write_str(name, out_str, out_len);
                1
            }
            None => 0,
        }
    }
}

/// Allocate a [`nickel_field_metadata`]. Free it with [`nickel_field_metadata_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_alloc() -> *mut nickel_field_metadata {
    Box::into_raw(Box::new(nickel_field_metadata {
        doc: None,
        optional: false,
        not_exported: false,
        type_annotation: None,
        contracts: Vec::new(),
        priority: nickel_merge_priority::NICKEL_MERGE_PRIORITY_NEUTRAL,
        priority_value: 0.0,
    }))
}

/// Free a [`nickel_field_metadata`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_free(meta: *mut nickel_field_metadata) {
    if !meta.is_null() {
        // Safety: `meta` came from `nickel_field_metadata_alloc`.
        drop(unsafe { Box::from_raw(meta) });
    }
}

/// Copy the metadata of the field named `key` into `out_meta`. Returns 1 if the record has the
/// field, 0 (leaving `out_meta` alone) if not.
///
/// Metadata is only kept on records from shallow evaluation, [`nickel_program_query`] and
/// [`nickel_program_eval_record_spine`]; deep evaluation drops it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_record_field_metadata_by_name(
    rec: *const nickel_record,
    key: *const c_char,
    out_meta: *mut nickel_field_metadata,
) -> c_int {
    if rec.is_null() {
        return 0;
    }
    // Safety: pre-conditions of this function.
    unsafe {
        let Ok(key) = utf8(key, "the key") else {
            return 0;
        };
        match record(rec).and_then(|r| r.field(key)) {
            Some(field) => {
                if let Some(out) = out_meta.as_mut() {
                    out.fill(field.metadata());
                }
                1
            }
            None => 0,
        }
    }
}

/// Copy the metadata of the field at `idx` (in the order of
/// [`nickel_record_key_value_by_index`]) into `out_meta`. Returns 1, or 0 if `idx` is
/// out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_record_field_metadata_by_index(
    rec: *const nickel_record,
    idx: usize,
    out_meta: *mut nickel_field_metadata,
) -> c_int {
    if rec.is_null() {
        return 0;
    }
    // Safety: pre-conditions of this function.
    unsafe {
        match record(rec).and_then(|r| r.fields().nth(idx)) {
            Some((_, field)) => {
                if let Some(out) = out_meta.as_mut() {
                    out.fill(field.metadata());
                }
                1
            }
            None => 0,
        }
    }
}

unsafe fn metadata<'a>(meta: *const nickel_field_metadata) -> &'a nickel_field_metadata {
    // Safety: `meta` came from `nickel_field_metadata_alloc`.
    unsafe { meta.as_ref() }.expect("null nickel_field_metadata")
}

unsafe fn write_opt_str(
    s: Option<&str>,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    match s {
        Some(s) => {
            // Safety: the out-pointers are null or valid.
            unsafe { write_str(s, out_str, out_len) };
            1
        }
        None => 0,
    }
}

/// The field's documentation, as a pointer to UTF-8 (not null-terminated) and a length, valid
/// until `meta` is overwritten or freed. Returns 1, or 0 if the field has no documentation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_doc(
    meta: *const nickel_field_metadata,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe { write_opt_str(metadata(meta).doc.as_deref(), out_str, out_len) }
}

/// The field's type annotation, rendered as Nickel, like
/// [`nickel_field_metadata_doc`]. Returns 0 if the field has none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_type_annotation(
    meta: *const nickel_field_metadata,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe { write_opt_str(metadata(meta).type_annotation.as_deref(), out_str, out_len) }
}

/// The number of contracts annotating the field.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_contract_count(
    meta: *const nickel_field_metadata,
) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { metadata(meta) }.contracts.len()
}

/// The contract at `idx`, rendered as Nickel, like [`nickel_field_metadata_doc`]. Returns 0 if
/// `idx` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_contract(
    meta: *const nickel_field_metadata,
    idx: usize,
    out_str: *mut *const c_char,
    out_len: *mut usize,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        let contract = metadata(meta).contracts.get(idx).map(String::as_str);
        write_opt_str(contract, out_str, out_len)
    }
}

/// Is the field marked `optional`?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_is_optional(
    meta: *const nickel_field_metadata,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe { metadata(meta) }.optional as c_int
}

/// Is the field marked `not_exported`?
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_is_not_exported(
    meta: *const nickel_field_metadata,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe { metadata(meta) }.not_exported as c_int
}

/// The field's merge priority. For `NICKEL_MERGE_PRIORITY_NUMERAL`, the number is written to
/// `out_value` (if not NULL), rounded to the nearest `double`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_field_metadata_priority(
    meta: *const nickel_field_metadata,
    out_value: *mut f64,
) -> nickel_merge_priority {
    // Safety: pre-conditions of this function.
    unsafe {
        let meta = metadata(meta);
        if meta.priority == nickel_merge_priority::NICKEL_MERGE_PRIORITY_NUMERAL
            && !out_value.is_null()
        {
            *out_value = meta.priority_value;
        }
        meta.priority
    }
}
