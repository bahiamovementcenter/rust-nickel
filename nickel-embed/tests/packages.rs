use std::{
    fs,
    path::{Path, PathBuf},
};

use nickel_embed::{Context, ErrorKind, PackageMap, Program, Source};

/// A fresh directory for one test, under Cargo's scratch space for integration tests.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("packages")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn program_imports_a_package() {
    let dir = scratch("program");
    write(&dir.join("greeting/main.ncl"), r#"{ hello = "world" }"#);

    let mut program = Program::builder()
        .source(Source::code("(import greeting).hello"))
        .package_map(PackageMap::new().package("greeting", dir.join("greeting")))
        .build()
        .unwrap();
    assert_eq!(program.eval_deep().unwrap().as_str(), Some("world"));
}

#[test]
fn context_imports_a_package() {
    let dir = scratch("context");
    write(&dir.join("lib/main.ncl"), "{ double = fun x => 2 * x }");

    let mut ctx =
        Context::new().with_package_map(PackageMap::new().package("lib", dir.join("lib")));
    let expr = ctx.eval_deep("(import lib).double 21").unwrap();
    assert_eq!(expr.as_i64(), Some(42));
}

#[test]
fn dependencies_are_local_to_each_package() {
    let dir = scratch("transitive");
    // The program's `util` and the package's `util` are different packages.
    write(&dir.join("app/main.ncl"), "{ inner = (import util).name }");
    write(&dir.join("util-a/main.ncl"), r#"{ name = "a" }"#);
    write(&dir.join("util-b/main.ncl"), r#"{ name = "b" }"#);

    let map = PackageMap::new()
        .package("app", dir.join("app"))
        .package("util", dir.join("util-a"))
        // `..` checks that paths are normalized the same way on both sides.
        .dependency(dir.join("app/../app"), "util", dir.join("util-b"));
    assert_eq!(map.packages().len(), 2);
    assert_eq!(map.dependencies().len(), 1);

    let mut program = Program::builder()
        .source(Source::code(
            "{ outer = (import util).name, inner = (import app).inner }",
        ))
        .package_map(map)
        .build()
        .unwrap();
    let expr = program.eval_deep().unwrap();
    let record = expr.as_record().unwrap();
    assert_eq!(record.value_by_name("outer").unwrap().as_str(), Some("a"));
    assert_eq!(record.value_by_name("inner").unwrap().as_str(), Some("b"));
}

#[test]
fn missing_package_is_an_import_error() {
    let mut program = Program::builder()
        .source(Source::code("import nowhere"))
        .package_map(PackageMap::new())
        .build()
        .unwrap();
    let err = program.eval_deep().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Import);
    assert!(err.to_string().contains("nowhere"), "{err}");

    // Without any map at all, too.
    let err = Context::new().eval_deep("import nowhere").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Import);
}

#[cfg(feature = "packages")]
mod manifest {
    use super::*;

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
    fn path_dependencies_resolve_from_a_manifest() {
        let dir = scratch("manifest");
        let cache = dir.join("cache");
        write(
            &dir.join("app/Nickel-pkg.ncl"),
            &manifest("app", r#"lib = 'Path "../lib""#),
        );
        write(&dir.join("app/main.ncl"), "(import lib).value");
        write(
            &dir.join("lib/Nickel-pkg.ncl"),
            &manifest("lib", r#"leaf = 'Path "../leaf""#),
        );
        write(&dir.join("lib/main.ncl"), "{ value = (import leaf) + 1 }");
        write(&dir.join("leaf/Nickel-pkg.ncl"), &manifest("leaf", ""));
        write(&dir.join("leaf/main.ncl"), "41");

        let found = PackageMap::find_manifest(dir.join("app")).unwrap();
        assert_eq!(found, dir.join("app/Nickel-pkg.ncl"));

        let map = PackageMap::from_manifest_with_cache(&found, &cache).unwrap();
        assert_eq!(map.packages().len(), 1);
        assert!(dir.join("app/Nickel-pkg.lock").is_file());

        let mut program = Program::builder()
            .source(Source::file(dir.join("app/main.ncl")))
            .package_map(map)
            .build()
            .unwrap();
        assert_eq!(program.eval_deep().unwrap().as_i64(), Some(42));

        // Again, now that the lock file exists.
        let map = PackageMap::from_manifest_with_cache(&found, &cache).unwrap();
        let mut ctx = Context::new().with_package_map(map);
        assert_eq!(
            ctx.eval_deep("import lib")
                .unwrap()
                .as_record()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn find_manifest_searches_parents() {
        let dir = scratch("find");
        write(&dir.join("Nickel-pkg.ncl"), &manifest("root", ""));
        fs::create_dir_all(dir.join("a/b")).unwrap();
        assert_eq!(
            PackageMap::find_manifest(dir.join("a/b")),
            Some(dir.join("Nickel-pkg.ncl"))
        );
    }

    #[test]
    fn invalid_manifest_is_a_package_error() {
        let dir = scratch("invalid");
        // `version` must be a semantic version.
        write(
            &dir.join("Nickel-pkg.ncl"),
            &manifest("bad", "").replace("0.1.0", "one"),
        );
        let err =
            PackageMap::from_manifest_with_cache(dir.join("Nickel-pkg.ncl"), dir.join("cache"))
                .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Package);
        // The error points into the manifest.
        assert!(!err.diagnostics().is_empty());
    }

    #[test]
    fn missing_path_dependency_is_a_package_error() {
        let dir = scratch("missing-dep");
        write(
            &dir.join("Nickel-pkg.ncl"),
            &manifest("app", r#"gone = 'Path "../does-not-exist""#),
        );
        let err =
            PackageMap::from_manifest_with_cache(dir.join("Nickel-pkg.ncl"), dir.join("cache"))
                .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Package);
    }
}
