//! Package imports: `import foo` resolves through a [`PackageMap`].
//!
//! A map can be built by hand from directories you already have (fetched by Nix, vendored, and so
//! on), with no extra dependencies. With the `packages` feature, [`PackageMap::from_manifest`]
//! builds it from a `Nickel-pkg.ncl` manifest the way the `nickel` command does: it resolves
//! versions, fetches git and index dependencies into a cache and writes `Nickel-pkg.lock`.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use nickel_lang_core::{cache::normalize_path, identifier::Ident, package::PackageMap as CoreMap};

/// Where package imports (`import foo`) are found.
///
/// Each package is a directory, and importing it evaluates its `main.ncl`. Names are local to the
/// importer: the program's own dependencies are set with [`Self::package`], and those of a
/// package with [`Self::dependency`], so two packages can each have their own `foo`.
///
/// ```rust
/// use nickel_embed::{PackageMap, Program, Source};
///
/// let dir = std::env::temp_dir().join("nickel-embed-doc-greeting");
/// std::fs::create_dir_all(&dir)?;
/// std::fs::write(dir.join("main.ncl"), r#"{ hello = "world" }"#)?;
///
/// let mut program = Program::builder()
///     .source(Source::code("(import greeting).hello"))
///     .package_map(PackageMap::new().package("greeting", &dir))
///     .build()?;
/// assert_eq!(program.eval_deep()?.as_str(), Some("world"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct PackageMap {
    inner: CoreMap,
}

impl PackageMap {
    /// An empty map: every package import fails until packages are added.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes `import name` in the program's own sources load the package in `dir`.
    ///
    /// A relative `dir` is taken from the current directory.
    pub fn package(mut self, name: &str, dir: impl AsRef<Path>) -> Self {
        self.inner
            .top_level
            .insert(Ident::new(name), normalize(dir.as_ref()));
        self
    }

    /// Makes `import name` inside the package in `parent` load the package in `dir`.
    ///
    /// `parent` must be the directory the parent package was added with.
    pub fn dependency(
        mut self,
        parent: impl AsRef<Path>,
        name: &str,
        dir: impl AsRef<Path>,
    ) -> Self {
        self.inner.packages.insert(
            (normalize(parent.as_ref()), Ident::new(name)),
            normalize(dir.as_ref()),
        );
        self
    }

    /// The program's own packages, by name, sorted.
    pub fn packages(&self) -> Vec<(&str, &Path)> {
        let mut packages: Vec<_> = self
            .inner
            .top_level
            .iter()
            .map(|(name, dir)| (name.label(), dir.as_path()))
            .collect();
        packages.sort();
        packages
    }

    /// The dependencies of packages, as `(parent, name, dir)`, sorted.
    pub fn dependencies(&self) -> Vec<(&Path, &str, &Path)> {
        let mut deps: Vec<_> = self
            .inner
            .packages
            .iter()
            .map(|((parent, name), dir)| (parent.as_path(), name.label(), dir.as_path()))
            .collect();
        deps.sort();
        deps
    }

    pub(crate) fn into_core(self) -> CoreMap {
        self.inner
    }
}

/// The map as `nickel package debug-resolution` prints it.
impl fmt::Display for PackageMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

// Nickel looks up a package's dependencies by the exact path the package was loaded from, so
// every path goes through the same normalization the package manager uses.
fn normalize(dir: &Path) -> PathBuf {
    normalize_path(dir).unwrap_or_else(|_| dir.to_owned())
}

#[cfg(feature = "packages")]
mod manifest {
    use std::path::{Path, PathBuf};

    use nickel_lang_core::{error::IOError, files::Files};
    use nickel_lang_package::{
        ManifestFile, config::Config, error::Error as PackageError, index, manifest::MANIFEST_NAME,
    };

    use super::PackageMap;
    use crate::{Error, ErrorKind};

    impl PackageMap {
        /// Builds the map for a package manifest (`Nickel-pkg.ncl`), like the `nickel` command
        /// does before evaluating. Requires the `packages` feature.
        ///
        /// This resolves versions, fetches git and index dependencies that aren't cached (into
        /// the platform's cache directory, such as `~/.cache/nickel`), and writes or updates
        /// `Nickel-pkg.lock` next to the manifest. Nickel's package manager is experimental: its
        /// manifest and lock formats may change.
        pub fn from_manifest(manifest: impl AsRef<Path>) -> Result<Self, Error> {
            resolve(manifest.as_ref(), None)
        }

        /// Like [`Self::from_manifest`], with fetched packages cached in `cache_dir`, like
        /// `--package-cache-dir`.
        pub fn from_manifest_with_cache(
            manifest: impl AsRef<Path>,
            cache_dir: impl Into<PathBuf>,
        ) -> Result<Self, Error> {
            resolve(manifest.as_ref(), Some(cache_dir.into()))
        }

        /// Finds the manifest for `dir`: the nearest `Nickel-pkg.ncl` in it or a parent
        /// directory, as the `nickel` command looks for one. Requires the `packages` feature.
        pub fn find_manifest(dir: impl AsRef<Path>) -> Option<PathBuf> {
            dir.as_ref()
                .ancestors()
                .map(|dir| dir.join(MANIFEST_NAME))
                .find(|path| path.is_file())
        }
    }

    fn resolve(path: &Path, cache_dir: Option<PathBuf>) -> Result<PackageMap, Error> {
        // The package manager takes the manifest's parent as the project root and panics if
        // there's none, so give it an absolute path.
        let path = std::path::absolute(path)?;
        let manifest = ManifestFile::from_path(&path).map_err(package_error)?;
        let mut config = Config::new().map_err(package_error)?;
        if let Some(dir) = cache_dir {
            config = config.with_cache_dir(dir);
        }
        let (_lock, resolution) = manifest.lock(config).map_err(package_error)?;
        index::ensure_index_packages_downloaded(&resolution).map_err(package_error)?;
        let inner = resolution.package_map(&manifest).map_err(package_error)?;
        Ok(PackageMap { inner })
    }

    fn package_error(error: PackageError) -> Error {
        let (error, files) = match error {
            // The manifest is Nickel code, so its errors point into it.
            PackageError::ManifestEval { files, error, .. } => (error, files),
            other => (Box::new(IOError(other.to_string()).into()), Files::empty()),
        };
        Error {
            error,
            files,
            kind: Some(ErrorKind::Package),
        }
    }
}
