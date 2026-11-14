//! xD's package (feature `xd`): fetched at the pinned commit into a temporary cache, then
//! the typed wrappers checked against it.
//!
//! The package is fetched from GitHub. To fetch it from a local clone of xD instead (offline,
//! or behind a proxy gix can't use), set `NICKEL_EMBED_XD_REPO` to its path or URL; it must
//! contain the pinned commit.
#![cfg(feature = "xd")]

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use nickel_embed::{
    Context, ErrorKind, PackageMap,
    xd::{self, Entry, Error, Pattern},
};

/// One fetch for the whole test binary, into a cache that lives as long as it does.
fn cache_dir() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let _ = fetch(dir.path().to_owned()).unwrap();
            dir
        })
        .path()
}

fn fetch(dir: PathBuf) -> Result<PackageMap, nickel_embed::Error> {
    match std::env::var("NICKEL_EMBED_XD_REPO") {
        Ok(mirror) => xd::package_map_from_mirror(dir, &mirror),
        Err(_) => xd::package_map_with_cache(dir),
    }
}

/// From the cache filled above, so with no network.
fn packages() -> PackageMap {
    xd::package_map_with_cache(cache_dir()).unwrap()
}

fn context() -> Context {
    Context::new().with_package_map(packages())
}

fn pattern(code: &str) -> Pattern {
    Pattern::with_context(context(), code)
}

fn jd() -> Pattern {
    pattern("xd.units.jd.pattern")
}

fn entry(address: &[u64], link: Option<&[u64]>, label: Option<&str>) -> Entry {
    Entry {
        address: address.to_vec(),
        link: link.map(<[u64]>::to_vec),
        label: label.map(str::to_owned),
    }
}

#[test]
fn the_package_is_imported_by_name() {
    let mut ctx = context();
    let expr = ctx
        .eval_deep("(import xd).units.jd.scheme.levels |> std.array.map (fun l => l.name)")
        .unwrap();
    let names: Vec<String> = expr.to_serde().unwrap();
    assert_eq!(names, ["area", "category", "id"]);
}

#[test]
fn the_cached_package_is_the_pinned_commit() {
    let map = packages();
    let [(name, dir)] = map.packages()[..] else {
        panic!("{map}")
    };
    assert_eq!(name, "xd");
    assert!(
        dir.ends_with(std::path::Path::new(xd::TREE).join(xd::PATH)),
        "{dir:?}"
    );
    let manifest = std::fs::read_to_string(dir.join("Nickel-pkg.ncl")).unwrap();
    assert!(
        manifest.contains(&format!("version = \"{}\"", xd::VERSION)),
        "{manifest}"
    );
}

#[test]
fn programs_import_it_too() {
    let mut program = nickel_embed::Program::builder()
        .source(nickel_embed::Source::code(
            r#"{ id = (import xd).units.jd.pattern.render [1, 1, 11] }"#,
        ))
        .package_map(packages())
        .build()
        .unwrap();
    let json = program.export(nickel_embed::ExportFormat::Json).unwrap();
    assert!(json.contains(r#""id": "11.11""#), "{json}");
}

#[test]
fn without_the_package_the_import_fails() {
    let err = Context::new().eval_deep("import xd").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Import);
}

#[test]
fn jd_renders_and_parses() {
    let mut jd = jd();
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
    let mut jd = jd();
    for name in ["11.11 Passport", "10-19 Life admin", "11", "99.99"] {
        let parsed = jd.parse(name).unwrap().unwrap();
        assert_eq!(jd.write(&parsed).unwrap(), name);
    }
}

#[test]
fn labels_are_passed_through_exactly() {
    let mut jd = jd();
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
    let mut alone = pattern("xd.units.work_packages.work_packages");
    assert_eq!(
        alone.parse("W0012 Taxes").unwrap(),
        Some(entry(&[12], None, Some("Taxes")))
    );
    assert_eq!(alone.parse("W0012~11.13").unwrap(), None);

    let mut on_jd = pattern("(xd.units.jd & xd.units.work_packages).work_packages");
    let linked = entry(&[12], Some(&[1, 1, 13]), Some("2026 tax return"));
    assert_eq!(
        on_jd.parse("W0012~11.13 2026 tax return").unwrap(),
        Some(linked.clone())
    );
    assert_eq!(on_jd.write(&linked).unwrap(), "W0012~11.13 2026 tax return");
}

#[test]
fn custom_patterns_see_the_primitives() {
    let mut boards = pattern(
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
    let scheme: Scheme = jd().scheme().unwrap();
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
    let mut jd = jd();
    assert!(matches!(jd.render(&[1, 1, 100]), Err(Error::Nickel(_))));
    assert!(matches!(jd.render(&[]), Err(Error::Nickel(_))));

    let mut bad = pattern("xd.pattern [xd.sep.within]");
    match bad.render(&[1]) {
        Err(Error::Nickel(e)) => assert!(e.to_string().contains("at least one slot"), "{e}"),
        other => panic!("expected a Nickel error, got {:?}", other.map(|_| ())),
    }

    let mut wp = pattern("(xd.units.jd & xd.units.work_packages).work_packages");
    assert!(matches!(
        wp.scheme::<serde_json::Value>(),
        Err(Error::Nickel(_))
    ));
}

/// The `packages` feature fetches https git dependencies too: a manifest that depends on xD
/// the documented way resolves to the same package.
#[test]
fn a_manifest_can_depend_on_the_pin_over_https() {
    if std::env::var("NICKEL_EMBED_XD_REPO").is_ok() {
        return; // offline run: this test is about fetching from GitHub
    }
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("Nickel-pkg.ncl");
    std::fs::write(
        &manifest,
        format!(
            r#"{{ name = "app", version = "0.1.0", minimal_nickel_version = "1.18.0", authors = [],
  dependencies = {{ xd = 'Git {{ url = "{}", ref = 'Commit "{}", path = "{}" }} }} }}
| std.package.Manifest"#,
            xd::GIT_URL,
            xd::COMMIT,
            xd::PATH
        ),
    )
    .unwrap();
    let map = PackageMap::from_manifest_with_cache(&manifest, dir.path().join("cache")).unwrap();
    let mut ctx = Context::new().with_package_map(map);
    let expr = ctx
        .eval_deep("(import xd).units.jd.pattern.render [1, 1, 11]")
        .unwrap();
    assert_eq!(expr.as_str(), Some("11.11"));
}
