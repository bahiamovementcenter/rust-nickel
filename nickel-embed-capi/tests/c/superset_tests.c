// Tests for the functions nickel-embed adds to nickel_lang.h.
//
// Build and run with tests/c/run.sh. FIXTURES is the path of nickel-embed/tests/fixtures.

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "nickel_lang.h"

#ifndef FIXTURES
#error "define FIXTURES as the path of nickel-embed/tests/fixtures"
#endif

// Compares a (pointer, length) string from the API with a C string.
static int str_eq(const char *s, uintptr_t len, const char *expected) {
  return len == strlen(expected) && strncmp(s, expected, len) == 0;
}

static void assert_string(const nickel_string *s, const char *expected) {
  const char *data;
  uintptr_t len;
  nickel_string_data(s, &data, &len);
  if (!str_eq(data, len, expected)) {
    fprintf(stderr, "expected %s, got %.*s\n", expected, (int)len, data);
    abort();
  }
}

static int contains(const nickel_string *s, const char *needle) {
  const char *data;
  uintptr_t len;
  nickel_string_data(s, &data, &len);
  char *copy = malloc(len + 1);
  memcpy(copy, data, len);
  copy[len] = '\0';
  int found = strstr(copy, needle) != NULL;
  free(copy);
  return found;
}

static void assert_contains(const nickel_string *s, const char *needle) {
  if (!contains(s, needle)) {
    const char *data;
    uintptr_t len;
    nickel_string_data(s, &data, &len);
    fprintf(stderr, "%s not in:\n%.*s\n", needle, (int)len, data);
    abort();
  }
}

static int64_t field_i64(const nickel_expr *record, const char *key) {
  nickel_expr *value = nickel_expr_alloc();
  assert(nickel_record_value_by_name(nickel_expr_as_record(record), key, value));
  const nickel_number *n = nickel_expr_as_number(value);
  assert(nickel_number_is_i64(n));
  int64_t result = nickel_number_as_i64(n);
  nickel_expr_free(value);
  return result;
}

// Builds a program from the builder, asserting success.
static nickel_program *build(nickel_program_builder *b) {
  nickel_program *p = nickel_program_alloc();
  nickel_error *err = nickel_error_alloc();
  if (nickel_program_builder_build(b, p, err) != NICKEL_RESULT_OK) {
    nickel_string *s = nickel_string_alloc();
    nickel_error_format_as_string(err, s, NICKEL_ERROR_FORMAT_TEXT);
    const char *data;
    uintptr_t len;
    nickel_string_data(s, &data, &len);
    fprintf(stderr, "build failed: %.*s\n", (int)len, data);
    abort();
  }
  nickel_error_free(err);
  return p;
}

static void test_version(void) {
  const char *v = nickel_embed_version();
  assert(v != NULL && strlen(v) > 0);
}

static void test_files_overrides_and_export(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();
  assert(nickel_program_builder_add_file(b, FIXTURES "/server.ncl", NICKEL_INPUT_FORMAT_AUTO, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_add_file(b, FIXTURES "/extra.json", NICKEL_INPUT_FORMAT_AUTO, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_add_override(b, "port", "8080", NICKEL_MERGE_PRIORITY_NEUTRAL, 0, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_apply_contract_file(b, FIXTURES "/schema.ncl", err) == NICKEL_RESULT_OK);
  nickel_program *p = build(b);

  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_program_eval_deep(p, expr, err) == NICKEL_RESULT_OK);
  assert(field_i64(expr, "port") == 8080);
  assert(field_i64(expr, "replicas") == 3);

  nickel_string *out = nickel_string_alloc();
  assert(nickel_program_export(p, NICKEL_EXPORT_FORMAT_JSON, out, err) == NICKEL_RESULT_OK);
  assert_contains(out, "\"greeting\": \"hello, myserver\"");
  // not_exported fields are left out of exports.
  assert(!contains(out, "secret"));

  assert(nickel_program_export(p, NICKEL_EXPORT_FORMAT_YAML, out, err) == NICKEL_RESULT_OK);
  assert_contains(out, "port: 8080");

  // The builder is empty after building: building again fails for lack of sources.
  nickel_program *again = nickel_program_alloc();
  assert(nickel_program_builder_build(b, again, err) == NICKEL_RESULT_ERR);
  nickel_program_free(again);

  nickel_string_free(out);
  nickel_expr_free(expr);
  nickel_program_free(p);
  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_in_memory_sources_and_field(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();
  assert(nickel_program_builder_add_source(b, "{ a = { b = 1 } }", "main.ncl", NICKEL_INPUT_FORMAT_AUTO, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_add_source(b, "a:\n  c: 2\n", NULL, NICKEL_INPUT_FORMAT_YAML, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_add_override(b, "a.b", "5", NICKEL_MERGE_PRIORITY_FORCE, 0, err) == NICKEL_RESULT_OK);
  assert(nickel_program_builder_set_field(b, "a", err) == NICKEL_RESULT_OK);
  nickel_program *p = build(b);

  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_program_eval_deep(p, expr, err) == NICKEL_RESULT_OK);
  assert(field_i64(expr, "b") == 5);
  assert(field_i64(expr, "c") == 2);

  // A bad field path is reported.
  assert(nickel_program_builder_set_field(b, "a.", err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) != NICKEL_ERROR_KIND_NONE);

  nickel_expr_free(expr);
  nickel_program_free(p);
  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_shallow_and_lazy(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();
  nickel_program_builder_add_source(b, "{ x = 1 + 1, y = std.fail_with \"never\" }", NULL, NICKEL_INPUT_FORMAT_AUTO, err);
  nickel_program *p = build(b);

  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_program_eval_shallow(p, expr, err) == NICKEL_RESULT_OK);
  nickel_expr *x = nickel_expr_alloc();
  assert(nickel_record_value_by_name(nickel_expr_as_record(expr), "x", x));
  assert(!nickel_expr_is_value(x));
  assert(nickel_program_eval_expr_shallow(p, x, x, err) == NICKEL_RESULT_OK);
  assert(nickel_number_as_i64(nickel_expr_as_number(x)) == 2);

  nickel_expr_free(x);
  nickel_expr_free(expr);
  nickel_program_free(p);
  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_query_and_metadata(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();
  nickel_program_builder_add_file(b, FIXTURES "/server.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  assert(nickel_program_builder_set_field(b, "name", err) == NICKEL_RESULT_OK);
  nickel_program *p = build(b);

  nickel_field_info *info = nickel_field_info_alloc();
  nickel_field_metadata *meta = nickel_field_metadata_alloc();
  assert(nickel_program_query(p, info, err) == NICKEL_RESULT_OK);
  assert(nickel_field_info_metadata(info, meta));

  const char *s;
  uintptr_t len;
  assert(nickel_field_metadata_doc(meta, &s, &len));
  assert(str_eq(s, len, "The server's name"));
  assert(nickel_field_metadata_contract_count(meta) == 1);
  assert(nickel_field_metadata_contract(meta, 0, &s, &len));
  assert(str_eq(s, len, "String"));
  assert(!nickel_field_metadata_contract(meta, 1, &s, &len));
  assert(!nickel_field_metadata_is_optional(meta));
  assert(nickel_field_info_value(info, NULL));
  nickel_program_free(p);

  // The whole program: its sub-fields, in source order.
  nickel_program_builder_add_file(b, FIXTURES "/server.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  p = build(b);
  assert(nickel_program_query(p, info, err) == NICKEL_RESULT_OK);
  assert(nickel_field_info_subfield_count(info) == 6);
  assert(nickel_field_info_subfield(info, 0, &s, &len) && str_eq(s, len, "name"));
  assert(!nickel_field_info_subfield(info, 6, &s, &len));

  // Metadata on a record from shallow evaluation.
  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_program_eval_shallow(p, expr, err) == NICKEL_RESULT_OK);
  const nickel_record *rec = nickel_expr_as_record(expr);
  assert(nickel_record_field_metadata_by_name(rec, "port", meta));
  double value = -1;
  assert(nickel_field_metadata_priority(meta, &value) == NICKEL_MERGE_PRIORITY_DEFAULT);
  assert(nickel_record_field_metadata_by_name(rec, "replicas", meta));
  assert(nickel_field_metadata_is_optional(meta));
  assert(nickel_record_field_metadata_by_name(rec, "secret", meta));
  assert(nickel_field_metadata_is_not_exported(meta));
  assert(!nickel_record_field_metadata_by_name(rec, "nope", meta));
  assert(nickel_record_field_metadata_by_index(rec, 0, meta));
  assert(nickel_field_metadata_doc(meta, &s, &len));
  assert(!nickel_record_field_metadata_by_index(rec, 99, meta));

  nickel_expr_free(expr);
  nickel_field_metadata_free(meta);
  nickel_field_info_free(info);
  nickel_program_free(p);
  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_typecheck_and_parse(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();

  nickel_program_builder_add_file(b, FIXTURES "/typed.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  nickel_program *p = build(b);
  assert(nickel_program_typecheck(p, NICKEL_TYPECHECK_MODE_WALK, err) == NICKEL_RESULT_OK);
  nickel_program_free(p);

  // Walk mode accepts untyped code that enforce mode rejects. A program remembers that it
  // has been typechecked, so each mode gets its own.
  nickel_program_builder_add_file(b, FIXTURES "/untyped_error.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  p = build(b);
  assert(nickel_program_typecheck(p, NICKEL_TYPECHECK_MODE_WALK, err) == NICKEL_RESULT_OK);
  nickel_program_free(p);
  nickel_program_builder_add_file(b, FIXTURES "/untyped_error.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  p = build(b);
  assert(nickel_program_typecheck(p, NICKEL_TYPECHECK_MODE_ENFORCE, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_TYPECHECK);
  nickel_program_free(p);

  nickel_program_builder_add_source(b, "{ a = }", NULL, NICKEL_INPUT_FORMAT_AUTO, err);
  p = build(b);
  assert(nickel_program_parse(p, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_PARSE);
  nickel_program_free(p);

  // Operations on a program that was never built fail instead of crashing.
  p = nickel_program_alloc();
  assert(nickel_program_eval_deep(p, NULL, err) == NICKEL_RESULT_ERR);
  nickel_program_free(p);

  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_diagnostics(void) {
  nickel_context *ctx = nickel_context_alloc();
  nickel_error *err = nickel_error_alloc();
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_NONE);
  assert(nickel_error_diagnostic_count(err) == 0);

  nickel_context_set_source_name(ctx, "config.ncl");
  assert(nickel_context_eval_deep(ctx, "{ port | String = 1 }", NULL, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_CONTRACT);
  assert(nickel_error_diagnostic_count(err) >= 1);
  assert(nickel_error_diagnostic(err, 999) == NULL);

  const nickel_diagnostic *d = nickel_error_diagnostic(err, 0);
  assert(nickel_diagnostic_severity(d) == NICKEL_SEVERITY_ERROR);
  const char *s;
  uintptr_t len = nickel_diagnostic_message(d, &s);
  assert(len > 0);
  assert(nickel_diagnostic_label_count(d) >= 1);
  assert(nickel_diagnostic_label(d, 999) == NULL);

  // Some label points into config.ncl, with 1-based lines and columns.
  int found = 0;
  for (uintptr_t i = 0; i < nickel_diagnostic_label_count(d); i++) {
    const nickel_label *l = nickel_diagnostic_label(d, i);
    len = nickel_label_source(l, &s);
    if (str_eq(s, len, "config.ncl")) {
      uintptr_t start, end, line, col;
      nickel_label_position(l, &start, &end, &line, &col, NULL, NULL);
      assert(start < end && end <= strlen("{ port | String = 1 }"));
      assert(line == 1 && col >= 1);
      found = 1;
    }
  }
  assert(found);

  // Overwriting the error refreshes its diagnostics.
  assert(nickel_context_eval_deep(ctx, "{ a = ", NULL, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_PARSE);

  nickel_error_free(err);
  nickel_context_free(ctx);
}

struct warnings {
  int count;
  int severity_ok;
};

static void on_warning(void *user_data, const nickel_diagnostic *d) {
  struct warnings *w = user_data;
  w->count++;
  w->severity_ok = nickel_diagnostic_severity(d) == NICKEL_SEVERITY_WARNING;
}

static void test_warnings_and_import_paths(void) {
  // Applying a plain function as a contract works, but Nickel warns about it.
  const char *warns = "1 | (fun label value => value)";

  nickel_context *ctx = nickel_context_alloc();
  struct warnings w = {0, 0};
  nickel_context_set_warning_callback(ctx, on_warning, &w);
  assert(nickel_context_eval_deep(ctx, warns, NULL, NULL) == NICKEL_RESULT_OK);
  assert(w.count == 1 && w.severity_ok);

  nickel_expr *expr = nickel_expr_alloc();
  nickel_context_add_import_path(ctx, FIXTURES "/vendor");
  assert(nickel_context_eval_deep(ctx, "(import \"shared.ncl\").region", expr, NULL) == NICKEL_RESULT_OK);
  const char *s;
  uintptr_t len = nickel_expr_as_str(expr, &s);
  assert(str_eq(s, len, "us-east-1"));
  nickel_expr_free(expr);
  nickel_context_free(ctx);

  // The same through a program.
  nickel_program_builder *b = nickel_program_builder_alloc();
  struct warnings pw = {0, 0};
  nickel_program_builder_set_warning_callback(b, on_warning, &pw);
  nickel_program_builder_add_source(b, warns, NULL, NICKEL_INPUT_FORMAT_AUTO, NULL);
  nickel_program *p = build(b);
  assert(nickel_program_eval_deep(p, NULL, NULL) == NICKEL_RESULT_OK);
  assert(pw.count == 1 && pw.severity_ok);
  nickel_program_free(p);

  nickel_program_builder_add_file(b, FIXTURES "/uses_import_path.ncl", NICKEL_INPUT_FORMAT_AUTO, NULL);
  nickel_program_builder_add_import_path(b, FIXTURES "/vendor", NULL);
  p = build(b);
  nickel_string *out = nickel_string_alloc();
  assert(nickel_program_export(p, NICKEL_EXPORT_FORMAT_TOML, out, NULL) == NICKEL_RESULT_OK);
  assert_string(out, "region = \"us-east-1\"\n");
  nickel_string_free(out);
  nickel_program_free(p);
  nickel_program_builder_free(b);
}

static void test_packages(void) {
  nickel_package_map *map = nickel_package_map_alloc();
  nickel_error *err = nickel_error_alloc();
  assert(nickel_package_map_add_package(map, "app", FIXTURES "/packages/app", err) == NICKEL_RESULT_OK);
  assert(nickel_package_map_add_package(map, "util", FIXTURES "/packages/util-a", err) == NICKEL_RESULT_OK);
  // Inside `app`, `util` is another package.
  assert(nickel_package_map_add_dependency(map, FIXTURES "/packages/app", "util",
                                           FIXTURES "/packages/util-b", err) == NICKEL_RESULT_OK);
  assert(nickel_package_map_package_count(map) == 2);
  nickel_string *name = nickel_string_alloc();
  assert(nickel_package_map_package(map, 1, name, NULL));
  assert_string(name, "util");
  assert(!nickel_package_map_package(map, 2, name, NULL));

  // A context.
  nickel_context *ctx = nickel_context_alloc();
  nickel_context_set_package_map(ctx, map);
  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_context_eval_deep(ctx, "(import app).inner", expr, err) == NICKEL_RESULT_OK);
  const char *s;
  uintptr_t len = nickel_expr_as_str(expr, &s);
  assert(str_eq(s, len, "b"));
  nickel_context_free(ctx);

  // A program.
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_program_builder_add_source(b, "(import util).name", NULL, NICKEL_INPUT_FORMAT_AUTO, NULL);
  nickel_program_builder_set_package_map(b, map);
  nickel_package_map_free(map);  // the builder has its own copy
  nickel_program *p = build(b);
  assert(nickel_program_eval_deep(p, expr, err) == NICKEL_RESULT_OK);
  len = nickel_expr_as_str(expr, &s);
  assert(str_eq(s, len, "a"));
  nickel_program_free(p);
  nickel_program_builder_free(b);

  // Without a map, a package import is an import error.
  ctx = nickel_context_alloc();
  assert(nickel_context_eval_deep(ctx, "import app", NULL, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_IMPORT);
  nickel_context_free(ctx);

  // There's no manifest above the fixtures in the repository.
  assert(!nickel_package_map_find_manifest(FIXTURES "/packages/app", name));

  nickel_string_free(name);
  nickel_expr_free(expr);
  nickel_error_free(err);
}

static void test_docs_and_format(void) {
  nickel_program_builder *b = nickel_program_builder_alloc();
  nickel_error *err = nickel_error_alloc();
  nickel_program_builder_add_file(b, FIXTURES "/server.ncl", NICKEL_INPUT_FORMAT_AUTO, err);
  nickel_program *p = build(b);
  nickel_string *out = nickel_string_alloc();
  // `doc` is a default feature.
  assert(nickel_program_docs(p, NICKEL_DOC_FORMAT_MARKDOWN, out, err) == NICKEL_RESULT_OK);
  assert_contains(out, "The server's name");

  // `format` isn't: either it formats, or it explains why it can't.
  if (nickel_format("{a=1}", out, err) == NICKEL_RESULT_OK) {
    assert_contains(out, "a = 1");
  } else {
    nickel_error_format_as_string(err, out, NICKEL_ERROR_FORMAT_TEXT);
    assert_contains(out, "format");
  }

  nickel_string_free(out);
  nickel_program_free(p);
  nickel_error_free(err);
  nickel_program_builder_free(b);
}

static void test_fresh_expr_is_null(void) {
  nickel_context *ctx = nickel_context_alloc();
  nickel_expr *expr = nickel_expr_alloc();
  assert(nickel_expr_is_null(expr) && nickel_expr_is_value(expr));
  assert(!nickel_expr_is_record(expr));
  nickel_string *out = nickel_string_alloc();
  assert(nickel_context_expr_to_json(ctx, expr, out, NULL) == NICKEL_RESULT_OK);
  assert_string(out, "null");
  nickel_string_free(out);

  // Invalid UTF-8 is an error, not a crash.
  nickel_error *err = nickel_error_alloc();
  assert(nickel_context_eval_deep(ctx, "\"\xff\"", expr, err) == NICKEL_RESULT_ERR);
  assert(nickel_error_get_kind(err) == NICKEL_ERROR_KIND_IO);
  nickel_error_free(err);

  nickel_expr_free(expr);
  nickel_context_free(ctx);
}

int main(void) {
  test_version();
  test_files_overrides_and_export();
  test_in_memory_sources_and_field();
  test_shallow_and_lazy();
  test_query_and_metadata();
  test_typecheck_and_parse();
  test_diagnostics();
  test_warnings_and_import_paths();
  test_packages();
  test_docs_and_format();
  test_fresh_expr_is_null();
  puts("superset tests passed");
  return 0;
}
