//! Nickel packages (feature `package`), with a path dependency, so no network is needed.
#![cfg(feature = "package")]

use nickel_embed::{Context, PackageCache, Packages};

#[test]
fn a_manifests_dependencies_import_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("greeting");
    std::fs::create_dir(&lib).unwrap();
    std::fs::write(
        lib.join("Nickel-pkg.ncl"),
        r#"{ name = "greeting", version = "1.0.0", minimal_nickel_version = "1.18.0", authors = [], dependencies = {} } | std.package.Manifest"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("main.ncl"),
        r#"{ hello = fun who => "hello " ++ who }"#,
    )
    .unwrap();
    let manifest = dir.path().join("Nickel-pkg.ncl");
    std::fs::write(
        &manifest,
        r#"{ name = "app", version = "0.1.0", minimal_nickel_version = "1.18.0", authors = [], dependencies = { greeting = 'Path "greeting" } } | std.package.Manifest"#,
    )
    .unwrap();

    let cache = PackageCache::at(dir.path().join("cache")).unwrap();
    let packages = Packages::from_manifest(&manifest, &cache).unwrap();
    assert!(
        dir.path().join("Nickel-pkg.lock").is_file(),
        "locked like `nickel` does"
    );
    assert_eq!(
        packages
            .top_level()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["greeting"]
    );

    let mut ctx = Context::new().with_packages(packages);
    let expr = ctx.eval_deep(r#"(import greeting).hello "world""#).unwrap();
    assert_eq!(expr.as_str(), Some("hello world"));
}

#[test]
fn a_bad_manifest_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("Nickel-pkg.ncl");
    std::fs::write(&manifest, "{ name = 1 }").unwrap();
    let cache = PackageCache::at(dir.path().join("cache")).unwrap();
    let err = Packages::from_manifest(&manifest, &cache).unwrap_err();
    // Nickel's own diagnostics, pointing into the manifest.
    assert_ne!(err.kind(), nickel_embed::ErrorKind::Io, "{err}");
    assert!(err.to_string().contains("version"), "{err}");
}
