use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelatedDiagnostic {
    pub(crate) span: Span,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) code: String,
    pub(crate) severity: Severity,
    pub(crate) message: String,
    pub(crate) primary: Span,
    pub(crate) related: Vec<RelatedDiagnostic>,
}

impl Diagnostic {
    pub(crate) fn error(code: &str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            code: code.to_owned(),
            severity: Severity::Error,
            message: message.into(),
            primary,
            related: Vec::new(),
        }
    }
}

pub(crate) fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.primary
            .source_id
            .cmp(&right.primary.source_id)
            .then_with(|| left.primary.start.cmp(&right.primary.start))
            .then_with(|| left.primary.end.cmp(&right.primary.end))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
}
