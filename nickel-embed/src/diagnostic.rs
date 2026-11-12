//! Structured errors and warnings: what went wrong and where, as plain data.

use std::ops::Range;

use codespan_reporting::{
    diagnostic::{self as cs, LabelStyle},
    files::Files as CodespanFiles,
};
use nickel_lang_core::{
    error::{
        Error as NickelCoreError, EvalErrorKind, IntoDiagnostics, Reporter, TypecheckErrorKind,
        warning::Warning,
    },
    files::{FileId, Files},
};

/// What kind of failure an [`Error`](crate::Error) is.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The source isn't valid syntax.
    Parse,
    /// An import couldn't be found or read.
    Import,
    /// A static type error.
    Typecheck,
    /// A value broke a contract.
    Contract,
    /// Any other evaluation failure, such as a missing field or a failed assertion.
    Eval,
    /// The value can't be serialized in the requested format.
    Export,
    /// Reading a source failed.
    Io,
    /// Package resolution failed: the manifest is invalid, a dependency can't be found or
    /// fetched, or versions conflict.
    Package,
    /// A failure with no more specific kind.
    Other,
}

impl ErrorKind {
    pub(crate) fn of(error: &NickelCoreError) -> Self {
        match error {
            NickelCoreError::ParseErrors(_) => Self::Parse,
            NickelCoreError::ImportError(_) => Self::Import,
            // The typechecker resolves imports, so a missing import can surface as a type error.
            NickelCoreError::TypecheckError(data) => match data.borrow_error() {
                TypecheckErrorKind::ImportError(_) => Self::Import,
                _ => Self::Typecheck,
            },
            NickelCoreError::EvalError(data) => match data.error {
                EvalErrorKind::BlameError { .. } => Self::Contract,
                _ => Self::Eval,
            },
            NickelCoreError::ExportError(_) => Self::Export,
            NickelCoreError::IOError(_) => Self::Io,
            _ => Self::Other,
        }
    }
}

/// How serious a [`Diagnostic`] is.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Help,
    Note,
    Warning,
    Error,
    /// An internal error in Nickel itself.
    Bug,
}

/// One message about a problem, with the places in the source it points at.
///
/// An [`Error`](crate::Error) can carry several, for example one per parse error.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A short code identifying the problem, when Nickel gives one.
    pub code: Option<String>,
    pub message: String,
    /// The places in the source this points at. The primary labels say where the problem is;
    /// secondary ones give context.
    pub labels: Vec<Label>,
    /// Extra explanation, not tied to a place in the source.
    pub notes: Vec<String>,
}

/// A place in a source that a [`Diagnostic`] points at.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Label {
    /// Whether this is where the problem is (`true`) or context for it.
    pub primary: bool,
    /// The source's name: a file path, or the name given to in-memory code.
    pub source: String,
    /// The byte range in the source.
    pub range: Range<usize>,
    /// Where the range starts, 1-based.
    pub start: Location,
    /// Where the range ends, 1-based.
    pub end: Location,
    pub message: String,
}

/// A line and column in a source, both counted from 1.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Diagnostic {
    pub(crate) fn from_codespan(d: cs::Diagnostic<FileId>, files: &Files) -> Self {
        let severity = match d.severity {
            cs::Severity::Help => Severity::Help,
            cs::Severity::Note => Severity::Note,
            cs::Severity::Warning => Severity::Warning,
            cs::Severity::Error => Severity::Error,
            cs::Severity::Bug => Severity::Bug,
        };
        let labels = d
            .labels
            .into_iter()
            .map(|l| {
                let location = |byte| {
                    CodespanFiles::location(files, l.file_id, byte)
                        .map(|loc| Location {
                            line: loc.line_number,
                            column: loc.column_number,
                        })
                        .unwrap_or(Location { line: 0, column: 0 })
                };
                Label {
                    primary: l.style == LabelStyle::Primary,
                    source: CodespanFiles::name(files, l.file_id).unwrap_or_default(),
                    start: location(l.range.start),
                    end: location(l.range.end),
                    range: l.range,
                    message: l.message,
                }
            })
            .collect();
        Diagnostic {
            severity,
            code: d.code,
            message: d.message,
            labels,
            notes: d.notes,
        }
    }

    pub(crate) fn from_error(error: NickelCoreError, files: &Files) -> Vec<Self> {
        let mut files = files.clone();
        error
            .into_diagnostics(&mut files)
            .into_iter()
            .map(|d| Diagnostic::from_codespan(d, &files))
            .collect()
    }
}

/// Passes Nickel's warnings to a callback as [`Diagnostic`]s.
pub(crate) struct WarningReporter(pub(crate) Box<dyn FnMut(Diagnostic)>);

impl WarningReporter {
    pub(crate) fn ignore() -> Self {
        Self(Box::new(|_| {}))
    }
}

impl Reporter<(Warning, Files)> for WarningReporter {
    fn report(&mut self, (warning, mut files): (Warning, Files)) {
        for d in warning.into_diagnostics(&mut files) {
            (self.0)(Diagnostic::from_codespan(d, &files));
        }
    }
}
