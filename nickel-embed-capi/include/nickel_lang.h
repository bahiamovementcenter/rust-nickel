// SPDX-License-Identifier: MIT

#ifndef NICKEL_LANG_H
#define NICKEL_LANG_H

#include <stdint.h>

/**
 * For functions that can fail, these are the interpretations of the return value.
 */
typedef enum {
    /**
     * A successful result.
     */
    NICKEL_RESULT_OK = 0,
    /**
     * A bad result.
     */
    NICKEL_RESULT_ERR = 1,
} nickel_result;

/**
 * For functions that can fail, these are the interpretations of the return value.
 */
typedef enum {
    /**
     * Format an error as human-readable text.
     */
    NICKEL_ERROR_FORMAT_TEXT = 0,
    /**
     * Format an error as human-readable text, with ANSI color codes.
     */
    NICKEL_ERROR_FORMAT_ANSI_TEXT = 1,
    /**
     * Format an error as JSON.
     */
    NICKEL_ERROR_FORMAT_JSON = 2,
    /**
     * Format an error as YAML.
     */
    NICKEL_ERROR_FORMAT_YAML = 3,
    /**
     * Format an error as TOML.
     */
    NICKEL_ERROR_FORMAT_TOML = 4,
} nickel_error_format;

/**
 * What kind of failure a [`nickel_error`] is.
 */
typedef enum {
    /**
     * The `nickel_error` holds no error.
     */
    NICKEL_ERROR_KIND_NONE = 0,
    /**
     * The source isn't valid syntax.
     */
    NICKEL_ERROR_KIND_PARSE = 1,
    /**
     * An import couldn't be found or read.
     */
    NICKEL_ERROR_KIND_IMPORT = 2,
    /**
     * A static type error.
     */
    NICKEL_ERROR_KIND_TYPECHECK = 3,
    /**
     * A value broke a contract.
     */
    NICKEL_ERROR_KIND_CONTRACT = 4,
    /**
     * Any other evaluation failure, such as a missing field or a failed assertion.
     */
    NICKEL_ERROR_KIND_EVAL = 5,
    /**
     * The value can't be serialized in the requested format.
     */
    NICKEL_ERROR_KIND_EXPORT = 6,
    /**
     * Reading a source failed, or a function was given a bad argument (such as a string that
     * isn't UTF-8, or a program that hasn't been built).
     */
    NICKEL_ERROR_KIND_IO = 7,
    /**
     * A failure with no more specific kind.
     */
    NICKEL_ERROR_KIND_OTHER = 8,
    /**
     * A package manifest couldn't be read or its dependencies resolved or fetched.
     */
    NICKEL_ERROR_KIND_PACKAGE = 9,
} nickel_error_kind;

/**
 * How serious a [`nickel_diagnostic`] is.
 */
typedef enum {
    /**
     * A suggestion.
     */
    NICKEL_SEVERITY_HELP = 0,
    /**
     * Extra information.
     */
    NICKEL_SEVERITY_NOTE = 1,
    /**
     * A warning.
     */
    NICKEL_SEVERITY_WARNING = 2,
    /**
     * An error.
     */
    NICKEL_SEVERITY_ERROR = 3,
    /**
     * An internal error in Nickel itself.
     */
    NICKEL_SEVERITY_BUG = 4,
} nickel_severity;

/**
 * The format of a source.
 */
typedef enum {
    /**
     * Nickel for in-memory code; for a file, the format its extension names (`.json`,
     * `.yaml`/`.yml`, `.toml`, `.txt`), or Nickel.
     */
    NICKEL_INPUT_FORMAT_AUTO = 0,
    /**
     * Nickel source.
     */
    NICKEL_INPUT_FORMAT_NICKEL = 1,
    /**
     * JSON.
     */
    NICKEL_INPUT_FORMAT_JSON = 2,
    /**
     * YAML.
     */
    NICKEL_INPUT_FORMAT_YAML = 3,
    /**
     * TOML.
     */
    NICKEL_INPUT_FORMAT_TOML = 4,
    /**
     * Plain text, read as a single string.
     */
    NICKEL_INPUT_FORMAT_TEXT = 5,
} nickel_input_format;

/**
 * A merge priority, as in Nickel's `default`, `priority n` and `force`.
 */
typedef enum {
    /**
     * No annotation, the same as `priority 0`.
     */
    NICKEL_MERGE_PRIORITY_NEUTRAL = 0,
    /**
     * `default`: the lowest priority.
     */
    NICKEL_MERGE_PRIORITY_DEFAULT = 1,
    /**
     * `priority n`.
     */
    NICKEL_MERGE_PRIORITY_NUMERAL = 2,
    /**
     * `force`: the highest priority.
     */
    NICKEL_MERGE_PRIORITY_FORCE = 3,
} nickel_merge_priority;

/**
 * The format of [`nickel_program_export`]'s output.
 */
typedef enum {
    /**
     * JSON.
     */
    NICKEL_EXPORT_FORMAT_JSON = 0,
    /**
     * YAML.
     */
    NICKEL_EXPORT_FORMAT_YAML = 1,
    /**
     * A stream of YAML documents, one per element of the exported array.
     */
    NICKEL_EXPORT_FORMAT_YAML_DOCUMENTS = 2,
    /**
     * TOML.
     */
    NICKEL_EXPORT_FORMAT_TOML = 3,
    /**
     * A string, as is.
     */
    NICKEL_EXPORT_FORMAT_TEXT = 4,
} nickel_export_format;

/**
 * How strictly [`nickel_program_typecheck`] checks.
 */
typedef enum {
    /**
     * Check statically typed code, and only walk the rest, as `nickel typecheck` does.
     */
    NICKEL_TYPECHECK_MODE_WALK = 0,
    /**
     * Check everything as if the whole program were statically typed.
     */
    NICKEL_TYPECHECK_MODE_ENFORCE = 1,
} nickel_typecheck_mode;

/**
 * The format of [`nickel_program_docs`]'s output.
 */
typedef enum {
    /**
     * Every field with its type, contracts and documentation, as JSON, like
     * `nickel doc --format json`.
     */
    NICKEL_DOC_FORMAT_JSON = 0,
    /**
     * Markdown, like `nickel doc --format markdown`.
     */
    NICKEL_DOC_FORMAT_MARKDOWN = 1,
} nickel_doc_format;

/**
 * A Nickel array.
 *
 * See [`nickel_expr_is_array`] and [`nickel_expr_as_array`].
 */
typedef struct nickel_array nickel_array;

/**
 * The main entry point.
 */
typedef struct nickel_context nickel_context;

/**
 * One message about a problem, with the places in the source it points at.
 *
 * Get one from [`nickel_error_diagnostic`], or in a [`nickel_warning_callback`]. It borrows
 * from where it came from: the error (until it is overwritten or freed), or the callback
 * (until it returns).
 */
typedef struct nickel_diagnostic nickel_diagnostic;

/**
 * A Nickel error.
 *
 * If you want to collect an error message from a fallible function
 * (like `nickel_context_eval_deep`), first allocate an error using
 * `nickel_error_alloc`, and then pass the resulting pointer to your fallible
 * function. If that function fails, it will save the error data in your
 * `nickel_error`.
 */
typedef struct nickel_error nickel_error;

/**
 * A Nickel expression.
 *
 * This might be fully evaluated (for example, if you got it from [`nickel_context_eval_deep`])
 * or might have unevaluated sub-expressions (if you got it from [`nickel_context_eval_shallow`]).
 */
typedef struct nickel_expr nickel_expr;

/**
 * What [`nickel_program_query`] found: a field's metadata, its value if it has one, and the
 * names of its sub-fields if its value is a record.
 */
typedef struct nickel_field_info nickel_field_info;

/**
 * The metadata of a record field: its documentation, type annotation, contracts, merge
 * priority, and whether it is optional or `not_exported`.
 *
 * This is a copy, so it stays valid when the record it came from is freed.
 */
typedef struct nickel_field_metadata nickel_field_metadata;

/**
 * A place in a source that a [`nickel_diagnostic`] points at. It borrows from the diagnostic.
 */
typedef struct nickel_label nickel_label;

/**
 * A Nickel number.
 *
 * See [`nickel_expr_is_number`] and [`nickel_expr_as_number`].
 */
typedef struct nickel_number nickel_number;

/**
 * Where package imports (`import foo`) are found: a map from package names to directories.
 *
 * Each package is a directory, and importing it evaluates its `main.ncl`. Names are local to
 * the importer: the program's own packages are added with [`nickel_package_map_add_package`],
 * and those of a package with [`nickel_package_map_add_dependency`], so two packages can each
 * have their own `foo`. [`nickel_package_map_load_manifest`] instead fills the map from a
 * `Nickel-pkg.ncl` manifest, the way the `nickel` command does.
 *
 * Use a map with [`nickel_context_set_package_map`] or
 * [`nickel_program_builder_set_package_map`], which copy it. Allocate one with
 * [`nickel_package_map_alloc`] and free it with [`nickel_package_map_free`].
 */
typedef struct nickel_package_map nickel_package_map;

/**
 * A Nickel program built from one or more sources, ready to evaluate, export, query or
 * typecheck, like the `nickel` command line tool.
 *
 * Allocate one with [`nickel_program_alloc`] and fill it with [`nickel_program_builder_build`].
 * A program keeps its sources and evaluation state, so operations can be repeated, and
 * expressions from [`nickel_program_eval_shallow`] evaluated further with
 * [`nickel_program_eval_expr_shallow`].
 */
typedef struct nickel_program nickel_program;

/**
 * Configures a [`nickel_program`]: its sources, import paths, overrides, applied contracts,
 * focused field, trace output and warnings. Nothing is read until
 * [`nickel_program_builder_build`].
 */
typedef struct nickel_program_builder nickel_program_builder;

/**
 * A Nickel record.
 *
 * See [`nickel_expr_is_record`] and [`nickel_expr_as_record`].
 */
typedef struct nickel_record nickel_record;

/**
 * A Nickel string.
 */
typedef struct nickel_string nickel_string;

/**
 * A callback function for writing data.
 *
 * This function will be called with a buffer (`buf`) of data, having length
 * `len`. It need not consume the entire buffer, and should return the number
 * of bytes consumed.
 */
typedef uintptr_t (*nickel_write_callback)(void *context, const uint8_t *buf, uintptr_t len);

/**
 * A callback function for flushing data that was written by a write callback.
 */
typedef void (*nickel_flush_callback)(const void *context);

/**
 * A callback for warnings: called with `user_data` and the warning, which is only valid during
 * the call.
 */
typedef void (*nickel_warning_callback)(void *user_data, const nickel_diagnostic *diagnostic);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Allocate a new [`nickel_context`], which can be used to evaluate Nickel expressions.
 *
 * Returns a newly-allocated [`nickel_context`] that can be freed with [`nickel_context_free`].
 */
nickel_context *nickel_context_alloc(void);

/**
 * Free a [`nickel_context`] that was created with [`nickel_context_alloc`].
 */
void nickel_context_free(nickel_context *ctx);

/**
 * Provide a callback that will be called when evaluating Nickel
 * code that uses `std.trace`.
 */
void nickel_context_set_trace_callback(nickel_context *ctx,
                                       nickel_write_callback write,
                                       nickel_flush_callback flush,
                                       void *user_data);

/**
 * Provide a name for the main input program.
 *
 * This is used to format error messages. If you read the main input
 * program from a file, its path is a good choice.
 *
 * `name` should be a UTF-8-encoded, null-terminated string. It is only
 * borrowed temporarily; the pointer need not remain valid.
 */
void nickel_context_set_source_name(nickel_context *ctx, const char *name);

/**
 * Evaluate a Nickel program deeply.
 *
 * "Deeply" means that we recursively evaluate records and arrays. For
 * an alternative, see [`nickel_context_eval_shallow`].
 *
 * - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
 * - `out_expr` either NULL or something that was created with [`nickel_expr_alloc`]
 * - `out_error` can be NULL if you aren't interested in getting detailed
 *   error messages
 *
 * If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces
 * the value at `out_expr` (if non-NULL) with the newly-evaluated Nickel expression.
 *
 * If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the
 * value at `out_error` (if non-NULL) by a pointer to a newly-allocated Nickel error.
 * That error should be freed with `nickel_error_free` when you are
 * done with it.
 */
nickel_result nickel_context_eval_deep(nickel_context *ctx,
                                       const char *src,
                                       nickel_expr *out_expr,
                                       nickel_error *out_error);

/**
 * Evaluate a Nickel program deeply.
 *
 * This differs from [`nickel_context_eval_deep`] in that it ignores
 * fields marked as `not_exported`.
 *
 * - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
 * - `out_expr` either NULL or something that was created with [`nickel_expr_alloc`]
 * - `out_error` can be NULL if you aren't interested in getting detailed
 *   error messages
 *
 * If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces
 * the value at `out_expr` (if non-NULL) with the newly-evaluated Nickel expression.
 *
 * If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the
 * value at `out_error` (if non-NULL) by a pointer to a newly-allocated Nickel error.
 * That error should be freed with `nickel_error_free` when you are
 * done with it.
 */
nickel_result nickel_context_eval_deep_for_export(nickel_context *ctx,
                                                  const char *src,
                                                  nickel_expr *out_expr,
                                                  nickel_error *out_error);

/**
 * Evaluate a Nickel program to weak head normal form (WHNF).
 *
 * The result of this evaluation is a null, bool, number, string,
 * enum, record, or array. In case it's a record, array, or enum
 * variant, the payload (record values, array elements, or enum
 * payloads) will be left unevaluated.
 *
 * Sub-expressions of the result can be evaluated further by [nickel_context_eval_expr_shallow].
 *
 * - `src` is a null-terminated string containing UTF-8-encoded Nickel source.
 * - `out_expr` is either NULL or something that was created with [`nickel_expr_alloc`]
 * - `out_error` can be NULL if you aren't interested in getting detailed
 *   error messages
 *
 * If evaluation is successful, returns `NICKEL_RESULT_OK` and replaces the value at `out_expr`
 * (if non-NULL) with the newly-evaluated Nickel expression.
 *
 * If evaluation fails, returns `NICKEL_RESULT_ERR` and replaces the value at `out_error` (if
 * non-NULL) by a pointer to a newly-allocated Nickel error. That error should be freed with
 * `nickel_error_free` when you are done with it.
 */
nickel_result nickel_context_eval_shallow(nickel_context *ctx,
                                          const char *src,
                                          nickel_expr *out_expr,
                                          nickel_error *out_error);

/**
 * Allocate a new Nickel expression.
 *
 * The returned expression pointer can be used to store the results of
 * evaluation, for example by passing it as the `out_expr` location of
 * `nickel_context_eval_deep`.
 *
 * Each call to `nickel_expr_alloc` should be paired with a call to
 * `nickel_expr_free`. The various functions (like `nickel_context_eval_deep`)
 * that take an `out_expr` parameter overwrite the existing expression
 * contents, and do not affect the pairing of `nickel_expr_alloc` and
 * `nickel_expr_free`.
 *
 * For example:
 *
 * ```c
 * nickel_context *ctx = nickel_context_alloc();
 * nickel_context *expr = nickel_expr_alloc();
 *
 * nickel_context_eval_deep(ctx, "{ foo = 1 }", expr, NULL);
 *
 * // now expr is a record
 * printf("record: %d\n", nickel_expr_is_record(expr));
 *
 * nickel_context_eval_deep(ctx, "[1, 2, 3]", expr, NULL);
 *
 * // now expr is an array
 * printf("array: %d\n", nickel_expr_is_array(expr));
 *
 * // the calls to nickel_context_eval_deep haven't created any new exprs:
 * // we only need to free it once
 * nickel_expr_free(expr);
 * nickel_context_free(ctx);
 * ```
 *
 * An `Expr` owns its data. There are various ways to get a reference to
 * data owned by an expression, which are then invalidated when the expression
 * is freed (by `nickel_expr_free`) or overwritten (for example, by
 * `nickel_context_deep_eval`).
 *
 * ```c
 * nickel_context *ctx = nickel_context_alloc();
 * nickel_expr *expr = nickel_expr_alloc();
 *
 * nickel_context_eval_deep(ctx, "{ foo = 1 }", expr, NULL);
 *
 * nickel_record *rec = nickel_expr_as_record(expr);
 * nickel_expr *field = nickel_expr_alloc();
 * nickel_record_value_by_name(rec, "foo", field);
 *
 * // Now `rec` points to data owned by `expr`, but `field`
 * // owns its own data. The following deallocation invalidates
 * // `rec`, but not `field`.
 * nickel_expr_free(expr);
 * printf("number: %d\n", nickel_expr_is_number(field));
 * ```
 */
nickel_expr *nickel_expr_alloc(void);

/**
 * Free a Nickel expression.
 *
 * See [`nickel_expr_alloc`].
 */
void nickel_expr_free(nickel_expr *expr);

/**
 * Is this expression a boolean?
 */
int nickel_expr_is_bool(const nickel_expr *expr);

/**
 * Is this expression a number?
 */
int nickel_expr_is_number(const nickel_expr *expr);

/**
 * Is this expression a string?
 */
int nickel_expr_is_str(const nickel_expr *expr);

/**
 * Is this expression an enum tag?
 */
int nickel_expr_is_enum_tag(const nickel_expr *expr);

/**
 * Is this expression an enum variant?
 */
int nickel_expr_is_enum_variant(const nickel_expr *expr);

/**
 * Is this expression a record?
 */
int nickel_expr_is_record(const nickel_expr *expr);

/**
 * Is this expression an array?
 */
int nickel_expr_is_array(const nickel_expr *expr);

/**
 * Has this expression been evaluated?
 *
 * An evaluated expression is either null, or it's a number, bool, string, record, array, or enum.
 * If this expression is not a value, you probably got it from looking inside the result of
 * [`nickel_context_eval_shallow`], and you can use the [`nickel_context_eval_expr_shallow`] to
 * evaluate this expression further.
 */
int nickel_expr_is_value(const nickel_expr *expr);

/**
 * Is this expression null?
 */
int nickel_expr_is_null(const nickel_expr *expr);

/**
 * If this expression is a boolean, returns that boolean.
 *
 * # Panics
 *
 * Panics if `expr` is not a boolean.
 */
int nickel_expr_as_bool(const nickel_expr *expr);

/**
 * If this expression is a string, returns that string.
 *
 * A pointer to the string contents, which are UTF-8 encoded, is returned in
 * `out_str`. These contents are *not* null-terminated. The return value of this
 * function is the length of these contents.
 *
 * The returned string contents are owned by this `Expr`, and will be invalidated
 * when the `Expr` is freed with [`nickel_expr_free`].
 *
 * # Panics
 *
 * Panics if `expr` is not a string.
 */
uintptr_t nickel_expr_as_str(const nickel_expr *expr, const char **out_str);

/**
 * If this expression is a number, returns the number.
 *
 * The returned number pointer borrows from `expr`, and will be invalidated
 * when `expr` is overwritten or freed.
 *
 * # Panics
 *
 * Panics if `expr` is not an number.
 */
const nickel_number *nickel_expr_as_number(const nickel_expr *expr);

/**
 * If this expression is an enum tag, returns its string value.
 *
 * A pointer to the string contents, which are UTF-8 encoded, is returned in
 * `out_str`. These contents are *not* null-terminated. The return value of this
 * function is the length of these contents.
 *
 * The returned string contents point to an interned string and will never be
 * invalidated.
 *
 * # Panics
 *
 * Panics if `expr` is null or is not an enum tag.
 */
uintptr_t nickel_expr_as_enum_tag(const nickel_expr *expr, const char **out_str);

/**
 * If this expression is an enum variant, returns its string value and its payload.
 *
 * A pointer to the string contents, which are UTF-8 encoded, is returned in
 * `out_str`. These contents are *not* null-terminated. The return value of this
 * function is the length of these contents.
 *
 * The returned string contents point to an interned string and will never be
 * invalidated.
 *
 * # Panics
 *
 * Panics if `expr` is not an enum tag.
 */
uintptr_t nickel_expr_as_enum_variant(const nickel_expr *expr,
                                      const char **out_str,
                                      nickel_expr *out_expr);

/**
 * If this expression is a record, returns the record.
 *
 * The returned record pointer borrows from `expr`, and will be invalidated
 * when `expr` is overwritten or freed.
 *
 * # Panics
 *
 * Panics if `expr` is not an record.
 */
const nickel_record *nickel_expr_as_record(const nickel_expr *expr);

/**
 * If this expression is an array, returns the array.
 *
 * The returned array pointer borrows from `expr`, and will be invalidated
 * when `expr` is overwritten or freed.
 *
 * # Panics
 *
 * Panics if `expr` is not an array.
 */
const nickel_array *nickel_expr_as_array(const nickel_expr *expr);

/**
 * Converts an expression to JSON.
 *
 * This is fallible because enum variants have no canonical conversion to
 * JSON: if the expression contains any enum variants, this will fail.
 * This also fails if the expression contains any unevaluated sub-expressions.
 */
nickel_result nickel_context_expr_to_json(nickel_context *ctx,
                                          const nickel_expr *expr,
                                          nickel_string *out_string,
                                          nickel_error *out_err);

/**
 * Converts an expression to YAML.
 *
 * This is fallible because enum variants have no canonical conversion to
 * YAML: if the expression contains any enum variants, this will fail.
 * This also fails if the expression contains any unevaluated sub-expressions.
 */
nickel_result nickel_context_expr_to_yaml(nickel_context *ctx,
                                          const nickel_expr *expr,
                                          nickel_string *out_string,
                                          nickel_error *out_err);

/**
 * Converts an expression to TOML.
 *
 * This is fallible because enum variants have no canonical conversion to
 * TOML: if the expression contains any enum variants, this will fail.
 * This also fails if the expression contains any unevaluated sub-expressions.
 */
nickel_result nickel_context_expr_to_toml(nickel_context *ctx,
                                          const nickel_expr *expr,
                                          nickel_string *out_string,
                                          nickel_error *out_err);

/**
 * Is this number an integer within the range of an `int64_t`?
 */
int nickel_number_is_i64(const nickel_number *num);

/**
 * If this number is an integer within the range of an `int64_t`, returns it.
 *
 * # Panics
 *
 * Panics if this number is not an integer in the appropriate range (you should
 * check with [`nickel_number_is_i64`] first).
 */
int64_t nickel_number_as_i64(const nickel_number *num);

/**
 * The value of this number, rounded to the nearest `double`.
 */
double nickel_number_as_f64(const nickel_number *num);

/**
 * The value of this number, as an exact rational number.
 *
 * - `out_numerator` must have been allocated with [`nickel_string_alloc`]. It
 *   will be overwritten with the numerator, as a decimal string.
 * - `out_denominator` must have been allocated with [`nickel_string_alloc`].
 *   It will be overwritten with the denominator, as a decimal string.
 */
void nickel_number_as_rational(const nickel_number *num,
                               nickel_string *out_numerator,
                               nickel_string *out_denominator);

/**
 * The number of elements of this Nickel array.
 */
uintptr_t nickel_array_len(const nickel_array *arr);

/**
 * Retrieve the element at the given array index.
 *
 * The retrieved element will be written to `out_expr`, which must have been allocated with
 * [`nickel_expr_alloc`].
 *
 * # Panics
 *
 * Panics if the given index is out of bounds.
 */
void nickel_array_get(const nickel_array *arr, uintptr_t idx, nickel_expr *out_expr);

/**
 * The number of keys in this Nickel record.
 */
uintptr_t nickel_record_len(const nickel_record *rec);

/**
 * Retrieve the key and value at the given index.
 *
 * If this record was deeply evaluated, every key will come with a value.
 * However, shallowly evaluated records may have fields with no value.
 *
 * Returns 1 if the key came with a value, and 0 if it didn't. The value
 * will be written to `out_expr` if it is non-NULL.
 *
 * # Panics
 *
 * Panics if `idx` is out of range.
 */
int nickel_record_key_value_by_index(const nickel_record *rec,
                                     uintptr_t idx,
                                     const char **out_key,
                                     uintptr_t *out_key_len,
                                     nickel_expr *out_expr);

/**
 * Look up a key in this record and return its value, if there is one.
 *
 * Returns 1 if the key has a value, and 0 if it didn't. The value is
 * written to `out_expr` if it is non-NULL.
 */
int nickel_record_value_by_name(const nickel_record *rec, const char *key, nickel_expr *out_expr);

/**
 * Allocates a new string.
 *
 * The lifecycle management of a string is much like that of an expression
 * (see `nickel_expr_alloc`). It gets allocated here, modified by various other
 * functions, and finally is freed by a call to `nickel_string_free`.
 */
nickel_string *nickel_string_alloc(void);

/**
 * Frees a string.
 */
void nickel_string_free(nickel_string *s);

/**
 * Retrieve the data inside a string.
 *
 * A pointer to the string contents, which are UTF-8 encoded, is written to
 * `data`. These contents are *not* null-terminated, but their length (in bytes)
 * is written to `len`. The string contents will be invalidated when `s` is
 * freed or overwritten.
 */
void nickel_string_data(const nickel_string *s, const char **data, uintptr_t *len);

/**
 * Evaluate an expression to weak head normal form (WHNF).
 *
 * This has no effect if the expression is already evaluated (see
 * [`nickel_expr_is_value`]).
 *
 * The result of this evaluation is a null, bool, number, string,
 * enum, record, or array. In case it's a record, array, or enum
 * variant, the payload (record values, array elements, or enum
 * payloads) will be left unevaluated.
 */
nickel_result nickel_context_eval_expr_shallow(nickel_context *ctx,
                                               const nickel_expr *expr,
                                               nickel_expr *out_expr,
                                               nickel_error *out_error);

/**
 * Allocate a new `nickel_error`.
 */
nickel_error *nickel_error_alloc(void);

/**
 * Frees a `nickel_error`.
 */
void nickel_error_free(nickel_error *err);

/**
 * Write out an error as a user- or machine-readable diagnostic.
 *
 * - `err` must have been allocated by `nickel_error_alloc` and initialized by some failing
 *   function (like `nickel_context_eval_deep`).
 * - `write` is a callback function that will be invoked with UTF-8 encoded data.
 * - `write_payload` is optional extra data to pass to `write`
 * - `format` selects the error-rendering format.
 */
nickel_result nickel_error_display(const nickel_error *err,
                                   nickel_write_callback write,
                                   void *write_payload,
                                   nickel_error_format format);

/**
 * Write out an error as a user- or machine-readable diagnostic.
 *
 * This is like `nickel_error_format`, but writes the error to a string instead
 * of via a callback function.
 */
nickel_result nickel_error_format_as_string(const nickel_error *err,
                                            nickel_string *out_string,
                                            nickel_error_format format);

/**
 * Call `callback` with each warning Nickel reports while evaluating in this context. Warnings
 * are discarded otherwise.
 */
void nickel_context_set_warning_callback(nickel_context *ctx,
                                         nickel_warning_callback callback,
                                         void *user_data);

/**
 * Add a directory to search for imports that aren't found relative to the importing file.
 *
 * `path` is a null-terminated string, copied. Like Nickel's own C API, contexts don't read
 * the `NICKEL_IMPORT_PATH` environment variable.
 */
void nickel_context_add_import_path(nickel_context *ctx, const char *path);

/**
 * What kind of failure this error is, or `NICKEL_ERROR_KIND_NONE` if it holds no error.
 */
nickel_error_kind nickel_error_get_kind(const nickel_error *err);

/**
 * The number of diagnostics in this error: usually one, but a parse error, for example, has
 * one per problem found. Zero if it holds no error.
 */
uintptr_t nickel_error_diagnostic_count(const nickel_error *err);

/**
 * The diagnostic at `idx`, or NULL if `idx` is out of range. It is valid until `err` is
 * overwritten or freed.
 */
const nickel_diagnostic *nickel_error_diagnostic(const nickel_error *err, uintptr_t idx);

/**
 * How serious this diagnostic is.
 */
nickel_severity nickel_diagnostic_severity(const nickel_diagnostic *d);

/**
 * The diagnostic's message. A pointer to UTF-8 (not null-terminated) is written to `out_str`
 * and its length returned; it is valid as long as the diagnostic.
 */
uintptr_t nickel_diagnostic_message(const nickel_diagnostic *d, const char **out_str);

/**
 * The diagnostic's code, a short identifier for the problem, written as in
 * [`nickel_field_metadata_doc`]. Returns 1, or 0 if Nickel gave none.
 */
int nickel_diagnostic_code(const nickel_diagnostic *d, const char **out_str, uintptr_t *out_len);

/**
 * The number of notes: extra explanation not tied to a place in the source.
 */
uintptr_t nickel_diagnostic_note_count(const nickel_diagnostic *d);

/**
 * The note at `idx`, written as in [`nickel_field_metadata_doc`]. Returns 1, or 0 if
 * `idx` is out of range.
 */
int nickel_diagnostic_note(const nickel_diagnostic *d,
                           uintptr_t idx,
                           const char **out_str,
                           uintptr_t *out_len);

/**
 * The number of places in the source this diagnostic points at.
 */
uintptr_t nickel_diagnostic_label_count(const nickel_diagnostic *d);

/**
 * The label at `idx`, or NULL if `idx` is out of range. It is valid as long as the diagnostic.
 */
const nickel_label *nickel_diagnostic_label(const nickel_diagnostic *d, uintptr_t idx);

/**
 * Is this where the problem is (1), or context for it (0)?
 */
int nickel_label_is_primary(const nickel_label *l);

/**
 * The label's message, which may be empty, returned like [`nickel_diagnostic_message`].
 */
uintptr_t nickel_label_message(const nickel_label *l, const char **out_str);

/**
 * The name of the source the label points into (a file path, or the name given to in-memory
 * code), returned like [`nickel_diagnostic_message`].
 */
uintptr_t nickel_label_source(const nickel_label *l, const char **out_str);

/**
 * The label's position: the byte range `[start, end)` in the source, and the 1-based line and
 * column where it starts and ends. Each out-pointer may be NULL.
 */
void nickel_label_position(const nickel_label *l,
                           uintptr_t *out_start_byte,
                           uintptr_t *out_end_byte,
                           uintptr_t *out_start_line,
                           uintptr_t *out_start_column,
                           uintptr_t *out_end_line,
                           uintptr_t *out_end_column);

/**
 * Allocate a new, empty [`nickel_package_map`]: every package import fails until packages are
 * added. Free it with [`nickel_package_map_free`].
 */
nickel_package_map *nickel_package_map_alloc(void);

/**
 * Free a [`nickel_package_map`].
 */
void nickel_package_map_free(nickel_package_map *m);

/**
 * Make `import name` in the program's own sources load the package in the directory `dir`,
 * replacing any package of that name.
 *
 * `name` and `dir` are null-terminated, UTF-8 strings, copied. A relative `dir` is taken from
 * the current directory. Fails only if a string isn't UTF-8; a missing directory is reported
 * when the package is imported.
 */
nickel_result nickel_package_map_add_package(nickel_package_map *m,
                                             const char *name,
                                             const char *dir,
                                             nickel_error *out_error);

/**
 * Make `import name` inside the package in the directory `parent` load the package in `dir`.
 *
 * `parent` must be the directory the parent package was added with (by
 * [`nickel_package_map_add_package`] or as another package's dependency). Strings are as in
 * [`nickel_package_map_add_package`]. Fails only if a string isn't UTF-8.
 */
nickel_result nickel_package_map_add_dependency(nickel_package_map *m,
                                                const char *parent,
                                                const char *name,
                                                const char *dir,
                                                nickel_error *out_error);

/**
 * Replace the contents of the map with the packages of a manifest (`Nickel-pkg.ncl`), like the
 * `nickel` command does before evaluating a package.
 *
 * This resolves versions, fetches git and index dependencies that aren't cached yet, and
 * writes or updates `Nickel-pkg.lock` next to the manifest. Fetched packages are cached in
 * `cache_dir` if it isn't NULL (like `--package-cache-dir`), and otherwise in the platform's
 * cache directory, such as `~/.cache/nickel`. Nickel's package manager is experimental: its
 * manifest and lock formats may change.
 *
 * - `manifest` is the null-terminated, UTF-8 path of the manifest; see
 *   [`nickel_package_map_find_manifest`].
 * - On failure, the map is unchanged and the error, of kind `NICKEL_ERROR_KIND_PACKAGE`, is
 *   stored in `out_error` (if not NULL). Errors in the manifest's own code have diagnostics
 *   pointing into it.
 *
 * Libraries built without the `packages` Cargo feature (on by default) always fail.
 */
nickel_result nickel_package_map_load_manifest(nickel_package_map *m,
                                               const char *manifest,
                                               const char *cache_dir,
                                               nickel_error *out_error);

/**
 * Find the manifest for the directory `dir`: the nearest `Nickel-pkg.ncl` in it or a parent
 * directory, as the `nickel` command looks for one.
 *
 * Returns 1 and writes the manifest's path to `out_path` (if not NULL, allocated with
 * [`nickel_string_alloc`]) if there is one, and returns 0 otherwise, or if `dir` isn't UTF-8.
 */
int nickel_package_map_find_manifest(const char *dir, nickel_string *out_path);

/**
 * The number of the program's own packages (those added with
 * [`nickel_package_map_add_package`], or a manifest's direct dependencies).
 */
uintptr_t nickel_package_map_package_count(const nickel_package_map *m);

/**
 * Write the name and directory of the program's package number `index`, in name order, to
 * `out_name` and `out_dir` (each NULL or allocated with [`nickel_string_alloc`]). Returns 1,
 * or 0 if `index` is out of range.
 */
int nickel_package_map_package(const nickel_package_map *m,
                               uintptr_t index,
                               nickel_string *out_name,
                               nickel_string *out_dir);

/**
 * Write a description of the whole map, with every package's dependencies, to `out_string`
 * (allocated with [`nickel_string_alloc`]), as `nickel package debug-resolution` prints it.
 */
void nickel_package_map_to_string(const nickel_package_map *m, nickel_string *out_string);

/**
 * Resolve package imports in this context with a copy of `map`, replacing any map set before.
 * Without one, package imports fail.
 */
void nickel_context_set_package_map(nickel_context *ctx, const nickel_package_map *map);

/**
 * Resolve the program's package imports with a copy of `map`, replacing any map set before.
 * Without one, package imports fail.
 */
void nickel_program_builder_set_package_map(nickel_program_builder *b,
                                            const nickel_package_map *map);

/**
 * Allocate a new, empty [`nickel_program_builder`]. Free it with
 * [`nickel_program_builder_free`].
 */
nickel_program_builder *nickel_program_builder_alloc(void);

/**
 * Free a [`nickel_program_builder`].
 */
void nickel_program_builder_free(nickel_program_builder *b);

/**
 * Add in-memory source code. With several sources, the program is their merge, like
 * `nickel eval a.ncl b.ncl`.
 *
 * - `src` is a null-terminated, UTF-8 string. It is copied.
 * - `name`, if not NULL, names the source in error messages.
 * - `format` is the source's format; `NICKEL_INPUT_FORMAT_AUTO` means Nickel.
 *
 * Relative imports in the source resolve against the current directory, then the import
 * paths. Fails only if `src` or `name` isn't UTF-8.
 */
nickel_result nickel_program_builder_add_source(nickel_program_builder *b,
                                                const char *src,
                                                const char *name,
                                                nickel_input_format format,
                                                nickel_error *out_error);

/**
 * Add a file, read when the program is built. With several sources, the program is their
 * merge.
 *
 * - `path` is a null-terminated, UTF-8 path.
 * - `format` is the file's format; `NICKEL_INPUT_FORMAT_AUTO` infers it from the extension.
 *
 * Relative imports in the file resolve against its directory, then the import paths. Fails
 * only if `path` isn't UTF-8; a missing file is reported by [`nickel_program_builder_build`].
 */
nickel_result nickel_program_builder_add_file(nickel_program_builder *b,
                                              const char *path,
                                              nickel_input_format format,
                                              nickel_error *out_error);

/**
 * Add a directory to search for imports that aren't found relative to the importing file.
 * `path` is a null-terminated, UTF-8 path. Fails only if it isn't UTF-8.
 */
nickel_result nickel_program_builder_add_import_path(nickel_program_builder *b,
                                                     const char *path,
                                                     nickel_error *out_error);

/**
 * Add the directories in the `NICKEL_IMPORT_PATH` environment variable as import paths, as
 * the `nickel` command does. Embedded programs ignore that variable unless you call this.
 */
void nickel_program_builder_add_import_paths_from_env(nickel_program_builder *b);

/**
 * Apply the contract in a Nickel file to the whole program, like `--apply-contract`.
 * `path` is a null-terminated, UTF-8 path. Fails only if it isn't UTF-8.
 */
nickel_result nickel_program_builder_apply_contract_file(nickel_program_builder *b,
                                                         const char *path,
                                                         nickel_error *out_error);

/**
 * Override a field with a value written in Nickel, like `--override path=value`.
 *
 * - `path` is a field path such as `server.port`; segments that aren't plain identifiers go
 *   in double quotes (`labels."app.kubernetes.io/name"`).
 * - `value` is Nickel code, evaluated on its own, so it can't refer to other fields.
 * - `priority` is the override's merge priority, and `numeral` the number for
 *   `NICKEL_MERGE_PRIORITY_NUMERAL` (ignored otherwise). The command line uses
 *   `NICKEL_MERGE_PRIORITY_NEUTRAL`, which replaces `default` values but conflicts with other
 *   plain definitions; `NICKEL_MERGE_PRIORITY_FORCE` replaces a field however it was defined.
 *
 * Fails if `path` doesn't parse or a string isn't UTF-8.
 */
nickel_result nickel_program_builder_add_override(nickel_program_builder *b,
                                                  const char *path,
                                                  const char *value,
                                                  nickel_merge_priority priority,
                                                  int64_t numeral,
                                                  nickel_error *out_error);

/**
 * Evaluate, export or query only this field, like `--field`. `path` is a field path, as in
 * [`nickel_program_builder_add_override`]. Fails if it doesn't parse.
 */
nickel_result nickel_program_builder_set_field(nickel_program_builder *b,
                                               const char *path,
                                               nickel_error *out_error);

/**
 * Send the output of `std.trace` to `write` (and flush with `flush`, which may be NULL),
 * as [`nickel_context_set_trace_callback`] does for a context. It is discarded
 * otherwise.
 */
void nickel_program_builder_set_trace_callback(nickel_program_builder *b,
                                               nickel_write_callback write,
                                               nickel_flush_callback flush,
                                               void *user_data);

/**
 * Call `callback` with each warning Nickel reports while evaluating the program. Warnings are
 * discarded otherwise. See [`nickel_warning_callback`].
 */
void nickel_program_builder_set_warning_callback(nickel_program_builder *b,
                                                 nickel_warning_callback callback,
                                                 void *user_data);

/**
 * Read the sources and prepare the program, storing it in `out_program` (allocated with
 * [`nickel_program_alloc`], replacing any program it held).
 *
 * The builder's configuration is used up: afterwards it is empty, as if newly allocated, and
 * still needs [`nickel_program_builder_free`].
 *
 * Fails if there are no sources or one can't be read. Parse and type errors are reported by
 * the operations on the program.
 */
nickel_result nickel_program_builder_build(nickel_program_builder *b,
                                           nickel_program *out_program,
                                           nickel_error *out_error);

/**
 * Allocate a [`nickel_program`] to build into. Free it with [`nickel_program_free`].
 */
nickel_program *nickel_program_alloc(void);

/**
 * Free a [`nickel_program`].
 */
void nickel_program_free(nickel_program *p);

/**
 * Evaluate the program fully, like `nickel eval`, storing the result in `out_expr` (if not
 * NULL). On failure, stores the error in `out_error` (if not NULL). All the program functions
 * fail if the program hasn't been built.
 */
nickel_result nickel_program_eval_deep(nickel_program *p,
                                       nickel_expr *out_expr,
                                       nickel_error *out_error);

/**
 * Like [`nickel_program_eval_deep`], leaving out fields marked `not_exported`.
 */
nickel_result nickel_program_eval_deep_for_export(nickel_program *p,
                                                  nickel_expr *out_expr,
                                                  nickel_error *out_error);

/**
 * Evaluate the program to weak head normal form, like
 * [`nickel_context_eval_shallow`].
 */
nickel_result nickel_program_eval_shallow(nickel_program *p,
                                          nickel_expr *out_expr,
                                          nickel_error *out_error);

/**
 * Evaluate the records of the program but not the other values in it. Fields that can't be
 * evaluated because something they need is missing are left unevaluated rather than failing,
 * so this works on incomplete configurations.
 */
nickel_result nickel_program_eval_record_spine(nickel_program *p,
                                               nickel_expr *out_expr,
                                               nickel_error *out_error);

/**
 * Evaluate an expression from this program a step further, like
 * [`nickel_context_eval_expr_shallow`]. `out_expr` may be `expr`.
 */
nickel_result nickel_program_eval_expr_shallow(nickel_program *p,
                                               const nickel_expr *expr,
                                               nickel_expr *out_expr,
                                               nickel_error *out_error);

/**
 * Evaluate the program and serialize it, like `nickel export`, storing the text in
 * `out_string` (allocated with [`nickel_string_alloc`]). Fields marked `not_exported`
 * are left out.
 */
nickel_result nickel_program_export(nickel_program *p,
                                    nickel_export_format format,
                                    nickel_string *out_string,
                                    nickel_error *out_error);

/**
 * Look up the focused field (see [`nickel_program_builder_set_field`]), or the whole program,
 * and store its metadata, value and sub-fields in `out_info` (allocated with
 * [`nickel_field_info_alloc`]) without evaluating it fully, like `nickel query`.
 */
nickel_result nickel_program_query(nickel_program *p,
                                   nickel_field_info *out_info,
                                   nickel_error *out_error);

/**
 * Typecheck the program and any applied contracts, like `nickel typecheck`.
 */
nickel_result nickel_program_typecheck(nickel_program *p,
                                       nickel_typecheck_mode mode,
                                       nickel_error *out_error);

/**
 * Parse the program without typechecking or evaluating it. Undefined variables and type
 * errors are not reported.
 */
nickel_result nickel_program_parse(nickel_program *p, nickel_error *out_error);

/**
 * Allocate a [`nickel_field_info`] for [`nickel_program_query`] to fill. Free it with
 * [`nickel_field_info_free`].
 */
nickel_field_info *nickel_field_info_alloc(void);

/**
 * Free a [`nickel_field_info`].
 */
void nickel_field_info_free(nickel_field_info *info);

/**
 * Copy the queried field's metadata into `out_meta` (allocated with
 * [`nickel_field_metadata_alloc`]). Returns 0, leaving `out_meta` alone, if nothing has been
 * queried into `info`.
 */
int nickel_field_info_metadata(const nickel_field_info *info, nickel_field_metadata *out_meta);

/**
 * Store the queried field's value in `out_expr`, if it has one. It may be unevaluated.
 * Returns 1 if the field has a value, 0 if not.
 */
int nickel_field_info_value(const nickel_field_info *info, nickel_expr *out_expr);

/**
 * The number of sub-fields of the queried field, if its value is a record.
 */
uintptr_t nickel_field_info_subfield_count(const nickel_field_info *info);

/**
 * The name of the sub-field at `idx`, in source order: written as a pointer to UTF-8 (not
 * null-terminated) and a length, valid until `info` is overwritten or freed. Returns 1, or 0
 * if `idx` is out of range.
 */
int nickel_field_info_subfield(const nickel_field_info *info,
                               uintptr_t idx,
                               const char **out_str,
                               uintptr_t *out_len);

/**
 * Allocate a [`nickel_field_metadata`]. Free it with [`nickel_field_metadata_free`].
 */
nickel_field_metadata *nickel_field_metadata_alloc(void);

/**
 * Free a [`nickel_field_metadata`].
 */
void nickel_field_metadata_free(nickel_field_metadata *meta);

/**
 * Copy the metadata of the field named `key` into `out_meta`. Returns 1 if the record has the
 * field, 0 (leaving `out_meta` alone) if not.
 *
 * Metadata is only kept on records from shallow evaluation, [`nickel_program_query`] and
 * [`nickel_program_eval_record_spine`]; deep evaluation drops it.
 */
int nickel_record_field_metadata_by_name(const nickel_record *rec,
                                         const char *key,
                                         nickel_field_metadata *out_meta);

/**
 * Copy the metadata of the field at `idx` (in the order of
 * [`nickel_record_key_value_by_index`]) into `out_meta`. Returns 1, or 0 if `idx` is
 * out of range.
 */
int nickel_record_field_metadata_by_index(const nickel_record *rec,
                                          uintptr_t idx,
                                          nickel_field_metadata *out_meta);

/**
 * The field's documentation, as a pointer to UTF-8 (not null-terminated) and a length, valid
 * until `meta` is overwritten or freed. Returns 1, or 0 if the field has no documentation.
 */
int nickel_field_metadata_doc(const nickel_field_metadata *meta,
                              const char **out_str,
                              uintptr_t *out_len);

/**
 * The field's type annotation, rendered as Nickel, like
 * [`nickel_field_metadata_doc`]. Returns 0 if the field has none.
 */
int nickel_field_metadata_type_annotation(const nickel_field_metadata *meta,
                                          const char **out_str,
                                          uintptr_t *out_len);

/**
 * The number of contracts annotating the field.
 */
uintptr_t nickel_field_metadata_contract_count(const nickel_field_metadata *meta);

/**
 * The contract at `idx`, rendered as Nickel, like [`nickel_field_metadata_doc`]. Returns 0 if
 * `idx` is out of range.
 */
int nickel_field_metadata_contract(const nickel_field_metadata *meta,
                                   uintptr_t idx,
                                   const char **out_str,
                                   uintptr_t *out_len);

/**
 * Is the field marked `optional`?
 */
int nickel_field_metadata_is_optional(const nickel_field_metadata *meta);

/**
 * Is the field marked `not_exported`?
 */
int nickel_field_metadata_is_not_exported(const nickel_field_metadata *meta);

/**
 * The field's merge priority. For `NICKEL_MERGE_PRIORITY_NUMERAL`, the number is written to
 * `out_value` (if not NULL), rounded to the nearest `double`.
 */
nickel_merge_priority nickel_field_metadata_priority(const nickel_field_metadata *meta,
                                                     double *out_value);

/**
 * Format Nickel source, like `nickel format`, storing the result in `out_string`.
 *
 * `src` is a null-terminated, UTF-8 string. Fails if it doesn't parse, or if this library was
 * built without the formatter (the `format` Cargo feature).
 */
nickel_result nickel_format(const char *src, nickel_string *out_string, nickel_error *out_error);

/**
 * Extract the documentation of the program's fields, like `nickel doc`, storing it in
 * `out_string`. Fails if the program isn't a record, or if this library was built without
 * documentation support (the `doc` Cargo feature, on by default).
 */
nickel_result nickel_program_docs(nickel_program *p,
                                  nickel_doc_format format,
                                  nickel_string *out_string,
                                  nickel_error *out_error);

/**
 * The version of this library, as a null-terminated string such as `"0.1.0"`. Its presence
 * tells a program it was linked against this superset of Nickel's C API.
 */
const char *nickel_embed_version(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* NICKEL_LANG_H */
