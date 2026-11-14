//! Package maps through the C API, called from Rust.

use std::{
    ffi::{CString, c_char},
    fs,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};

use nickel_lang::*;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn path(p: &Path) -> CString {
    c(p.to_str().unwrap())
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nickel-embed/tests/fixtures/packages")
        .join(name)
}

/// A fresh directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("capi-packages")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

unsafe fn string(s: *const nickel_string) -> String {
    let mut data: *const c_char = null();
    let mut len = 0;
    unsafe {
        nickel_string_data(s, &mut data, &mut len);
        String::from_utf8(std::slice::from_raw_parts(data.cast::<u8>(), len).to_vec()).unwrap()
    }
}

const OK: nickel_result = nickel_result::NICKEL_RESULT_OK;
const ERR: nickel_result = nickel_result::NICKEL_RESULT_ERR;

#[test]
fn context_imports_a_package() {
    unsafe {
        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_package_map_add_package(
                map,
                c("greeting").as_ptr(),
                path(&fixture("greeting")).as_ptr(),
                err
            ),
            OK
        );
        let ctx = nickel_context_alloc();
        nickel_context_set_package_map(ctx, map);
        // The context has its own copy.
        nickel_package_map_free(map);

        let expr = nickel_expr_alloc();
        assert_eq!(
            nickel_context_eval_deep(ctx, c("(import greeting).hello").as_ptr(), expr, err),
            OK
        );
        let mut data: *const c_char = null();
        let len = nickel_expr_as_str(expr, &mut data);
        assert_eq!(std::slice::from_raw_parts(data.cast::<u8>(), len), b"world");

        nickel_expr_free(expr);
        nickel_error_free(err);
        nickel_context_free(ctx);
    }
}

#[test]
fn program_dependencies_are_local_to_each_package() {
    unsafe {
        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        let app = path(&fixture("app"));
        assert_eq!(
            nickel_package_map_add_package(map, c("app").as_ptr(), app.as_ptr(), err),
            OK
        );
        assert_eq!(
            nickel_package_map_add_package(
                map,
                c("util").as_ptr(),
                path(&fixture("util-a")).as_ptr(),
                err
            ),
            OK
        );
        assert_eq!(
            nickel_package_map_add_dependency(
                map,
                app.as_ptr(),
                c("util").as_ptr(),
                path(&fixture("util-b")).as_ptr(),
                err
            ),
            OK
        );

        assert_eq!(nickel_package_map_package_count(map), 2);
        let name = nickel_string_alloc();
        let dir = nickel_string_alloc();
        assert_eq!(nickel_package_map_package(map, 0, name, dir), 1);
        assert_eq!(string(name), "app");
        assert!(string(dir).ends_with("app"), "{}", string(dir));
        assert_eq!(nickel_package_map_package(map, 2, name, dir), 0);
        nickel_package_map_to_string(map, name);
        assert!(string(name).contains("util-b"), "{}", string(name));

        let b = nickel_program_builder_alloc();
        assert_eq!(
            nickel_program_builder_add_source(
                b,
                c("{ outer = (import util).name, inner = (import app).inner }").as_ptr(),
                null(),
                nickel_input_format::NICKEL_INPUT_FORMAT_AUTO,
                err
            ),
            OK
        );
        nickel_program_builder_set_package_map(b, map);
        let p = nickel_program_alloc();
        assert_eq!(nickel_program_builder_build(b, p, err), OK);
        let out = nickel_string_alloc();
        assert_eq!(
            nickel_program_export(p, nickel_export_format::NICKEL_EXPORT_FORMAT_JSON, out, err),
            OK
        );
        let json = string(out);
        assert!(json.contains(r#""outer": "a""#), "{json}");
        assert!(json.contains(r#""inner": "b""#), "{json}");

        for s in [name, dir, out] {
            nickel_string_free(s);
        }
        nickel_program_free(p);
        nickel_program_builder_free(b);
        nickel_package_map_free(map);
        nickel_error_free(err);
    }
}

#[test]
fn missing_package_is_an_import_error() {
    unsafe {
        let ctx = nickel_context_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_context_eval_deep(ctx, c("import nowhere").as_ptr(), null_mut(), err),
            ERR
        );
        assert_eq!(
            nickel_error_get_kind(err),
            nickel_error_kind::NICKEL_ERROR_KIND_IMPORT
        );
        nickel_error_free(err);
        nickel_context_free(ctx);
    }
}

#[test]
fn bad_strings_are_errors() {
    unsafe {
        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        let bad = [0xffu8, 0];
        assert_eq!(
            nickel_package_map_add_package(map, bad.as_ptr().cast(), c("/x").as_ptr(), err),
            ERR
        );
        assert_eq!(
            nickel_package_map_add_package(map, c("x").as_ptr(), null(), err),
            ERR
        );
        assert_eq!(nickel_package_map_package_count(map), 0);
        assert_eq!(nickel_package_map_find_manifest(null(), null_mut()), 0);
        nickel_error_free(err);
        nickel_package_map_free(map);
    }
}

fn manifest(name: &str, dependencies: &str) -> String {
    format!(
        r#"{{
  name = "{name}",
  description = "test package",
  version = "0.1.0",
  authors = ["nickel-embed"],
  minimal_nickel_version = "1.9.0",
  dependencies = {{ {dependencies} }},
}} | std.package.Manifest
"#
    )
}

#[test]
fn find_manifest_searches_parents() {
    unsafe {
        let dir = scratch("find");
        write(&dir.join("Nickel-pkg.ncl"), &manifest("root", ""));
        fs::create_dir_all(dir.join("a/b")).unwrap();
        let out = nickel_string_alloc();
        assert_eq!(
            nickel_package_map_find_manifest(path(&dir.join("a/b")).as_ptr(), out),
            1
        );
        assert_eq!(PathBuf::from(string(out)), dir.join("Nickel-pkg.ncl"),);
        nickel_string_free(out);
    }
}

#[cfg(feature = "packages")]
#[test]
fn manifest_resolves_path_dependencies() {
    unsafe {
        let dir = scratch("manifest");
        write(
            &dir.join("app/Nickel-pkg.ncl"),
            &manifest("app", r#"lib = 'Path "../lib""#),
        );
        write(&dir.join("app/main.ncl"), "(import lib).value");
        write(&dir.join("lib/Nickel-pkg.ncl"), &manifest("lib", ""));
        write(&dir.join("lib/main.ncl"), "{ value = 42 }");

        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_package_map_load_manifest(
                map,
                path(&dir.join("app/Nickel-pkg.ncl")).as_ptr(),
                path(&dir.join("cache")).as_ptr(),
                err
            ),
            OK
        );
        assert!(dir.join("app/Nickel-pkg.lock").is_file());
        assert_eq!(nickel_package_map_package_count(map), 1);

        let b = nickel_program_builder_alloc();
        assert_eq!(
            nickel_program_builder_add_file(
                b,
                path(&dir.join("app/main.ncl")).as_ptr(),
                nickel_input_format::NICKEL_INPUT_FORMAT_AUTO,
                err
            ),
            OK
        );
        nickel_program_builder_set_package_map(b, map);
        let p = nickel_program_alloc();
        assert_eq!(nickel_program_builder_build(b, p, err), OK);
        let out = nickel_string_alloc();
        assert_eq!(
            nickel_program_export(p, nickel_export_format::NICKEL_EXPORT_FORMAT_JSON, out, err),
            OK
        );
        assert_eq!(string(out), "42");

        nickel_string_free(out);
        nickel_program_free(p);
        nickel_program_builder_free(b);
        nickel_package_map_free(map);
        nickel_error_free(err);
    }
}

#[cfg(feature = "packages")]
#[test]
fn bad_manifest_is_a_package_error_and_keeps_the_map() {
    unsafe {
        let dir = scratch("bad-manifest");
        write(
            &dir.join("Nickel-pkg.ncl"),
            &manifest("bad", "").replace("0.1.0", "one"),
        );
        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_package_map_add_package(map, c("kept").as_ptr(), c("/kept").as_ptr(), err),
            OK
        );
        assert_eq!(
            nickel_package_map_load_manifest(
                map,
                path(&dir.join("Nickel-pkg.ncl")).as_ptr(),
                path(&dir.join("cache")).as_ptr(),
                err
            ),
            ERR
        );
        assert_eq!(
            nickel_error_get_kind(err),
            nickel_error_kind::NICKEL_ERROR_KIND_PACKAGE
        );
        assert!(nickel_error_diagnostic_count(err) > 0);
        assert_eq!(nickel_package_map_package_count(map), 1);
        nickel_error_free(err);
        nickel_package_map_free(map);
    }
}

#[cfg(not(feature = "packages"))]
#[test]
fn manifests_need_the_packages_feature() {
    unsafe {
        let map = nickel_package_map_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_package_map_load_manifest(map, c("Nickel-pkg.ncl").as_ptr(), null(), err),
            ERR
        );
        nickel_error_free(err);
        nickel_package_map_free(map);
    }
}
