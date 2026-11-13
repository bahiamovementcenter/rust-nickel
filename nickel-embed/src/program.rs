//! Programs: evaluation requests with files, several inputs, overrides, contracts and a field
//! focus, matching what the `nickel` command line tool can do.

use std::{
    ffi::OsString,
    fmt,
    io::{self, Cursor, Read, Write},
    path::PathBuf,
    str::FromStr,
};

use nickel_lang_core::{
    cache::{CacheHub, InputFormat as CoreInputFormat},
    error::{Error as NickelCoreError, IOError, PointedExportErrorData},
    eval::{Closure, cache::CacheImpl},
    files::Files,
    identifier::{Ident, LocIdent},
    program::{
        BuilderError, FieldOverride, FieldPath as CoreFieldPath, Program as CoreProgram,
        ProgramBuilder as CoreProgramBuilder,
    },
    serialize::{self, ExportFormat as CoreExportFormat},
    term::{self, record::Field as CoreField},
    typecheck::TypecheckMode as CoreTypecheckMode,
};

use crate::{Diagnostic, Error, Expr, FieldMetadata, diagnostic::WarningReporter};

/// The format of a [`Source`].
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputFormat {
    #[default]
    Nickel,
    Json,
    Yaml,
    Toml,
    /// Plain text, read as a single Nickel string.
    Text,
}

impl InputFormat {
    /// Guesses the format from a file extension (`ncl`, `json`, `yaml`/`yml`, `toml`, `txt`).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ncl" => Some(Self::Nickel),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "txt" => Some(Self::Text),
            _ => None,
        }
    }

    fn to_core(self) -> CoreInputFormat {
        match self {
            Self::Nickel => CoreInputFormat::Nickel,
            Self::Json => CoreInputFormat::Json,
            Self::Yaml => CoreInputFormat::Yaml,
            Self::Toml => CoreInputFormat::Toml,
            Self::Text => CoreInputFormat::Text,
        }
    }
}

/// An output format for [`Program::export`] and [`Expr::export`].
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    #[default]
    Json,
    Yaml,
    /// Several YAML documents separated by `---`. The value must be an array: each element
    /// becomes a document.
    YamlDocuments,
    Toml,
    /// The value must be a string, written as is.
    Text,
}

impl ExportFormat {
    pub(crate) fn to_core(self) -> CoreExportFormat {
        match self {
            Self::Json => CoreExportFormat::Json,
            Self::Yaml => CoreExportFormat::Yaml,
            Self::YamlDocuments => CoreExportFormat::YamlDocuments,
            Self::Toml => CoreExportFormat::Toml,
            Self::Text => CoreExportFormat::Text,
        }
    }
}

/// How strictly [`Program::typecheck`] checks types.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TypecheckMode {
    /// Only check code inside statically typed blocks (`: Type` annotations). This is what
    /// evaluation does.
    #[default]
    Walk,
    /// Check everything as if the whole program were statically typed.
    Enforce,
}

/// One input to a [`Program`].
pub struct Source {
    kind: SourceKind,
    format: Option<InputFormat>,
    name: Option<String>,
}

enum SourceKind {
    Code(String),
    File(PathBuf),
    Reader(Box<dyn Read>),
}

impl Source {
    /// Source code held in memory. It is Nickel unless [`Self::with_format`] says otherwise.
    ///
    /// Relative imports resolve against the current directory, then the import paths.
    pub fn code(src: impl Into<String>) -> Self {
        Self::new(SourceKind::Code(src.into()))
    }

    /// A file, read when the program is built. Its format comes from its extension unless
    /// [`Self::with_format`] says otherwise. Relative imports resolve against its directory.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::new(SourceKind::File(path.into()))
    }

    /// Anything readable, read when the program is built.
    pub fn reader(reader: impl Read + 'static) -> Self {
        Self::new(SourceKind::Reader(Box::new(reader)))
    }

    /// Sets the input format.
    pub fn with_format(mut self, format: InputFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets the name shown in error messages. Files are named by their path otherwise.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    fn new(kind: SourceKind) -> Self {
        Self {
            kind,
            format: None,
            name: None,
        }
    }

    fn add_to<R, W>(
        self,
        builder: CoreProgramBuilder<R, W>,
        index: usize,
    ) -> CoreProgramBuilder<R, W> {
        let default_name = || OsString::from(format!("<source {index}>"));
        match (self.kind, self.format, self.name) {
            // Core infers the format from the extension; that's the only way to keep the file's
            // own directory as the base for its relative imports.
            (SourceKind::File(path), None, None) => builder.add_path(path),
            (SourceKind::File(path), format, name) => {
                let format = format
                    .or_else(|| {
                        path.extension()
                            .and_then(|e| e.to_str())
                            .and_then(InputFormat::from_extension)
                    })
                    .unwrap_or_default();
                let name = name.map(OsString::from).unwrap_or(path.clone().into());
                builder.add_source_with_format(LazyFile::new(path), name, format.to_core())
            }
            (SourceKind::Code(code), format, name) => builder.add_source_with_format(
                Cursor::new(code),
                name.map(OsString::from).unwrap_or_else(default_name),
                format.unwrap_or_default().to_core(),
            ),
            (SourceKind::Reader(reader), format, name) => builder.add_source_with_format(
                reader,
                name.map(OsString::from).unwrap_or_else(default_name),
                format.unwrap_or_default().to_core(),
            ),
        }
    }
}

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match &self.kind {
            SourceKind::Code(_) => "code".to_owned(),
            SourceKind::File(path) => format!("file {}", path.display()),
            SourceKind::Reader(_) => "reader".to_owned(),
        };
        f.debug_struct("Source")
            .field("kind", &kind)
            .field("format", &self.format)
            .field("name", &self.name)
            .finish()
    }
}

/// Opens its file on the first read, so a missing file is reported when the program is built
/// rather than when the source is created.
struct LazyFile {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl LazyFile {
    fn new(path: PathBuf) -> Self {
        Self { path, file: None }
    }
}

impl Read for LazyFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.file.is_none() {
            self.file = Some(std::fs::File::open(&self.path)?);
        }
        self.file.as_mut().expect("just opened").read(buf)
    }
}

/// A path to a field inside nested records, such as `server.ports.http`.
///
/// Parse one from a string with [`str::parse`]: segments are separated by `.`, and a segment that
/// isn't a plain identifier goes in double quotes (`labels."app.kubernetes.io/name"`). Or build
/// one from segments with [`FromIterator`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FieldPath(Vec<String>);

impl FieldPath {
    /// The segments of the path.
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    fn to_core(&self) -> CoreFieldPath {
        CoreFieldPath(
            self.0
                .iter()
                .map(|s| LocIdent::from(Ident::new(s)))
                .collect(),
        )
    }
}

impl FromStr for FieldPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let mut cache = CacheHub::new();
        match CoreFieldPath::parse(&mut cache, s.to_owned()) {
            Ok(path) => Ok(FieldPath(
                path.0.iter().map(|id| id.label().to_owned()).collect(),
            )),
            Err(e) => Err(Error {
                error: Box::new(e.into()),
                files: cache.sources.files().clone(),
            }),
        }
    }
}

impl<S: Into<String>> FromIterator<S> for FieldPath {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        FieldPath(iter.into_iter().map(Into::into).collect())
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_core().fmt(f)
    }
}

/// A merge priority for an override, as in Nickel's `default`, `priority n` and `force`.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Priority {
    /// `default`: the lowest priority.
    Default,
    /// No annotation, the same as `priority 0`. The command line uses this for `--override`.
    #[default]
    Neutral,
    /// `priority n`.
    Numeral(i64),
    /// `force`: the highest priority.
    Force,
}

impl Priority {
    fn to_core(&self) -> term::MergePriority {
        match self {
            Self::Default => term::MergePriority::Bottom,
            Self::Neutral => term::MergePriority::Neutral,
            Self::Numeral(n) => term::MergePriority::Numeral((*n).into()),
            Self::Force => term::MergePriority::Top,
        }
    }
}

/// Configures a [`Program`]. Nothing is read until [`Self::build`].
///
/// ```rust
/// use nickel_embed::{Program, Source};
///
/// let expr = Program::builder()
///     .source(Source::code("{ port | Number | default = 80, name = \"server\" }"))
///     .override_value("port".parse().unwrap(), "8080")
///     .build()
///     .unwrap()
///     .eval_deep()
///     .unwrap();
/// let port = expr.as_record().unwrap().value_by_name("port").unwrap();
/// assert_eq!(port.as_i64(), Some(8080));
/// ```
#[must_use]
pub struct ProgramBuilder {
    sources: Vec<Source>,
    import_paths: Vec<PathBuf>,
    contract_files: Vec<PathBuf>,
    overrides: Vec<(FieldPath, String, Priority)>,
    field: FieldPath,
    trace: Box<dyn Write>,
    warnings: WarningReporter,
    #[cfg(feature = "package")]
    packages: Option<crate::Packages>,
}

impl Default for ProgramBuilder {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            import_paths: Vec::new(),
            contract_files: Vec::new(),
            overrides: Vec::new(),
            field: FieldPath::default(),
            trace: Box::new(io::sink()),
            warnings: WarningReporter::ignore(),
            #[cfg(feature = "package")]
            packages: None,
        }
    }
}

impl ProgramBuilder {
    /// Adds an input. With several inputs, the program is their merge, like
    /// `nickel eval a.ncl b.ncl`.
    pub fn source(mut self, source: Source) -> Self {
        self.sources.push(source);
        self
    }

    /// Adds several inputs. See [`Self::source`].
    pub fn sources(mut self, sources: impl IntoIterator<Item = Source>) -> Self {
        self.sources.extend(sources);
        self
    }

    /// Adds a directory to search for imports that aren't found relative to the importing file.
    pub fn import_path(mut self, dir: impl Into<PathBuf>) -> Self {
        self.import_paths.push(dir.into());
        self
    }

    /// Adds several import directories. See [`Self::import_path`].
    pub fn import_paths<P: Into<PathBuf>>(mut self, dirs: impl IntoIterator<Item = P>) -> Self {
        self.import_paths.extend(dirs.into_iter().map(Into::into));
        self
    }

    /// Adds the directories in `NICKEL_IMPORT_PATH` (`:`-separated, `;` on Windows), as the
    /// `nickel` command does. Embedded programs ignore that variable unless you call this.
    pub fn import_paths_from_env(self) -> Self {
        match std::env::var_os("NICKEL_IMPORT_PATH") {
            Some(paths) => {
                let dirs: Vec<PathBuf> = std::env::split_paths(&paths).collect();
                self.import_paths(dirs)
            }
            None => self,
        }
    }

    /// Applies the contract in a Nickel file to the whole program, like `--apply-contract`.
    pub fn apply_contract_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.contract_files.push(path.into());
        self
    }

    /// Overrides a field with a value written in Nickel, like `--override path=value`.
    ///
    /// The value is evaluated on its own, so it can't refer to other fields. It has the neutral
    /// priority, so it replaces `default` values but conflicts with other plain definitions.
    pub fn override_value(self, path: FieldPath, nickel_code: impl Into<String>) -> Self {
        self.override_with_priority(path, nickel_code, Priority::Neutral)
    }

    /// Like [`Self::override_value`], with a chosen merge priority. Use [`Priority::Force`]
    /// to replace a field however it was defined.
    pub fn override_with_priority(
        mut self,
        path: FieldPath,
        nickel_code: impl Into<String>,
        priority: Priority,
    ) -> Self {
        self.overrides.push((path, nickel_code.into(), priority));
        self
    }

    /// Evaluates, exports or queries only this field, like `--field`.
    pub fn field(mut self, path: FieldPath) -> Self {
        self.field = path;
        self
    }

    /// Sends the output of `std.trace` here. It is discarded otherwise.
    pub fn trace(mut self, sink: impl Write + 'static) -> Self {
        self.trace = Box::new(sink);
        self
    }

    /// Calls `on_warning` with each warning Nickel reports. Warnings are discarded otherwise.
    pub fn on_warning(mut self, on_warning: impl FnMut(Diagnostic) + 'static) -> Self {
        self.warnings = WarningReporter(Box::new(on_warning));
        self
    }

    /// Makes `packages` importable by name (`import my_dependency`), like `nickel` does for the
    /// dependencies in a `Nickel-pkg.ncl` manifest. Calling it again merges the packages.
    #[cfg(feature = "package")]
    pub fn packages(mut self, packages: crate::Packages) -> Self {
        self.packages = Some(match self.packages.take() {
            Some(earlier) => earlier.merge(packages),
            None => packages,
        });
        self
    }

    /// Reads the sources and prepares the program.
    ///
    /// Fails if there are no sources or one can't be read. Parse and type errors are reported
    /// by the operations on [`Program`].
    pub fn build(self) -> Result<Program, Error> {
        let mut builder = CoreProgramBuilder::new()
            .with_trace(self.trace)
            .with_reporter(self.warnings)
            .add_import_paths(self.import_paths)
            .add_contract_paths(self.contract_files)
            .with_field_path(self.field.to_core())
            .add_overrides(self.overrides.into_iter().map(|(path, value, priority)| {
                FieldOverride {
                    path: path.to_core(),
                    value,
                    priority: priority.to_core(),
                }
            }));
        #[cfg(feature = "package")]
        if let Some(packages) = self.packages {
            builder = builder.with_package_map(packages.map);
        }
        for (i, source) in self.sources.into_iter().enumerate() {
            builder = source.add_to(builder, i);
        }
        let inner = builder.build::<CacheImpl>().map_err(builder_error)?;
        Ok(Program { inner })
    }
}

fn builder_error(e: BuilderError) -> Error {
    Error {
        error: Box::new(NickelCoreError::IOError(IOError(e.to_string()))),
        files: Files::empty(),
    }
}

/// A Nickel program built from one or more [`Source`]s, ready to evaluate, export, query or
/// typecheck. Each operation corresponds to a `nickel` subcommand.
///
/// A program keeps its sources and evaluation state, so operations can be repeated or combined,
/// and expressions from [`Self::eval_shallow`] evaluated further with
/// [`Self::eval_expr_shallow`]. Like [`crate::Context`], it is not `Send`.
pub struct Program {
    inner: CoreProgram<CacheImpl>,
}

impl Program {
    /// Starts configuring a program.
    pub fn builder() -> ProgramBuilder {
        ProgramBuilder::default()
    }

    /// Evaluates the program fully, like `nickel eval`.
    pub fn eval_deep(&mut self) -> Result<Expr, Error> {
        let result = self.inner.eval_full();
        self.wrap(result)
    }

    /// Evaluates fully, leaving out fields marked `not_exported`.
    pub fn eval_deep_for_export(&mut self) -> Result<Expr, Error> {
        let result = self.inner.eval_full_for_export();
        self.wrap(result)
    }

    /// Evaluates to weak head normal form: see [`crate::Context::eval_shallow`].
    pub fn eval_shallow(&mut self) -> Result<Expr, Error> {
        let result = self.inner.eval();
        self.wrap(result)
    }

    /// Evaluates the records of the program, but not the other values in it. Fields that can't
    /// be evaluated because something they need is missing are left unevaluated rather than
    /// failing, so this works on incomplete configurations.
    pub fn eval_record_spine(&mut self) -> Result<Expr, Error> {
        let result = self.inner.eval_record_spine();
        self.wrap(result)
    }

    /// Evaluates an expression from this program a step further: see
    /// [`crate::Context::eval_expr_shallow`].
    pub fn eval_expr_shallow(&mut self, expr: Expr) -> Result<Expr, Error> {
        let result = self
            .inner
            .eval_closure(Closure::from(expr.value))
            .map_err(NickelCoreError::from);
        self.wrap(result)
    }

    /// Evaluates the program and serializes it, like `nickel export`. Fields marked
    /// `not_exported` are left out.
    pub fn export(&mut self, format: ExportFormat) -> Result<String, Error> {
        let expr = self.eval_deep_for_export()?;
        serialize::validate(format.to_core(), &expr.value)
            .and_then(|()| serialize::to_string(format.to_core(), &expr.value))
            .map_err(|e| self.export_error(e))
    }

    /// Like [`Self::export`], writing to `out`.
    pub fn export_to(&mut self, format: ExportFormat, mut out: impl Write) -> Result<(), Error> {
        let text = self.export(format)?;
        out.write_all(text.as_bytes()).map_err(Error::from)
    }

    /// Looks up the focused field (see [`ProgramBuilder::field`]), or the whole program, and
    /// returns its metadata and value without evaluating it fully, like `nickel query`.
    pub fn query(&mut self) -> Result<FieldInfo, Error> {
        let result = self.inner.query();
        let field = self.wrap_core(result)?;
        Ok(FieldInfo { field })
    }

    /// Typechecks the program and any applied contracts, like `nickel typecheck`.
    pub fn typecheck(&mut self, mode: TypecheckMode) -> Result<(), Error> {
        let mode = match mode {
            TypecheckMode::Walk => CoreTypecheckMode::Walk,
            TypecheckMode::Enforce => CoreTypecheckMode::Enforce,
        };
        let result = self.inner.typecheck(mode);
        self.wrap_core(result)
    }

    /// Parses the program without typechecking or evaluating it. Undefined variables and
    /// type errors are not reported.
    pub fn parse(&mut self) -> Result<(), Error> {
        // Core's `Program::parse` panics after a successful parse in nickel-lang-core 0.19
        // (it reads the parsed term from a cache that only compilation fills), so this compiles
        // instead, which parses the same way and does no more checking. Switch back once
        // upstream fixes it.
        let result = self.inner.compile();
        self.wrap_core(result)
    }

    /// Extracts the documentation of the program's fields, like `nickel doc`.
    ///
    /// Fails if the program isn't a record.
    #[cfg(feature = "doc")]
    pub fn docs(&mut self) -> Result<Documentation, Error> {
        let result = self.inner.extract_doc();
        let extracted = self.wrap_core(result)?;
        // Core's documentation type isn't nameable from outside the crate, so render it now.
        let render = |result: Result<(), NickelCoreError>, out: Vec<u8>| {
            result
                .map(|()| String::from_utf8_lossy(&out).into_owned())
                .map_err(|error| Error {
                    error: Box::new(error),
                    files: Files::empty(),
                })
        };
        let mut json = Vec::new();
        let json = render(extracted.write_json(&mut json), json)?;
        let mut markdown = Vec::new();
        let markdown = render(extracted.write_markdown(&mut markdown), markdown)?;
        let entries = extracted
            .docstrings()
            .into_iter()
            .map(|(path, doc)| (path.into_iter().collect(), doc.to_owned()))
            .collect();
        Ok(Documentation {
            entries,
            json,
            markdown,
        })
    }

    fn wrap(
        &self,
        result: Result<nickel_lang_core::eval::value::NickelValue, NickelCoreError>,
    ) -> Result<Expr, Error> {
        self.wrap_core(result).map(|value| Expr { value })
    }

    fn wrap_core<T>(&self, result: Result<T, NickelCoreError>) -> Result<T, Error> {
        result.map_err(|error| Error {
            error: Box::new(error),
            files: self.inner.files(),
        })
    }

    fn export_error(&self, error: PointedExportErrorData) -> Error {
        Error {
            error: Box::new(error.with_pos_table(self.inner.pos_table().clone()).into()),
            files: self.inner.files(),
        }
    }
}

/// What [`Program::query`] found: a field's metadata, its value if it has one, and the names
/// of its sub-fields if the value is a record.
pub struct FieldInfo {
    field: CoreField,
}

impl FieldInfo {
    /// The field's metadata.
    pub fn metadata(&self) -> FieldMetadata<'_> {
        FieldMetadata {
            inner: self.field.metadata.0.as_deref(),
        }
    }

    /// The field's value, if it has one. It may be unevaluated.
    pub fn value(&self) -> Option<Expr> {
        self.field.value.clone().map(|value| Expr { value })
    }

    /// The names of the field's sub-fields, in source order, if its value is a record.
    pub fn subfields(&self) -> Vec<String> {
        self.value()
            .and_then(|v| v.as_record().map(|r| r.keys().map(str::to_owned).collect()))
            .unwrap_or_default()
    }
}

impl fmt::Debug for FieldInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.metadata();
        f.debug_struct("FieldInfo")
            .field("doc", &m.doc())
            .field("type_annotation", &m.type_annotation())
            .field("contracts", &m.contracts())
            .field("optional", &m.optional())
            .field("not_exported", &m.not_exported())
            .field("subfields", &self.subfields())
            .finish()
    }
}

/// Documentation extracted by [`Program::docs`].
#[cfg(feature = "doc")]
#[derive(Clone, Debug)]
pub struct Documentation {
    entries: Vec<(FieldPath, String)>,
    json: String,
    markdown: String,
}

#[cfg(feature = "doc")]
impl Documentation {
    /// Every documented field, with its path and documentation.
    pub fn entries(&self) -> &[(FieldPath, String)] {
        &self.entries
    }

    /// All fields with their types, contracts and documentation, as JSON, like
    /// `nickel doc --format json`.
    pub fn to_json(&self) -> &str {
        &self.json
    }

    /// As Markdown, like `nickel doc --format markdown`.
    pub fn to_markdown(&self) -> &str {
        &self.markdown
    }
}
