// Compiler library; source loading stays independent of filesystem access.

mod analyzer;
mod diagnostics;
#[cfg(test)]
mod interpreter;
mod ir;
mod js_backend;
mod lexer;
mod parser;
mod source;
mod span;

use std::collections::BTreeMap;

use analyzer::Analysis;
use diagnostics::sort_diagnostics;
use parser::Module;
use source::SourceSet;

pub use diagnostics::{Diagnostic, RelatedDiagnostic, Severity};
pub use source::SourceError;
pub use span::Span;

pub const MAX_SOURCE_BYTES: usize = source::MAX_SOURCE_BYTES;
pub const MAX_PROJECT_BYTES: usize = source::MAX_PROJECT_BYTES;
pub const MAX_MODULES: usize = source::MAX_MODULES;

/// Filesystem-independent entry point for checking and compiling supplied sources.
#[derive(Debug, Default)]
pub struct Compiler {
    sources: SourceSet,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(
        &mut self,
        id: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), SourceError> {
        let id = id.into();
        self.sources
            .insert(source::SourceFile::new(&id, bytes.into())?)
    }

    pub fn contains_source(&self, id: &str) -> bool {
        self.sources.get(id).is_some()
    }

    pub fn source_revisions(&self) -> Vec<SourceRevision> {
        self.sources
            .iter()
            .map(|source| SourceRevision {
                id: source.id().to_owned(),
                revision: source.revision().to_owned(),
            })
            .collect()
    }

    pub fn check(&self) -> CheckResult {
        let analysis = analyzer::analyze(&self.sources);
        check_result(&analysis)
    }

    pub fn build(&self) -> BuildResult {
        let analysis = analyzer::analyze(&self.sources);
        let checked = check_result(&analysis);
        if checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return BuildResult {
                diagnostics: checked.diagnostics,
                imports: checked.imports,
                javascript: None,
            };
        }

        let mut diagnostics = checked.diagnostics.clone();
        let javascript = match ir::lower(&analysis).and_then(|program| js_backend::emit(&program)) {
            Ok(files) => Some(files),
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                sort_diagnostics(&mut diagnostics);
                None
            }
        };
        BuildResult {
            diagnostics,
            imports: checked.imports,
            javascript,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRevision {
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRequest {
    pub importer_id: String,
    pub path: String,
    pub target_id: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub imports: Vec<ImportRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildResult {
    pub diagnostics: Vec<Diagnostic>,
    pub imports: Vec<ImportRequest>,
    pub javascript: Option<BTreeMap<String, String>>,
}

fn check_result(analysis: &Analysis) -> CheckResult {
    let imports = analysis
        .modules
        .iter()
        .flat_map(|(importer_id, module): (&String, &Module)| {
            module.imports.iter().map(move |import| ImportRequest {
                importer_id: importer_id.clone(),
                path: import.path.clone(),
                target_id: source::resolve_import_id(importer_id, &import.path).ok(),
                span: import.path_span.clone(),
            })
        })
        .collect();
    CheckResult {
        diagnostics: analysis.diagnostics.clone(),
        imports,
    }
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod lexer_tests;

#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;

#[cfg(test)]
#[path = "parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod analysis_tests;

#[cfg(test)]
#[path = "ir_tests.rs"]
mod ir_tests;

#[cfg(test)]
#[path = "interpreter_tests.rs"]
mod interpreter_tests;

#[cfg(test)]
#[path = "js_backend_tests.rs"]
mod js_backend_tests;

#[cfg(test)]
#[path = "../benchmarks/m1/grade.rs"]
mod benchmark_grade;

#[cfg(test)]
#[path = "records_tests.rs"]
mod records_tests;
