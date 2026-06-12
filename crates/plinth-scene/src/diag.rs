//! Diagnostics: errors rendered for humans and agents alike.

use std::fmt;

/// What stage of validation produced a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagKind {
    /// The file is not valid JSON, or does not match the scene types
    /// (unknown fields, wrong types, missing required fields).
    Parse,
    /// The file parsed, but violates a semantic rule (duplicate ids,
    /// out-of-range values, components that require other components).
    Semantic,
}

impl fmt::Display for DiagKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagKind::Parse => write!(f, "parse"),
            DiagKind::Semantic => write!(f, "semantic"),
        }
    }
}

/// One validation finding, locatable enough for an agent to fix it
/// without guessing: a path into the document plus line/column when known.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub kind: DiagKind,
    /// Path into the document, e.g. `entities[2].components.shape`.
    /// Empty when the error is document-wide.
    pub location: String,
    /// 1-based line in the source file, when known.
    pub line: Option<usize>,
    /// 1-based column in the source file, when known.
    pub column: Option<usize>,
    pub message: String,
}

impl Diagnostic {
    pub fn semantic(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: DiagKind::Semantic,
            location: location.into(),
            line: None,
            column: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]", self.kind)?;
        if !self.location.is_empty() {
            write!(f, " {}", self.location)?;
        }
        write!(f, ": {}", self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {line}")?;
            if let Some(col) = self.column {
                write!(f, ", column {col}")?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}
