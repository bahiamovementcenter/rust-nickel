//! Phase 3: error kinds, structured diagnostics, warnings, formatting and docs.

use std::{cell::RefCell, rc::Rc};

use nickel_embed::{Context, ErrorKind, Program, Severity, Source, TypecheckMode};

fn eval_err(src: &str) -> nickel_embed::Error {
    Context::new()
        .with_source_name("main.ncl".into())
        .eval_deep(src)
        .unwrap_err()
}

#[test]
fn error_kinds() {
    assert_eq!(eval_err("{ a = ").kind(), ErrorKind::Parse);
    assert_eq!(
        eval_err("{ a | Number = \"x\" }").kind(),
        ErrorKind::Contract
    );
    assert_eq!(eval_err("{ a = 1 }.b").kind(), ErrorKind::Eval);
    assert_eq!(eval_err("import \"nope.ncl\"").kind(), ErrorKind::Import);
    assert_eq!(
        eval_err("let x : Number = \"s\" in x").kind(),
        ErrorKind::Typecheck
    );

    let mut ctx = Context::new();
    let expr = ctx.eval_deep("'Tag 1").unwrap();
    assert_eq!(
        ctx.expr_to_toml(&expr).unwrap_err().kind(),
        ErrorKind::Export
    );

    let typecheck = Program::builder()
        .source(Source::code("fun x => x + \"a\""))
        .build()
        .unwrap()
        .typecheck(TypecheckMode::Enforce)
        .unwrap_err();
    assert_eq!(typecheck.kind(), ErrorKind::Typecheck);

    let missing = Program::builder()
        .source(Source::file("does-not-exist.ncl"))
        .build()
        .err()
        .unwrap();
    assert_eq!(missing.kind(), ErrorKind::Io);
}

#[test]
fn diagnostics_point_at_the_source() {
    let err = eval_err("{\n  port | Number = \"80\",\n}");
    let diags = err.diagnostics();
    assert!(!diags.is_empty());
    let first = &diags[0];
    assert_eq!(first.severity, Severity::Error);
    assert!(first.message.contains("contract broken"), "{first:?}");

    let primary = first
        .labels
        .iter()
        .find(|l| l.primary && l.source == "main.ncl")
        .unwrap_or_else(|| panic!("no primary label in main.ncl: {first:?}"));
    assert_eq!(primary.start.line, 2, "{primary:?}");
    assert!(primary.start.column > 1);
    assert_eq!(
        &"{\n  port | Number = \"80\",\n}"[primary.range.clone()],
        "\"80\""
    );
}

#[test]
fn diagnostics_serialize() {
    let err = eval_err("{ a = 1 + \"x\" }");
    let json = serde_json::to_value(err.diagnostics()).unwrap();
    assert_eq!(json[0]["severity"], "error");
    assert!(json[0]["labels"].is_array());
}

#[test]
fn several_parse_errors() {
    let diags = eval_err("{ a = , b = }").diagnostics();
    assert!(!diags.is_empty());
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
}

fn collect_warnings() -> (
    Rc<RefCell<Vec<nickel_embed::Diagnostic>>>,
    impl FnMut(nickel_embed::Diagnostic),
) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    (seen, move |d| sink.borrow_mut().push(d))
}

// Applying a plain function as a contract still works, but Nickel warns about it.
const NAKED_CONTRACT: &str = "1 | (fun label value => value)";

#[test]
fn warnings_from_context() {
    let (seen, on_warning) = collect_warnings();
    let mut ctx = Context::new().with_warnings(on_warning);
    assert_eq!(ctx.eval_deep(NAKED_CONTRACT).unwrap().as_i64(), Some(1));
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].severity, Severity::Warning);
}

#[test]
fn warnings_from_program() {
    let (seen, on_warning) = collect_warnings();
    Program::builder()
        .source(Source::code(NAKED_CONTRACT))
        .on_warning(on_warning)
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    assert_eq!(seen.borrow().len(), 1);
}

#[test]
fn no_warnings_without_a_callback() {
    // Just checks this doesn't fail or print.
    Context::new().eval_deep(NAKED_CONTRACT).unwrap();
}

#[cfg(feature = "format")]
#[test]
fn format() {
    assert_eq!(
        nickel_embed::format("{a=1,b=   2}").unwrap(),
        "{ a = 1, b = 2 }\n"
    );
    assert!(nickel_embed::format("{ a = ").is_err());
}

#[cfg(feature = "doc")]
#[test]
fn docs() {
    let docs = Program::builder()
        .source(Source::code(
            "{ port | Number | doc \"The port\" = 80, server = { name | doc \"Its name\" = \"x\" }, plain = 1 }",
        ))
        .build()
        .unwrap()
        .docs()
        .unwrap();

    let mut entries: Vec<_> = docs
        .entries()
        .iter()
        .map(|(path, doc)| (path.to_string(), doc.as_str()))
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        [
            ("port".to_string(), "The port"),
            ("server.name".to_string(), "Its name")
        ]
    );

    let json: serde_json::Value = serde_json::from_str(docs.to_json()).unwrap();
    assert!(json.to_string().contains("The port"), "{json}");
    assert!(docs.to_markdown().contains("The port"));
}
