//! The bundled xD primitives (feature `xd`): xD's own examples run unchanged, and the typed
//! wrappers agree with them.
#![cfg(feature = "xd")]

use nickel_embed::{
    Context,
    xd::{Entry, Error, Pattern},
};

fn entry(address: &[u64], link: Option<&[u64]>, label: Option<&str>) -> Entry {
    Entry {
        address: address.to_vec(),
        link: link.map(<[u64]>::to_vec),
        label: label.map(str::to_owned),
    }
}

/// xD's `nickel/tests/primitives.ncl`, copied with the library, passes as it does in xD.
#[test]
fn xds_examples_pass() {
    let mut ctx = Context::new().with_xd();
    let result = ctx.eval_deep(include_str!("../xd/tests/primitives.ncl"));
    assert!(result.is_ok(), "{}", result.err().unwrap());
}

#[test]
fn bundled_files_import_without_a_context_of_their_own() {
    let mut ctx = Context::new().with_xd();
    let expr = ctx
        .eval_deep(r#"(import "jd.ncl").scheme.levels |> std.array.map (fun l => l.name)"#)
        .unwrap();
    let names: Vec<String> = expr.to_serde().unwrap();
    assert_eq!(names, ["area", "category", "id"]);
}

#[test]
fn jd_renders_and_parses() {
    let mut jd = Pattern::jd();
    assert_eq!(jd.render(&[1]).unwrap(), "10-19");
    assert_eq!(jd.render(&[1, 1]).unwrap(), "11");
    assert_eq!(jd.render(&[1, 1, 11]).unwrap(), "11.11");
    assert_eq!(jd.name(&[1, 1, 11], "Passport").unwrap(), "11.11 Passport");
    assert_eq!(jd.name(&[1, 1, 11], "").unwrap(), "11.11");
    assert_eq!(
        jd.parse("11.11 Passport").unwrap(),
        Some(entry(&[1, 1, 11], None, Some("Passport")))
    );
    assert_eq!(
        jd.parse("10-19 Life admin").unwrap(),
        Some(entry(&[1], None, Some("Life admin")))
    );
    assert_eq!(jd.parse("11").unwrap(), Some(entry(&[1, 1], None, None)));
    for outside in ["1", "10-18", "11.1", "+ Wendy", ""] {
        assert_eq!(jd.parse(outside).unwrap(), None, "{outside:?}");
    }
}

#[test]
fn write_inverts_parse() {
    let mut jd = Pattern::jd();
    for name in ["11.11 Passport", "10-19 Life admin", "11", "99.99"] {
        let parsed = jd.parse(name).unwrap().unwrap();
        assert_eq!(jd.write(&parsed).unwrap(), name);
    }
}

#[test]
fn labels_are_passed_through_exactly() {
    let mut jd = Pattern::jd();
    for label in [
        r#"quotes " and \ backslash"#,
        "%{not interpolated}",
        "tab\there",
        "ünïcødé 😭",
    ] {
        let name = jd.name(&[1, 1, 11], label).unwrap();
        assert_eq!(name, format!("11.11 {label}"));
        assert_eq!(
            jd.parse(&name).unwrap(),
            Some(entry(&[1, 1, 11], None, Some(label)))
        );
    }
}

#[test]
fn work_packages_stand_alone_and_stack() {
    let mut alone = Pattern::work_packages();
    assert_eq!(
        alone.parse("W0012 Taxes").unwrap(),
        Some(entry(&[12], None, Some("Taxes")))
    );
    assert_eq!(alone.parse("W0012~11.13").unwrap(), None);

    let mut on_jd =
        Pattern::new(r#"((import "jd.ncl") & (import "work-packages.ncl")).work_packages"#);
    let linked = entry(&[12], Some(&[1, 1, 13]), Some("2026 tax return"));
    assert_eq!(
        on_jd.parse("W0012~11.13 2026 tax return").unwrap(),
        Some(linked.clone())
    );
    assert_eq!(on_jd.write(&linked).unwrap(), "W0012~11.13 2026 tax return");
}

#[test]
fn custom_patterns_see_the_primitives() {
    let mut boards = Pattern::new(
        r#"xd.pattern [
             xd.slot { name = "category", width = 2 },
             xd.sep.within,
             xd.slot { name = "board", width = 2, alphabet = xd.alphabets.hex },
           ]"#,
    );
    assert_eq!(boards.render(&[11, 255]).unwrap(), "11.FF");
    assert_eq!(
        boards.parse("11.ff").unwrap(),
        Some(entry(&[11, 255], None, None))
    );
}

#[test]
fn the_scheme_deserializes() {
    #[derive(serde::Deserialize)]
    struct Level {
        name: String,
        width: u32,
        separator: String,
        range: bool,
    }
    #[derive(serde::Deserialize)]
    struct Scheme {
        base: u32,
        levels: Vec<Level>,
    }
    let scheme: Scheme = Pattern::jd().scheme().unwrap();
    assert_eq!(scheme.base, 10);
    let levels: Vec<_> = scheme
        .levels
        .iter()
        .map(|l| (l.name.as_str(), l.width, l.separator.as_str(), l.range))
        .collect();
    assert_eq!(
        levels,
        [
            ("area", 1, "", true),
            ("category", 1, "", false),
            ("id", 2, ".", false)
        ]
    );
}

#[test]
fn mistakes_are_nickel_errors() {
    let mut jd = Pattern::jd();
    assert!(matches!(jd.render(&[1, 1, 100]), Err(Error::Nickel(_))));
    assert!(matches!(jd.render(&[]), Err(Error::Nickel(_))));

    let mut bad = Pattern::new(r#"xd.pattern [xd.sep.within]"#);
    match bad.render(&[1]) {
        Err(Error::Nickel(e)) => assert!(e.to_string().contains("at least one slot"), "{e}"),
        other => panic!("expected a Nickel error, got {:?}", other.map(|_| ())),
    }

    let mut wp =
        Pattern::new(r#"((import "jd.ncl") & (import "work-packages.ncl")).work_packages"#);
    assert!(matches!(
        wp.scheme::<serde_json::Value>(),
        Err(Error::Nickel(_))
    ));
}
