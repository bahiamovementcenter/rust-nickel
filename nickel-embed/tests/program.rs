//! Phase 2: programs built from files and other sources, with import paths, overrides,
//! contracts, field focus, export formats, query and typecheck.

use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc};

use nickel_embed::{
    ExportFormat, FieldPath, InputFormat, Priority, Program, Source, TypecheckMode,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn path(s: &str) -> FieldPath {
    s.parse().unwrap()
}

fn server() -> nickel_embed::ProgramBuilder {
    Program::builder().source(Source::file(fixture("server.ncl")))
}

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Server {
    name: String,
    port: u16,
    greeting: String,
}

#[test]
fn eval_file_with_relative_import() {
    let server: Server = server()
        .build()
        .unwrap()
        .eval_deep()
        .unwrap()
        .to_serde()
        .unwrap();
    assert_eq!(
        server,
        Server {
            name: "myserver".into(),
            port: 80,
            greeting: "hello, myserver".into()
        }
    );
}

#[test]
fn import_paths() {
    let err = Program::builder()
        .source(Source::file(fixture("uses_import_path.ncl")))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap_err();
    assert!(err.to_string().contains("shared.ncl"), "{err}");

    let expr = Program::builder()
        .source(Source::file(fixture("uses_import_path.ncl")))
        .import_path(fixture("vendor"))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let region = expr.as_record().unwrap().value_by_name("region").unwrap();
    assert_eq!(region.as_str(), Some("us-east-1"));
}

#[test]
fn merge_several_inputs_of_different_formats() {
    let expr = server()
        .source(Source::file(fixture("extra.json")))
        .source(Source::code("{ tier = \"gold\" }"))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let record = expr.as_record().unwrap();
    assert_eq!(record.value_by_name("replicas").unwrap().as_i64(), Some(3));
    assert_eq!(record.value_by_name("tier").unwrap().as_str(), Some("gold"));
    assert_eq!(
        record.value_by_name("name").unwrap().as_str(),
        Some("myserver")
    );
}

#[test]
fn code_in_other_formats() {
    let expr = Program::builder()
        .source(Source::code("a: 1\nb: [x, y]\n").with_format(InputFormat::Yaml))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let b = expr.as_record().unwrap().value_by_name("b").unwrap();
    assert_eq!(b.as_array().unwrap().len(), 2);

    let text = Program::builder()
        .source(Source::reader(std::io::Cursor::new("plain text")).with_format(InputFormat::Text))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    assert_eq!(text.as_str(), Some("plain text"));
}

#[test]
fn overrides() {
    let expr = server()
        .override_value(path("port"), "8080")
        .override_with_priority(path("labels.tier"), "\"api\"", Priority::Force)
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let record = expr.as_record().unwrap();
    assert_eq!(record.value_by_name("port").unwrap().as_i64(), Some(8080));
    let labels = record.value_by_name("labels").unwrap();
    assert_eq!(
        labels
            .as_record()
            .unwrap()
            .value_by_name("tier")
            .unwrap()
            .as_str(),
        Some("api")
    );

    // A neutral override conflicts with a plain definition; force wins.
    let mut conflicting = server()
        .override_value(path("name"), "\"other\"")
        .build()
        .unwrap();
    assert!(conflicting.eval_deep().is_err());

    let forced = server()
        .override_with_priority(path("name"), "\"other\"", Priority::Force)
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let name = forced.as_record().unwrap().value_by_name("name").unwrap();
    assert_eq!(name.as_str(), Some("other"));
}

#[test]
fn apply_contract_file() {
    let mut ok = server()
        .apply_contract_file(fixture("schema.ncl"))
        .build()
        .unwrap();
    assert!(ok.eval_deep().is_ok());

    let err = server()
        .apply_contract_file(fixture("schema.ncl"))
        .override_with_priority(path("port"), "\"eighty\"", Priority::Force)
        .build()
        .unwrap()
        .eval_deep()
        .unwrap_err();
    assert!(err.to_string().contains("port"), "{err}");
}

#[test]
fn field_focus() {
    let expr = server()
        .field(path("labels.app"))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    assert_eq!(expr.as_str(), Some("myserver"));
}

#[test]
fn export_formats() {
    let mut program = server().build().unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&program.export(ExportFormat::Json).unwrap()).unwrap();
    assert_eq!(json["port"], 80);
    assert!(
        json.get("secret").is_none(),
        "not_exported fields are left out"
    );

    let yaml = program.export(ExportFormat::Yaml).unwrap();
    assert!(yaml.contains("port: 80"), "{yaml}");

    let toml = program.export(ExportFormat::Toml).unwrap();
    assert!(toml.contains("port = 80"), "{toml}");

    let mut docs = Program::builder()
        .source(Source::code("[{ a = 1 }, { a = 2 }]"))
        .build()
        .unwrap();
    let out = docs.export(ExportFormat::YamlDocuments).unwrap();
    assert_eq!(out.matches("a:").count(), 2, "{out}");

    let mut text = Program::builder()
        .source(Source::code("\"line\""))
        .build()
        .unwrap();
    let mut buf = Vec::new();
    text.export_to(ExportFormat::Text, &mut buf).unwrap();
    assert_eq!(buf, b"line");

    let mut not_a_string = Program::builder()
        .source(Source::code("{ a = 1 }"))
        .build()
        .unwrap();
    assert!(not_a_string.export(ExportFormat::Text).is_err());
}

#[test]
fn query_metadata() {
    let info = server()
        .field(path("name"))
        .build()
        .unwrap()
        .query()
        .unwrap();
    let meta = info.metadata();
    assert_eq!(meta.doc(), Some("The server's name"));
    assert_eq!(meta.contracts(), vec!["String".to_string()]);
    assert!(!meta.optional());

    let replicas = server()
        .field(path("replicas"))
        .build()
        .unwrap()
        .query()
        .unwrap();
    assert!(replicas.metadata().optional());
    assert!(replicas.value().is_none());

    let root = server().build().unwrap().query().unwrap();
    assert_eq!(
        root.subfields(),
        ["name", "port", "replicas", "secret", "greeting", "labels"]
    );
}

#[test]
fn record_fields_with_metadata() {
    let expr = server().build().unwrap().eval_shallow().unwrap();
    let record = expr.as_record().unwrap();
    assert!(record.contains_key("secret"));
    assert!(record.field("secret").unwrap().metadata().not_exported());
    assert_eq!(
        record.field("port").unwrap().metadata().contracts(),
        ["Number"]
    );
    let keys: Vec<_> = record.keys().collect();
    assert_eq!(keys.len(), 6);
    let undocumented = record
        .fields()
        .filter(|(_, f)| f.metadata().doc().is_none())
        .count();
    assert_eq!(undocumented, 5);
}

#[test]
fn typecheck_modes() {
    let mut typed = Program::builder()
        .source(Source::file(fixture("typed.ncl")))
        .build()
        .unwrap();
    typed.typecheck(TypecheckMode::Walk).unwrap();

    let mut bad = Program::builder()
        .source(Source::code(
            "let f : Number -> Number = fun x => x ++ \"a\" in f",
        ))
        .build()
        .unwrap();
    assert!(bad.typecheck(TypecheckMode::Walk).is_err());

    // Untyped code only fails when the whole program is checked.
    let mut untyped = Program::builder()
        .source(Source::file(fixture("untyped_error.ncl")))
        .build()
        .unwrap();
    untyped.typecheck(TypecheckMode::Walk).unwrap();
    let mut untyped = Program::builder()
        .source(Source::file(fixture("untyped_error.ncl")))
        .build()
        .unwrap();
    assert!(untyped.typecheck(TypecheckMode::Enforce).is_err());
}

#[test]
fn parse_only() {
    let mut ok = Program::builder()
        .source(Source::code("{ a = undefined_variable }"))
        .build()
        .unwrap();
    ok.parse().unwrap();

    let mut bad = Program::builder()
        .source(Source::code("{ a = "))
        .build()
        .unwrap();
    assert!(bad.parse().is_err());
}

#[test]
fn shallow_then_deeper() {
    let mut program = server().build().unwrap();
    let expr = program.eval_shallow().unwrap();
    let greeting = expr.as_record().unwrap().value_by_name("greeting").unwrap();
    assert!(!greeting.is_value());
    let greeting = program.eval_expr_shallow(greeting).unwrap();
    assert_eq!(greeting.as_str(), Some("hello, myserver"));
}

#[test]
fn record_spine_tolerates_missing_fields() {
    let mut program = Program::builder()
        .source(Source::code(
            "{ needed | Number, derived = needed + 1, nested = { a = 1 } }",
        ))
        .build()
        .unwrap();
    assert!(program.eval_deep().is_err());

    let mut program = Program::builder()
        .source(Source::code(
            "{ needed | Number, derived = needed + 1, nested = { a = 1 } }",
        ))
        .build()
        .unwrap();
    let spine = program.eval_record_spine().unwrap();
    let nested = spine.as_record().unwrap().value_by_name("nested").unwrap();
    assert!(nested.is_record());
}

#[test]
fn errors_name_the_source() {
    let err = Program::builder()
        .source(Source::code("{ a = 1 + \"x\" }").with_name("inline.ncl"))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap_err();
    assert!(err.to_string().contains("inline.ncl"), "{err}");
}

#[test]
fn build_errors() {
    assert!(Program::builder().build().is_err(), "no sources");
    let err = Program::builder()
        .source(Source::file(fixture("does-not-exist.ncl")))
        .build()
        .err()
        .expect("missing file");
    assert!(err.to_string().contains("does-not-exist.ncl"), "{err}");
}

#[test]
fn field_paths() {
    let p: FieldPath = "labels.\"app.kubernetes.io/name\"".parse().unwrap();
    assert_eq!(p.segments(), ["labels", "app.kubernetes.io/name"]);
    assert_eq!(p.to_string(), "labels.\"app.kubernetes.io/name\"");
    assert!("".parse::<FieldPath>().is_err());
    assert!("a..b".parse::<FieldPath>().is_err());
    let built: FieldPath = ["a", "b"].into_iter().collect();
    assert_eq!(built, path("a.b"));
}

#[derive(Clone, Default)]
struct SharedBuf(Rc<RefCell<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn trace() {
    let buf = SharedBuf::default();
    Program::builder()
        .source(Source::code("std.trace \"hi\" 1"))
        .trace(buf.clone())
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    assert_eq!(&*buf.0.borrow(), b"std.trace: hi\n");
}

#[test]
fn display_is_nickel() {
    let expr = Program::builder()
        .source(Source::code("{ a = [1, \"x\"] }"))
        .build()
        .unwrap()
        .eval_deep()
        .unwrap();
    let shown = expr.to_string();
    assert!(
        shown.contains("a = [ 1, \"x\" ]") || shown.contains("a = [1, \"x\"]"),
        "{shown}"
    );
}
