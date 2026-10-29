//! Pure preparation of checked, revision-bound function-body replacements.

use serde::{Deserialize, Serialize};

use crate::parser::{self, Declaration};
use crate::{Compiler, Diagnostic, Severity, SourceError, SourceRevision, MAX_SOURCE_BYTES};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FunctionBodyEdit {
    pub schema_version: u32,
    pub source_id: String,
    pub function: String,
    pub source_revision: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedBodyEdit {
    pub source_id: String,
    pub function: String,
    pub source_revision: String,
    pub resulting_revision: String,
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub diff: String,
    #[serde(skip)]
    original_source: String,
    #[serde(skip)]
    updated_source: String,
}

impl PreparedBodyEdit {
    pub fn original_source(&self) -> &str {
        &self.original_source
    }

    pub fn updated_source(&self) -> &str {
        &self.updated_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    Request(String),
    Diagnostics {
        sources: Vec<SourceRevision>,
        diagnostics: Vec<Diagnostic>,
    },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(message) => formatter.write_str(message),
            Self::Diagnostics { diagnostics, .. } => {
                write!(
                    formatter,
                    "edit rejected by {} compiler diagnostic(s)",
                    diagnostics.len()
                )
            }
        }
    }
}

impl std::error::Error for EditError {}

pub(crate) fn prepare(
    compiler: &Compiler,
    request: &FunctionBodyEdit,
) -> Result<PreparedBodyEdit, EditError> {
    let invalid = |message: &str| EditError::Request(message.to_owned());
    if request.schema_version != 1 {
        return Err(invalid("edit schemaVersion must be 1"));
    }
    if !request
        .source_revision
        .strip_prefix("sha256:")
        .is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
    {
        return Err(invalid(
            "sourceRevision must be sha256: followed by 64 lowercase hex digits",
        ));
    }
    let source = compiler
        .sources
        .get(&request.source_id)
        .ok_or_else(|| invalid("edit sourceId is not in the loaded source graph"))?;
    if source.revision() != request.source_revision {
        return Err(invalid(
            "stale sourceRevision; inspect the current source before editing",
        ));
    }
    let original = source.text().map_err(|error| EditError::Diagnostics {
        sources: compiler.source_revisions(),
        diagnostics: vec![Diagnostic::error(&error.code, error.message, error.primary)],
    })?;
    let module = parser::parse(source.id(), original).map_err(|error| EditError::Diagnostics {
        sources: compiler.source_revisions(),
        diagnostics: vec![Diagnostic::error(&error.code, error.message, error.primary)],
    })?;
    let mut functions = module
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Function(function) if function.name.name == request.function => {
                Some(function)
            }
            _ => None,
        });
    let function = functions
        .next()
        .ok_or_else(|| invalid("edit target is not a top-level function"))?;
    if functions.next().is_some() {
        return Err(invalid("edit target function name is ambiguous"));
    }
    if request.body.len() > MAX_SOURCE_BYTES {
        return Err(invalid(
            "UBI0090: replacement body exceeds the source byte limit",
        ));
    }
    let body = request.body.trim();
    let start = function.body.span.start;
    let end = function.body.span.end;
    let new_len = original.len() - (end - start) + body.len();
    if new_len > MAX_SOURCE_BYTES {
        return Err(invalid(
            "UBI0090: edited source exceeds the source byte limit",
        ));
    }
    // Parsing a minimal function isolates the replacement block from the original
    // declaration. Checking the span also excludes trailing comments or tokens.
    const PREFIX: &str = "fn f()";
    let wrapper = format!("{PREFIX}{body}");
    let parsed_body = parser::parse(source.id(), &wrapper);
    let resource_limited = matches!(&parsed_body, Err(error) if error.code == "UBI0090");
    let valid_block = body.starts_with('{') && body.ends_with('}')
        && parsed_body.as_ref().is_ok_and(|module| {
            matches!(module.declarations.as_slice(), [Declaration::Function(function)]
                if function.body.span.start == PREFIX.len() && function.body.span.end == wrapper.len())
        });
    if !valid_block && !resource_limited {
        return Err(invalid("body must be exactly one brace-delimited block without outside comments or declarations"));
    }
    let mut updated = String::with_capacity(new_len);
    updated.push_str(&original[..start]);
    updated.push_str(body);
    updated.push_str(&original[end..]);
    let mut candidate = Compiler::new();
    for file in compiler.sources.iter() {
        let bytes = if file.id() == request.source_id {
            updated.as_bytes().to_vec()
        } else {
            // Existing source bytes may be malformed; retain them so the ordinary
            // compiler reports errors instead of silently dropping other modules.
            file.bytes().to_vec()
        };
        candidate
            .add_source(file.id(), bytes)
            .map_err(|error| match error {
                SourceError::Diagnostic(diagnostic) => {
                    EditError::Request(format!("{}: {}", diagnostic.code, diagnostic.message))
                }
                error => EditError::Request(format!("cannot prepare edited sources: {error:?}")),
            })?;
    }
    let check = candidate.check();
    if check
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Err(EditError::Diagnostics {
            sources: candidate.source_revisions(),
            diagnostics: check.diagnostics,
        });
    }
    // Resource diagnostics must use candidate offsets and revisions, rather
    // than the synthetic wrapper. Never accept a block we could not validate.
    if !valid_block {
        return Err(invalid("body could not be validated as exactly one block"));
    }
    let resulting_revision = candidate
        .sources
        .get(source.id())
        .expect("edited source exists")
        .revision()
        .to_owned();
    Ok(PreparedBodyEdit {
        source_id: request.source_id.clone(),
        function: request.function.clone(),
        source_revision: request.source_revision.clone(),
        resulting_revision,
        start,
        end,
        replacement: body.to_owned(),
        diff: unified_diff(source.id(), original, &updated),
        original_source: original.to_owned(),
        updated_source: updated,
    })
}

fn unified_diff(id: &str, original: &str, updated: &str) -> String {
    if original == updated {
        return String::new();
    }
    let old: Vec<_> = original.split_inclusive('\n').collect();
    let new: Vec<_> = updated.split_inclusive('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let old_changed_end = old.len() - suffix;
    let new_changed_end = new.len() - suffix;
    let old_end = old_changed_end + suffix.min(3);
    let new_end = new_changed_end + suffix.min(3);
    let mut diff = format!(
        "--- a/{id}\n+++ b/{id}\n@@ -{},{} +{},{} @@\n",
        start + 1,
        old_end - start,
        start + 1,
        new_end - start
    );
    for line in &old[start..prefix] {
        diff_line(&mut diff, ' ', line);
    }
    for line in &old[prefix..old_changed_end] {
        diff_line(&mut diff, '-', line);
    }
    for line in &new[prefix..new_changed_end] {
        diff_line(&mut diff, '+', line);
    }
    for line in &old[old_changed_end..old_end] {
        diff_line(&mut diff, ' ', line);
    }
    diff
}

fn diff_line(diff: &mut String, prefix: char, line: &str) {
    diff.push(prefix);
    diff.push_str(line);
    if !line.ends_with('\n') {
        diff.push_str("\n\\ No newline at end of file\n");
    }
}
