//! Package imports: `import foo` resolved through a package map.

use std::{
    ffi::{c_char, c_int},
    mem,
};

use nickel_embed::PackageMap;

use crate::{
    context, finish, nickel_context, nickel_error, nickel_program_builder, nickel_result,
    nickel_string, program::builder, set_string, utf8,
};

/// Where package imports (`import foo`) are found: a map from package names to directories.
///
/// Each package is a directory, and importing it evaluates its `main.ncl`. Names are local to
/// the importer: the program's own packages are added with [`nickel_package_map_add_package`],
/// and those of a package with [`nickel_package_map_add_dependency`], so two packages can each
/// have their own `foo`. [`nickel_package_map_load_manifest`] instead fills the map from a
/// `Nickel-pkg.ncl` manifest, the way the `nickel` command does.
///
/// Use a map with [`nickel_context_set_package_map`] or
/// [`nickel_program_builder_set_package_map`], which copy it. Allocate one with
/// [`nickel_package_map_alloc`] and free it with [`nickel_package_map_free`].
pub struct nickel_package_map {
    inner: PackageMap,
}

unsafe fn map<'a>(m: *mut nickel_package_map) -> &'a mut PackageMap {
    // Safety: `m` came from `nickel_package_map_alloc`.
    unsafe { &mut m.as_mut().expect("null nickel_package_map").inner }
}

unsafe fn map_ref<'a>(m: *const nickel_package_map) -> &'a PackageMap {
    // Safety: `m` came from `nickel_package_map_alloc`.
    unsafe { &m.as_ref().expect("null nickel_package_map").inner }
}

/// Allocate a new, empty [`nickel_package_map`]: every package import fails until packages are
/// added. Free it with [`nickel_package_map_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_alloc() -> *mut nickel_package_map {
    Box::into_raw(Box::new(nickel_package_map {
        inner: PackageMap::new(),
    }))
}

/// Free a [`nickel_package_map`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_free(m: *mut nickel_package_map) {
    if !m.is_null() {
        // Safety: `m` came from `nickel_package_map_alloc`.
        drop(unsafe { Box::from_raw(m) });
    }
}

/// Make `import name` in the program's own sources load the package in the directory `dir`,
/// replacing any package of that name.
///
/// `name` and `dir` are null-terminated, UTF-8 strings, copied. A relative `dir` is taken from
/// the current directory. Fails only if a string isn't UTF-8; a missing directory is reported
/// when the package is imported.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_add_package(
    m: *mut nickel_package_map,
    name: *const c_char,
    dir: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(name, "the package name")
            .and_then(|name| Ok((name, utf8(dir, "the package directory")?)));
        finish(result, out_error, |(name, dir)| {
            let m = map(m);
            *m = mem::take(m).package(name, dir);
        })
    }
}

/// Make `import name` inside the package in the directory `parent` load the package in `dir`.
///
/// `parent` must be the directory the parent package was added with (by
/// [`nickel_package_map_add_package`] or as another package's dependency). Strings are as in
/// [`nickel_package_map_add_package`]. Fails only if a string isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_add_dependency(
    m: *mut nickel_package_map,
    parent: *const c_char,
    name: *const c_char,
    dir: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(parent, "the parent directory").and_then(|parent| {
            Ok((
                parent,
                utf8(name, "the package name")?,
                utf8(dir, "the package directory")?,
            ))
        });
        finish(result, out_error, |(parent, name, dir)| {
            let m = map(m);
            *m = mem::take(m).dependency(parent, name, dir);
        })
    }
}

/// Replace the contents of the map with the packages of a manifest (`Nickel-pkg.ncl`), like the
/// `nickel` command does before evaluating a package.
///
/// This resolves versions, fetches git and index dependencies that aren't cached yet, and
/// writes or updates `Nickel-pkg.lock` next to the manifest. Fetched packages are cached in
/// `cache_dir` if it isn't NULL (like `--package-cache-dir`), and otherwise in the platform's
/// cache directory, such as `~/.cache/nickel`. Nickel's package manager is experimental: its
/// manifest and lock formats may change.
///
/// - `manifest` is the null-terminated, UTF-8 path of the manifest; see
///   [`nickel_package_map_find_manifest`].
/// - On failure, the map is unchanged and the error, of kind `NICKEL_ERROR_KIND_PACKAGE`, is
///   stored in `out_error` (if not NULL). Errors in the manifest's own code have diagnostics
///   pointing into it.
///
/// Libraries built without the `packages` Cargo feature (on by default) always fail.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_load_manifest(
    m: *mut nickel_package_map,
    manifest: *const c_char,
    cache_dir: *const c_char,
    out_error: *mut nickel_error,
) -> nickel_result {
    // Safety: pre-conditions of this function.
    unsafe {
        let result = utf8(manifest, "the manifest path").and_then(|manifest| {
            let cache_dir = if cache_dir.is_null() {
                None
            } else {
                Some(utf8(cache_dir, "the cache directory")?)
            };
            load_manifest(manifest, cache_dir)
        });
        finish(result, out_error, |loaded| *map(m) = loaded)
    }
}

#[cfg(feature = "packages")]
fn load_manifest(
    manifest: &str,
    cache_dir: Option<&str>,
) -> Result<PackageMap, nickel_embed::Error> {
    match cache_dir {
        Some(cache_dir) => PackageMap::from_manifest_with_cache(manifest, cache_dir),
        None => PackageMap::from_manifest(manifest),
    }
}

#[cfg(not(feature = "packages"))]
fn load_manifest(
    _manifest: &str,
    _cache_dir: Option<&str>,
) -> Result<PackageMap, nickel_embed::Error> {
    Err(crate::usage_error(
        "this library was built without package manifest support (the `packages` feature)"
            .to_owned(),
    ))
}

/// Find the manifest for the directory `dir`: the nearest `Nickel-pkg.ncl` in it or a parent
/// directory, as the `nickel` command looks for one.
///
/// Returns 1 and writes the manifest's path to `out_path` (if not NULL, allocated with
/// [`nickel_string_alloc`]) if there is one, and returns 0 otherwise, or if `dir` isn't UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_find_manifest(
    dir: *const c_char,
    out_path: *mut nickel_string,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        let Ok(dir) = utf8(dir, "the directory") else {
            return 0;
        };
        match PackageMap::find_manifest(dir) {
            Some(path) => {
                set_string(out_path, path.to_string_lossy().into_owned());
                1
            }
            None => 0,
        }
    }
}

/// The number of the program's own packages (those added with
/// [`nickel_package_map_add_package`], or a manifest's direct dependencies).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_package_count(m: *const nickel_package_map) -> usize {
    // Safety: pre-conditions of this function.
    unsafe { map_ref(m) }.packages().len()
}

/// Write the name and directory of the program's package number `index`, in name order, to
/// `out_name` and `out_dir` (each NULL or allocated with [`nickel_string_alloc`]). Returns 1,
/// or 0 if `index` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_package(
    m: *const nickel_package_map,
    index: usize,
    out_name: *mut nickel_string,
    out_dir: *mut nickel_string,
) -> c_int {
    // Safety: pre-conditions of this function.
    unsafe {
        match map_ref(m).packages().get(index) {
            Some((name, dir)) => {
                set_string(out_name, (*name).to_owned());
                set_string(out_dir, dir.to_string_lossy().into_owned());
                1
            }
            None => 0,
        }
    }
}

/// Write a description of the whole map, with every package's dependencies, to `out_string`
/// (allocated with [`nickel_string_alloc`]), as `nickel package debug-resolution` prints it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_package_map_to_string(
    m: *const nickel_package_map,
    out_string: *mut nickel_string,
) {
    // Safety: pre-conditions of this function.
    unsafe { set_string(out_string, map_ref(m).to_string()) }
}

/// Resolve package imports in this context with a copy of `map`, replacing any map set before.
/// Without one, package imports fail.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_context_set_package_map(
    ctx: *mut nickel_context,
    map: *const nickel_package_map,
) {
    // Safety: pre-conditions of this function.
    unsafe {
        let map = map_ref(map).clone();
        let ctx = context(ctx);
        *ctx = mem::take(ctx).with_package_map(map);
    }
}

/// Resolve the program's package imports with a copy of `map`, replacing any map set before.
/// Without one, package imports fail.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nickel_program_builder_set_package_map(
    b: *mut nickel_program_builder,
    map: *const nickel_package_map,
) {
    // Safety: pre-conditions of this function.
    unsafe {
        let map = map_ref(map).clone();
        builder(b).update(|b| b.package_map(map));
    }
}
