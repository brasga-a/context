use crate::Span;

/// A non-fatal problem found while parsing a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Span,
    pub kind: DiagnosticKind,
}

/// The kinds of diagnostics emitted by the parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticKind {
    UnterminatedFrontmatter,
    UnclosedCodeFence,
}
