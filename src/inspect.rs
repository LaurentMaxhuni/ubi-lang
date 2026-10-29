//! Checked public interfaces; no execution or JavaScript lowering.

use serde::Serialize;

use crate::analyzer::{self, Analysis, TypeInfo, ValueType};
use crate::lexer::CommentKind;
use crate::parser::{Declaration, Module, TypeParameter};
use crate::source::{resolve_import_id, SourceSet};
use crate::{Diagnostic, Severity, Span};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInspection {
    pub schema_version: u32,
    pub offset_unit: &'static str,
    pub modules: Vec<ModuleSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleSummary {
    pub id: String,
    pub revision: String,
    pub imports: Vec<ImportSummary>,
    pub exports: Vec<ExportSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub names: Vec<String>,
    pub path: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub name: String,
    pub documentation: String,
    pub span: Span,
    pub type_parameters: Vec<TypeParameterSummary>,
    #[serde(flatten)]
    pub declaration: DeclarationSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeParameterSummary {
    pub name: String,
    pub bound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParameterSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantSummary {
    pub name: String,
    pub payloads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DeclarationSummary {
    Function {
        parameters: Vec<ParameterSummary>,
        #[serde(rename = "returnType")]
        return_type: String,
        #[serde(rename = "async")]
        asynchronous: bool,
        effects: Vec<String>,
    },
    Record {
        fields: Vec<ParameterSummary>,
    },
    Enum {
        variants: Vec<VariantSummary>,
    },
}

pub(crate) fn inspect(
    analysis: &Analysis,
    sources: &SourceSet,
) -> Result<ModuleInspection, Vec<Diagnostic>> {
    if analysis
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return Err(analysis.diagnostics.clone());
    }
    let mut modules = Vec::new();
    for (id, module) in &analysis.modules {
        let source = sources.get(id).expect("analyzed source exists");
        let text = source.text().expect("checked source is UTF-8");
        let mut exports = Vec::new();
        for declaration in &module.declarations {
            let (name, span, type_parameters, declaration) = match declaration {
                Declaration::Function(function) if function.exported => {
                    let signature = &analysis.signatures[&(id.clone(), function.name.name.clone())];
                    let mut effects: Vec<_> =
                        function.effects.iter().map(|e| e.name.clone()).collect();
                    effects.sort();
                    let parameters = function
                        .parameters
                        .iter()
                        .zip(&signature.parameters)
                        .map(|(parameter, ty)| {
                            Ok(ParameterSummary {
                                name: parameter.name.name.clone(),
                                ty: type_name(analysis, *ty, &parameter.span)?,
                            })
                        })
                        .collect::<Result<_, Vec<Diagnostic>>>()?;
                    (
                        &function.name.name,
                        &function.span,
                        &function.type_parameters,
                        DeclarationSummary::Function {
                            parameters,
                            return_type: type_name(
                                analysis,
                                signature.return_type.expect("checked return type"),
                                &function.span,
                            )?,
                            asynchronous: signature.asynchronous,
                            effects,
                        },
                    )
                }
                Declaration::Record(record) if record.exported => {
                    let info = &analysis.records
                        [analysis.record_keys[&(id.clone(), record.name.name.clone())]];
                    let fields = record
                        .fields
                        .iter()
                        .map(|field| {
                            Ok(ParameterSummary {
                                name: field.name.name.clone(),
                                ty: type_name(
                                    analysis,
                                    info.fields[&field.name.name],
                                    &field.span,
                                )?,
                            })
                        })
                        .collect::<Result<_, Vec<Diagnostic>>>()?;
                    (
                        &record.name.name,
                        &record.span,
                        &record.type_parameters,
                        DeclarationSummary::Record { fields },
                    )
                }
                Declaration::Enum(enumeration) if enumeration.exported => {
                    let key = (id.clone(), enumeration.name.name.clone());
                    let index = analyzer::enum_type(&analysis.types, &key).expect("checked enum");
                    let TypeInfo::Enum { variants, .. } = &analysis.types[index] else {
                        unreachable!()
                    };
                    let variants = enumeration
                        .variants
                        .iter()
                        .map(|variant| {
                            Ok(VariantSummary {
                                name: variant.name.name.clone(),
                                payloads: variants[&variant.name.name]
                                    .iter()
                                    .map(|ty| type_name(analysis, *ty, &enumeration.span))
                                    .collect::<Result<_, _>>()?,
                            })
                        })
                        .collect::<Result<_, Vec<Diagnostic>>>()?;
                    (
                        &enumeration.name.name,
                        &enumeration.span,
                        &enumeration.type_parameters,
                        DeclarationSummary::Enum { variants },
                    )
                }
                _ => continue,
            };
            exports.push(ExportSummary {
                name: name.clone(),
                documentation: documentation(module, text, span.start),
                span: span.clone(),
                type_parameters: type_parameters.iter().map(type_parameter).collect(),
                declaration,
            });
        }
        exports.sort_by(|a, b| a.name.cmp(&b.name));
        modules.push(ModuleSummary {
            id: id.clone(),
            revision: source.revision().to_owned(),
            imports: module
                .imports
                .iter()
                .map(|import| ImportSummary {
                    names: import.names.iter().map(|name| name.name.clone()).collect(),
                    path: import.path.clone(),
                    target_id: resolve_import_id(id, &import.path).expect("checked import"),
                })
                .collect(),
            exports,
        });
    }
    Ok(ModuleInspection {
        schema_version: 1,
        offset_unit: "utf8-byte",
        modules,
    })
}

fn type_parameter(parameter: &TypeParameter) -> TypeParameterSummary {
    TypeParameterSummary {
        name: parameter.name.clone(),
        bound: parameter.bound.as_ref().map(|bound| bound.name.clone()),
    }
}

fn documentation(module: &Module, source: &str, start: usize) -> String {
    let mut next = start;
    let mut lines = Vec::new();
    for comment in module
        .comments
        .iter()
        .rev()
        .filter(|comment| comment.span.end <= start)
    {
        let gap = &source[comment.span.end..next];
        let newlines = gap.replace("\r\n", "\n").replace('\r', "\n");
        let line_prefix = source[..comment.span.start]
            .rsplit(['\r', '\n'])
            .next()
            .unwrap();
        if comment.kind != CommentKind::Line
            || !gap.chars().all(char::is_whitespace)
            || newlines.bytes().filter(|byte| *byte == b'\n').count() != 1
            || !line_prefix.chars().all(char::is_whitespace)
        {
            break;
        }
        let Some(line) = source[comment.span.start..comment.span.end].strip_prefix("///") else {
            break;
        };
        lines.push(line.strip_prefix(' ').unwrap_or(line));
        next = comment.span.start;
    }
    lines.reverse();
    lines.join("\n")
}

fn type_name(analysis: &Analysis, ty: ValueType, span: &Span) -> Result<String, Vec<Diagnostic>> {
    let mut output = String::new();
    write_type(analysis, ty, &mut output).map_err(|()| {
        vec![Diagnostic::error(
            "UBI0090",
            "Summary type exceeds 65536 UTF-8 bytes",
            span.clone(),
        )]
    })?;
    Ok(output)
}

fn append(output: &mut String, text: &str) -> Result<(), ()> {
    if text.len() > 65_536 - output.len() {
        return Err(());
    }
    output.push_str(text);
    Ok(())
}

fn write_types(analysis: &Analysis, types: &[ValueType], output: &mut String) -> Result<(), ()> {
    for (index, ty) in types.iter().enumerate() {
        if index != 0 {
            append(output, ", ")?;
        }
        write_type(analysis, *ty, output)?;
    }
    Ok(())
}

fn write_nominal(
    analysis: &Analysis,
    key: &(String, String),
    arguments: &[ValueType],
    output: &mut String,
) -> Result<(), ()> {
    if !key.0.is_empty() {
        append(output, &key.0)?;
        append(output, "::")?;
    }
    append(output, &key.1)?;
    if !arguments.is_empty() {
        append(output, "<")?;
        write_types(analysis, arguments, output)?;
        append(output, ">")?;
    }
    Ok(())
}

fn write_type(analysis: &Analysis, ty: ValueType, output: &mut String) -> Result<(), ()> {
    if let ValueType::Record(id) = ty {
        let record = &analysis.records[id];
        return write_nominal(analysis, &record.key, &record.arguments, output);
    }
    let info = match ty {
        ValueType::Tuple(id)
        | ValueType::List(id)
        | ValueType::Option(id)
        | ValueType::Map(id)
        | ValueType::Enum(id)
        | ValueType::Function(id)
        | ValueType::Parameter(id) => &analysis.types[id],
        _ => return append(output, ty.name()),
    };
    match info {
        TypeInfo::Parameter { name, .. } => append(output, name),
        TypeInfo::List(inner) | TypeInfo::Option(inner) => {
            append(output, ty.name())?;
            append(output, "<")?;
            write_type(analysis, *inner, output)?;
            append(output, ">")
        }
        TypeInfo::Tuple(elements) => {
            append(output, "(")?;
            write_types(analysis, elements, output)?;
            if elements.len() == 1 {
                append(output, ",")?;
            }
            append(output, ")")
        }
        TypeInfo::Map { key, value } => {
            append(output, "Map<")?;
            write_types(analysis, &[*key, *value], output)?;
            append(output, ">")
        }
        TypeInfo::Enum { key, arguments, .. } => write_nominal(analysis, key, arguments, output),
        TypeInfo::Function {
            parameters,
            return_type,
        } => {
            append(output, "fn(")?;
            write_types(analysis, parameters, output)?;
            append(output, ") -> ")?;
            write_type(analysis, *return_type, output)
        }
    }
}
