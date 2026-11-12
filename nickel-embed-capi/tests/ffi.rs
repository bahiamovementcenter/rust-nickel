//! The C API called from Rust, so `cargo test` covers it on every platform. The C tests in
//! `tests/c/` check the same through the header.

use std::{
    ffi::{CString, c_char},
    path::Path,
    ptr::{null, null_mut},
};

use nickel_lang::*;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn fixture(name: &str) -> CString {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nickel-embed/tests/fixtures")
        .join(name);
    c(path.to_str().unwrap())
}

unsafe fn string(s: *const nickel_string) -> String {
    let mut data: *const c_char = null();
    let mut len = 0;
    unsafe {
        nickel_string_data(s, &mut data, &mut len);
        String::from_utf8(std::slice::from_raw_parts(data as *const u8, len).to_vec()).unwrap()
    }
}

unsafe fn borrowed(data: *const c_char, len: usize) -> String {
    unsafe { String::from_utf8(std::slice::from_raw_parts(data as *const u8, len).to_vec()) }
        .unwrap()
}

#[test]
fn program_with_files_overrides_and_contracts() {
    unsafe {
        let b = nickel_program_builder_alloc();
        let err = nickel_error_alloc();
        let ok = nickel_result::NICKEL_RESULT_OK;
        let auto = nickel_input_format::NICKEL_INPUT_FORMAT_AUTO;
        assert_eq!(
            nickel_program_builder_add_file(b, fixture("server.ncl").as_ptr(), auto, err),
            ok
        );
        assert_eq!(
            nickel_program_builder_add_file(b, fixture("extra.json").as_ptr(), auto, err),
            ok
        );
        assert_eq!(
            nickel_program_builder_add_override(
                b,
                c("port").as_ptr(),
                c("8080").as_ptr(),
                nickel_merge_priority::NICKEL_MERGE_PRIORITY_NEUTRAL,
                0,
                err
            ),
            ok
        );
        assert_eq!(
            nickel_program_builder_apply_contract_file(b, fixture("schema.ncl").as_ptr(), err),
            ok
        );
        let p = nickel_program_alloc();
        assert_eq!(nickel_program_builder_build(b, p, err), ok);

        let out = nickel_string_alloc();
        assert_eq!(
            nickel_program_export(p, nickel_export_format::NICKEL_EXPORT_FORMAT_JSON, out, err),
            ok
        );
        let json = string(out);
        assert!(json.contains("\"port\": 8080"), "{json}");
        assert!(json.contains("\"replicas\": 3"), "{json}");
        assert!(!json.contains("secret"), "{json}");

        nickel_string_free(out);
        nickel_program_free(p);
        nickel_error_free(err);
        nickel_program_builder_free(b);
    }
}

#[test]
fn query_metadata() {
    unsafe {
        let b = nickel_program_builder_alloc();
        nickel_program_builder_add_file(
            b,
            fixture("server.ncl").as_ptr(),
            nickel_input_format::NICKEL_INPUT_FORMAT_AUTO,
            null_mut(),
        );
        nickel_program_builder_set_field(b, c("name").as_ptr(), null_mut());
        let p = nickel_program_alloc();
        assert_eq!(
            nickel_program_builder_build(b, p, null_mut()),
            nickel_result::NICKEL_RESULT_OK
        );
        let info = nickel_field_info_alloc();
        let meta = nickel_field_metadata_alloc();
        assert_eq!(
            nickel_program_query(p, info, null_mut()),
            nickel_result::NICKEL_RESULT_OK
        );
        assert_eq!(nickel_field_info_metadata(info, meta), 1);
        let (mut s, mut len) = (null(), 0);
        assert_eq!(nickel_field_metadata_doc(meta, &mut s, &mut len), 1);
        assert_eq!(borrowed(s, len), "The server's name");
        assert_eq!(nickel_field_metadata_contract_count(meta), 1);
        assert_eq!(
            nickel_field_metadata_priority(meta, null_mut()),
            nickel_merge_priority::NICKEL_MERGE_PRIORITY_NEUTRAL
        );

        nickel_field_metadata_free(meta);
        nickel_field_info_free(info);
        nickel_program_free(p);
        nickel_program_builder_free(b);
    }
}

#[test]
fn error_kinds_and_diagnostics() {
    unsafe {
        let ctx = nickel_context_alloc();
        let err = nickel_error_alloc();
        assert_eq!(
            nickel_error_get_kind(err),
            nickel_error_kind::NICKEL_ERROR_KIND_NONE
        );
        nickel_context_set_source_name(ctx, c("config.ncl").as_ptr());
        let src = c("{ port | String = 1 }");
        assert_eq!(
            nickel_context_eval_deep(ctx, src.as_ptr(), null_mut(), err),
            nickel_result::NICKEL_RESULT_ERR
        );
        assert_eq!(
            nickel_error_get_kind(err),
            nickel_error_kind::NICKEL_ERROR_KIND_CONTRACT
        );
        assert!(nickel_error_diagnostic_count(err) >= 1);
        let d = nickel_error_diagnostic(err, 0);
        assert_eq!(
            nickel_diagnostic_severity(d),
            nickel_severity::NICKEL_SEVERITY_ERROR
        );
        let mut s = null();
        let len = nickel_diagnostic_message(d, &mut s);
        assert!(borrowed(s, len).contains("contract broken"));

        let sources: Vec<String> = (0..nickel_diagnostic_label_count(d))
            .map(|i| {
                let mut s = null();
                let len = nickel_label_source(nickel_diagnostic_label(d, i), &mut s);
                borrowed(s, len)
            })
            .collect();
        assert!(sources.iter().any(|s| s == "config.ncl"), "{sources:?}");
        assert!(nickel_error_diagnostic(err, 999).is_null());

        nickel_error_free(err);
        nickel_context_free(ctx);
    }
}

#[test]
fn misuse_is_an_error_not_a_crash() {
    unsafe {
        let err = nickel_error_alloc();
        let p = nickel_program_alloc();
        assert_eq!(
            nickel_program_eval_deep(p, null_mut(), err),
            nickel_result::NICKEL_RESULT_ERR
        );
        let b = nickel_program_builder_alloc();
        assert_eq!(
            nickel_program_builder_set_field(b, c("a.").as_ptr(), err),
            nickel_result::NICKEL_RESULT_ERR
        );
        let ctx = nickel_context_alloc();
        let bad_utf8 = CString::new(vec![b'"', 0xff, b'"']).unwrap();
        assert_eq!(
            nickel_context_eval_deep(ctx, bad_utf8.as_ptr(), null_mut(), err),
            nickel_result::NICKEL_RESULT_ERR
        );
        assert_eq!(
            nickel_error_get_kind(err),
            nickel_error_kind::NICKEL_ERROR_KIND_IO
        );
        nickel_context_free(ctx);
        nickel_program_builder_free(b);
        nickel_program_free(p);
        nickel_error_free(err);
    }
}
