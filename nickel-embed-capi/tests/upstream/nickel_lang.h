// SPDX-License-Identifier: MIT

#ifndef NICKEL_LANG_H
#define NICKEL_LANG_H

#include <stdint.h>

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
 * A Nickel number.
 *
 * See [`nickel_expr_is_number`] and [`nickel_expr_as_number`].
 */
typedef struct nickel_number nickel_number;

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

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* NICKEL_LANG_H */
