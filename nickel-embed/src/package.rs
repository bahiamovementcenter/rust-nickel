//! Nickel packages (Cargo feature `package`): `import name` of a library declared in a
//! `Nickel-pkg.ncl` manifest, as `nickel` built with `package-experimental` handles them.
//!
//! Package management is experimental in Nickel itself, and so is this module.
//!
//! ```no_run
//! use nickel_embed::{Context, PackageCache, Packages};
//!
//! // Locks the manifest's dependencies (writing `Nickel-pkg.lock` next to it, as `nickel`
//! // does) and fetches any that aren't in the cache yet.
//! let packages = Packages::from_manifest("config/Nickel-pkg.ncl", &PackageCache::new()?)?;
//! let mut ctx = Context::new().with_packages(packages);
//! let expr = ctx.eval_deep("(import my_dependency).value")?;
//! # Ok::<(), nickel_embed::Error>(())
//! ```

use std::path::{Path, PathBuf};

use nickel_lang_core::{
    error::{Error as NickelCoreError, IOError},
    files::Files,
    package::PackageMap,
};
use nickel_lang_package::{ManifestFile, config::Config};

use crate::Error;

/// Where fetched packages are kept, and where they're fetched from.
#[derive(Clone, Debug)]
pub struct PackageCache {
    pub(crate) config: Config,
}

impl PackageCache {
    /// Nickel's own cache folder (on Linux, `$XDG_CACHE_HOME/nickel`), shared with the
    /// `nickel` command line tool.
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            config: Config::new().map_err(package_error)?,
        })
    }

    /// A cache in the given folder.
    pub fn at(dir: impl Into<PathBuf>) -> Result<Self, Error> {
        Ok(Self {
            config: Config::new()
                .map_err(package_error)?
                .with_cache_dir(dir.into()),
        })
    }

    /// Fetches git packages from `replacement` whenever a dependency names `url`: for mirrors,
    /// vendored copies and tests. The dependency's pinned commit still has to be there.
    pub fn replace_git(mut self, url: &str, replacement: &str) -> Result<Self, Error> {
        let parse = |u: &str| {
            gix::Url::try_from(u)
                .map_err(|e| package_message(format!("invalid git url `{u}`: {e}")))
        };
        self.config
            .git_replacements
            .insert(parse(url)?, parse(replacement)?);
        Ok(self)
    }

    /// The folder a git package whose top-level tree is `tree` is unpacked in.
    #[cfg_attr(not(feature = "xd"), allow(dead_code))]
    pub(crate) fn git_package_dir(&self, tree: &str) -> PathBuf {
        self.config.git_package_dir.join(tree)
    }
}

/// The packages a program can import by name, with where each one is on disk.
#[derive(Clone, Debug, Default)]
pub struct Packages {
    pub(crate) map: PackageMap,
}

impl Packages {
    /// The dependencies of a `Nickel-pkg.ncl` manifest, as `nickel` resolves them: through its
    /// lock file, which is written next to the manifest when it's missing or out of date, and
    /// fetching what isn't in `cache` yet.
    pub fn from_manifest(manifest: impl AsRef<Path>, cache: &PackageCache) -> Result<Self, Error> {
        let manifest = ManifestFile::from_path(manifest.as_ref()).map_err(package_error)?;
        let (_lock, resolution) = manifest.lock(cache.config.clone()).map_err(package_error)?;
        nickel_lang_package::index::ensure_index_packages_downloaded(&resolution)
            .map_err(package_error)?;
        let map = resolution.package_map(&manifest).map_err(package_error)?;
        Ok(Self { map })
    }

    /// Both sets of packages. Where both have a top-level package of the same name, `other`'s
    /// is kept.
    pub fn merge(mut self, other: Packages) -> Self {
        self.map.top_level.extend(other.map.top_level);
        self.map.packages.extend(other.map.packages);
        self
    }

    /// The top-level packages, by the name they're imported with, and the folder each is in.
    pub fn top_level(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.map
            .top_level
            .iter()
            .map(|(name, path)| (name.label(), path.as_path()))
    }
}

impl crate::Context {
    /// Makes `packages` importable by name: `import my_dependency`. Packages added earlier stay,
    /// unless one of the new ones has the same name.
    pub fn with_packages(mut self, packages: Packages) -> Self {
        let sources = &mut self.vm_ctxt.import_resolver.sources;
        let merged = match sources.package_map.take() {
            Some(map) => Packages { map }.merge(packages),
            None => packages,
        };
        sources.set_package_map(merged.map);
        self
    }
}

pub(crate) fn package_message(message: String) -> Error {
    Error {
        error: Box::new(NickelCoreError::IOError(IOError(message))),
        files: Files::empty(),
    }
}

/// A manifest that fails to evaluate keeps Nickel's own error, with its diagnostics. Other
/// package errors become [`crate::ErrorKind::Io`] errors that carry the package manager's
/// message: fetching, locking and reading manifests are all I/O from the program's side.
pub(crate) fn package_error(e: nickel_lang_package::error::Error) -> Error {
    match e {
        nickel_lang_package::error::Error::ManifestEval { files, error, .. } => {
            Error { error, files }
        }
        e => package_message(format!("Nickel package: {e}")),
    }
}
