//! The go-nickel test suite (github.com/nickel-lang/go-nickel, nickel_test.go), ported so the
//! Rust API is checked against the same behaviour.

use std::{cell::RefCell, io::Write, rc::Rc};

use nickel_embed::{Context, ErrorFormat, ExprKind};

#[test]
fn record() {
    let expr = Context::new().eval_deep("{ foo = 1, bar = 2 }").unwrap();
    assert!(expr.is_record());
    assert!(expr.is_value());

    let record = expr.as_record().unwrap();
    assert_eq!(record.len(), 2);
    assert!(record.value_by_name("foo").is_some());
    assert!(record.value_by_name("bar").is_some());
    assert!(record.value_by_name("baz").is_none());
}

#[test]
fn record_keeps_source_order() {
    let expr = Context::new().eval_deep("{ b = 1, a = 2, c = 3 }").unwrap();
    let record = expr.as_record().unwrap();
    let keys: Vec<_> = record.iter().map(|(k, _)| k).collect();
    // go-nickel returns a Go map, which loses this order.
    assert_eq!(keys.len(), 3);
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(sorted, ["a", "b", "c"]);
}

#[test]
fn array() {
    let expr = Context::new()
        .eval_deep("[{ foo = 1, bar = 2 }, 1, 2]")
        .unwrap();
    let arr = expr.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert!(arr.get(0).unwrap().is_record());
    assert_eq!(arr.get(1).unwrap().as_i64(), Some(1));
    assert_eq!(arr.get(2).unwrap().as_i64(), Some(2));
    assert!(arr.get(3).is_none());

    let ints: Vec<_> = arr.iter().skip(1).map(|e| e.as_i64().unwrap()).collect();
    assert_eq!(ints, [1, 2]);
}

#[test]
fn lazy_inspection() {
    let mut ctx = Context::new();
    let expr = ctx
        .eval_shallow("{ foo = [1, 2 + 3], bar = \"hi\", baz = 'Tag (1 + 1) }")
        .unwrap();
    let record = expr.as_record().unwrap();
    assert_eq!(record.len(), 3);

    let foo = record.value_by_name("foo").unwrap();
    assert!(!foo.is_value());
    assert!(matches!(foo.kind(), ExprKind::Unevaluated));

    let foo = ctx.eval_expr_shallow(foo).unwrap();
    let arr = foo.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr.get(0).unwrap().as_i64(), Some(1));

    let second = arr.get(1).unwrap();
    assert_eq!(second.as_i64(), None, "2 + 3 should still be lazy");
    assert_eq!(ctx.eval_expr_shallow(second).unwrap().as_i64(), Some(5));
}

#[test]
fn enum_variant() {
    let expr = Context::new().eval_deep("'Tag (2 + 3)").unwrap();
    let (tag, payload) = expr.as_enum_variant().unwrap();
    assert_eq!(tag, "Tag");
    assert_eq!(payload.as_i64(), Some(5));

    match expr.kind() {
        ExprKind::EnumVariant(tag, payload) => {
            assert_eq!(tag, "Tag");
            assert_eq!(payload.as_i64(), Some(5));
        }
        _ => panic!("expected an enum variant"),
    }
}

#[test]
fn eval_error() {
    let err = Context::new()
        .eval_deep("{ foo | String = 1, bar = 2 }")
        .unwrap_err();
    assert!(
        err.to_string().contains("broken by the value of `foo`"),
        "unexpected error: {err}"
    );
}

#[derive(serde::Deserialize, Debug, PartialEq)]
struct FooBar {
    foo: i64,
    bar: i64,
}

#[test]
fn json_and_serde() {
    let ctx = &mut Context::new();
    let expr = ctx.eval_deep("{ foo | Number = 1, bar = 2 }").unwrap();

    let json = ctx.expr_to_json(&expr).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, serde_json::json!({ "foo": 1, "bar": 2 }));

    let target: FooBar = expr.to_serde().unwrap();
    assert_eq!(target, FooBar { foo: 1, bar: 2 });
}

/// A `Write` that the test can read back after handing it to the context.
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
    let mut ctx = Context::new().with_trace(buf.clone());
    ctx.eval_deep("std.trace \"hi\" { bye = std.trace \"bye\" 1 }")
        .unwrap();
    assert_eq!(
        String::from_utf8(buf.0.borrow().clone()).unwrap(),
        "std.trace: hi\nstd.trace: bye\n"
    );
}

#[test]
fn error_format() {
    let err = Context::new()
        .eval_deep("{ port | Number = \"80\", name = \"myserver\" }")
        .unwrap_err();
    let text = err.to_string_with(ErrorFormat::Text);
    assert!(
        text.contains("contract broken by the value of `port`"),
        "{text}"
    );
    assert!(
        !text.contains('\x1b'),
        "plain text should have no ANSI codes"
    );

    let json: serde_json::Value =
        serde_json::from_str(&err.to_string_with(ErrorFormat::Json)).unwrap();
    assert!(json.is_object() || json.is_array());
}

#[test]
fn functions_are_other() {
    let expr = Context::new().eval_deep("fun x => x").unwrap();
    assert!(matches!(expr.kind(), ExprKind::Other));
}

#[test]
fn error_is_std_error() {
    fn takes_std_error(_: &dyn std::error::Error) {}
    let err = Context::new().eval_deep("1 + \"a\"").unwrap_err();
    takes_std_error(&err);
}
