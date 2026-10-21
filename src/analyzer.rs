use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::diagnostics::{sort_diagnostics, Diagnostic, RelatedDiagnostic};
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Function, Identifier, MatchArm, Module, Parameter,
    ParseError, Pattern, PatternKind, StatementKind, Type, TypeParameter,
};
use crate::source::{resolve_import_id, SourceSet};
use crate::span::Span;

pub(crate) type FunctionKey = (String, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapabilityKind {
    Console,
    Storage,
    Http,
}

impl CapabilityKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Storage => "storage",
            Self::Http => "http",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ValueType {
    Int,
    Float,
    Bool,
    String,
    Unit,
    Tuple(usize),
    Record(usize),
    List(usize),
    Option(usize),
    Map(usize),
    Capability(CapabilityKind),
    Enum(usize),
    Function(usize),
    Parameter(usize),
    Range,
    Never,
    Error,
}

impl ValueType {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::String => "string",
            Self::Unit => "unit",
            Self::Tuple(_) => "tuple",
            Self::Record(_) => "record",
            Self::List(_) => "List",
            Self::Option(_) => "Option",
            Self::Map(_) => "Map",
            Self::Capability(CapabilityKind::Console) => "Console",
            Self::Capability(CapabilityKind::Storage) => "Storage",
            Self::Capability(CapabilityKind::Http) => "Http",
            Self::Enum(_) => "enum",
            Self::Function(_) => "function",
            Self::Parameter(_) => "type parameter",
            Self::Range => "range",
            Self::Never => "never",
            Self::Error => "<error>",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeInfo {
    Parameter {
        key: FunctionKey,
        name: String,
        equatable: bool,
    },
    List(ValueType),
    Tuple(Vec<ValueType>),
    Option(ValueType),
    Map {
        key: ValueType,
        value: ValueType,
    },
    Enum {
        key: FunctionKey,
        exported: bool,
        arguments: Vec<ValueType>,
        variants: BTreeMap<String, Vec<ValueType>>,
    },
    Function {
        parameters: Vec<ValueType>,
        return_type: ValueType,
    },
}

fn intern_type(types: &mut Vec<TypeInfo>, info: TypeInfo) -> ValueType {
    let id = types.iter().position(|ty| *ty == info).unwrap_or_else(|| {
        types.push(info.clone());
        types.len() - 1
    });
    match info {
        TypeInfo::Parameter { .. } => ValueType::Parameter(id),
        TypeInfo::List(_) => ValueType::List(id),
        TypeInfo::Tuple(_) => ValueType::Tuple(id),
        TypeInfo::Option(_) => ValueType::Option(id),
        TypeInfo::Map { .. } => ValueType::Map(id),
        TypeInfo::Enum { .. } => ValueType::Enum(id),
        TypeInfo::Function { .. } => ValueType::Function(id),
    }
}

pub(crate) fn builtin_name(name: &str) -> bool {
    matches!(
        name,
        "range"
            | "mapEmpty"
            | "mapSet"
            | "mapGet"
            | "mapHas"
            | "mapRemove"
            | "mapKeys"
            | "mapValues"
            | "consoleLog"
            | "storageGet"
            | "storageSet"
            | "storageRemove"
            | "storageGetAsync"
            | "storageSetAsync"
            | "storageRemoveAsync"
            | "httpGet"
            | "length"
            | "append"
            | "set"
            | "map"
            | "filter"
            | "find"
            | "fold"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "trim"
            | "split"
            | "join"
            | "replace"
            | "slice"
            | "abs"
            | "min"
            | "max"
            | "clamp"
            | "floor"
            | "ceil"
            | "round"
            | "sqrt"
            | "sin"
            | "cos"
            | "tan"
            | "log"
            | "exp"
            | "pow"
            | "toFloat"
            | "toInt"
            | "parseInt"
            | "parseFloat"
            | "toString"
    )
}

#[derive(Debug, Clone)]
pub(crate) struct Signature {
    pub(crate) asynchronous: bool,
    pub(crate) type_parameters: Vec<ValueType>,
    pub(crate) parameters: Vec<ValueType>,
    pub(crate) return_type: Option<ValueType>,
    name_span: Span,
}

#[derive(Debug, Clone)]
struct ModuleEdges {
    from: String,
    to: String,
    span: Span,
}

pub(crate) struct Analysis {
    pub(crate) types: Vec<TypeInfo>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) modules: BTreeMap<String, Module>,
    pub(crate) module_symbols: BTreeMap<String, BTreeMap<String, FunctionKey>>,
    pub(crate) signatures: BTreeMap<FunctionKey, Signature>,
    pub(crate) expression_types: BTreeMap<(FunctionKey, Span), ValueType>,
    pub(crate) reference_targets: BTreeMap<(FunctionKey, Span), FunctionKey>,
    pub(crate) records: Vec<RecordInfo>,
    pub(crate) record_keys: BTreeMap<FunctionKey, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordInfo {
    pub(crate) key: FunctionKey,
    pub(crate) exported: bool,
    pub(crate) arguments: Vec<ValueType>,
    pub(crate) fields: BTreeMap<String, ValueType>,
}

pub(crate) fn contains_function(ty: ValueType, records: &[RecordInfo], types: &[TypeInfo]) -> bool {
    let mut pending = vec![ty];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match ty {
            ValueType::Tuple(id) => {
                if let TypeInfo::Tuple(elements) = &types[id] {
                    pending.extend(elements.iter().copied());
                }
            }
            ValueType::Function(_) => return true,
            ValueType::Record(id) => {
                pending.extend(records[id].arguments.iter().copied());
                pending.extend(records[id].fields.values().copied());
            }
            ValueType::Enum(id) => {
                if let TypeInfo::Enum {
                    arguments,
                    variants,
                    ..
                } = &types[id]
                {
                    pending.extend(arguments.iter().copied());
                    pending.extend(variants.values().flatten().copied());
                }
            }
            ValueType::List(id) | ValueType::Option(id) => {
                if let TypeInfo::List(inner) | TypeInfo::Option(inner) = types[id] {
                    pending.push(inner);
                }
            }
            ValueType::Map(id) => {
                if let TypeInfo::Map { key, value } = types[id] {
                    pending.extend([key, value]);
                }
            }
            _ => {}
        }
    }
    false
}

fn is_equatable(ty: ValueType, records: &[RecordInfo], types: &[TypeInfo]) -> bool {
    let mut pending = vec![ty];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match ty {
            ValueType::Parameter(id) => {
                if !matches!(
                    types[id],
                    TypeInfo::Parameter {
                        equatable: true,
                        ..
                    }
                ) {
                    return false;
                }
            }
            ValueType::Function(_) | ValueType::Range | ValueType::Capability(_) => return false,
            ValueType::Tuple(id) => {
                if let TypeInfo::Tuple(elements) = &types[id] {
                    pending.extend(elements.iter().copied());
                }
            }
            ValueType::List(id) | ValueType::Option(id) => match types[id] {
                TypeInfo::List(inner) | TypeInfo::Option(inner) => pending.push(inner),
                _ => unreachable!(),
            },
            ValueType::Map(id) => {
                if let TypeInfo::Map { key, value } = types[id] {
                    pending.extend([key, value]);
                }
            }
            ValueType::Record(id) => {
                pending.extend(records[id].arguments.iter().copied());
                pending.extend(records[id].fields.values().copied());
            }
            ValueType::Enum(id) => {
                let TypeInfo::Enum {
                    arguments,
                    variants,
                    ..
                } = &types[id]
                else {
                    unreachable!()
                };
                pending.extend(arguments.iter().copied());
                pending.extend(variants.values().flatten().copied());
            }
            ValueType::Error
            | ValueType::Int
            | ValueType::Float
            | ValueType::Bool
            | ValueType::String
            | ValueType::Unit
            | ValueType::Never => {}
        }
    }
    true
}

#[cfg(test)]
pub(crate) fn check(sources: &SourceSet) -> Vec<Diagnostic> {
    analyze(sources).diagnostics
}

pub(crate) fn analyze(sources: &SourceSet) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut modules = BTreeMap::<String, Module>::new();

    for file in sources.iter() {
        let text = match file.text() {
            Ok(text) => text,
            Err(error) => {
                diagnostics.push(lexical_diagnostic(error));
                continue;
            }
        };
        match crate::parser::parse(file.id(), text) {
            Ok(module) => {
                modules.insert(file.id().to_owned(), module);
            }
            Err(error) => diagnostics.push(parse_diagnostic(error)),
        }
    }
    if !diagnostics.is_empty() {
        sort_diagnostics(&mut diagnostics);
        return Analysis {
            types: Vec::new(),
            diagnostics,
            modules,
            module_symbols: BTreeMap::new(),
            signatures: BTreeMap::new(),
            expression_types: BTreeMap::new(),
            reference_targets: BTreeMap::new(),
            records: Vec::new(),
            record_keys: BTreeMap::new(),
        };
    }

    let mut module_symbols = BTreeMap::new();
    let mut module_edges = Vec::new();
    resolve_imports(
        sources,
        &modules,
        &mut module_symbols,
        &mut module_edges,
        &mut diagnostics,
    );
    report_import_cycles(modules.keys(), &module_edges, &mut diagnostics);
    if !diagnostics.is_empty() {
        sort_diagnostics(&mut diagnostics);
        return Analysis {
            types: Vec::new(),
            diagnostics,
            modules,
            module_symbols,
            signatures: BTreeMap::new(),
            expression_types: BTreeMap::new(),
            reference_targets: BTreeMap::new(),
            records: Vec::new(),
            record_keys: BTreeMap::new(),
        };
    }

    let mut types = Vec::new();
    let (mut records, record_keys) =
        collect_records(&modules, &mut module_symbols, &mut types, &mut diagnostics);
    let (functions, mut signatures) = collect_signatures(
        &modules,
        &module_symbols,
        &mut records,
        &record_keys,
        &mut types,
        &mut diagnostics,
    );
    let call_graph = build_call_graph(&functions, &module_symbols);
    let components = strongly_connected_components(&call_graph);
    let mut expression_types = BTreeMap::new();
    let mut reference_targets = BTreeMap::new();

    for component in components.into_iter().rev() {
        let recursive = component.len() > 1
            || component
                .first()
                .is_some_and(|key| call_graph.get(key).is_some_and(|edges| edges.contains(key)));
        for key in component {
            let Some(function) = functions.get(&key).copied() else {
                continue;
            };
            let Some(signature) = signatures.get(&key).cloned() else {
                continue;
            };
            if recursive && signature.return_type.is_none() {
                diagnostics.push(Diagnostic::error(
                    "UBI0021",
                    "Recursive functions require an explicit return annotation",
                    signature.name_span.clone(),
                ));
            }

            let (inferred_return, function_types, references, mut function_diagnostics) =
                check_function(
                    &key,
                    function,
                    &signature,
                    &signatures,
                    &module_symbols,
                    &mut records,
                    &record_keys,
                    &mut types,
                );
            reference_targets.extend(
                references
                    .into_iter()
                    .map(|(span, target)| ((key.clone(), span), target)),
            );
            diagnostics.append(&mut function_diagnostics);
            expression_types.extend(
                function_types
                    .into_iter()
                    .map(|(span, ty)| ((key.clone(), span), ty)),
            );
            if signature.return_type.is_none() {
                if let Some(return_type) = inferred_return.filter(|ty| *ty != ValueType::Error) {
                    if let Some(signature) = signatures.get_mut(&key) {
                        signature.return_type = Some(return_type);
                    }
                }
            }
        }
    }

    materialize_calls(
        &signatures,
        &expression_types,
        &reference_targets,
        &mut records,
        &mut types,
        &mut diagnostics,
    );
    if let Some(declaration) = modules
        .values()
        .next()
        .and_then(|module| module.declarations.first())
    {
        let span = match declaration {
            Declaration::Function(v) => &v.span,
            Declaration::Record(v) => &v.span,
            Declaration::Enum(v) => &v.span,
        };
        validate_instantiated_aggregates(&records, &types, &record_keys, &mut diagnostics, span);
    }
    sort_diagnostics(&mut diagnostics);
    let mut analysis = Analysis {
        types,
        diagnostics,
        modules,
        module_symbols,
        signatures,
        expression_types,
        reference_targets,
        records,
        record_keys,
    };
    analysis
        .diagnostics
        .extend(crate::effects::check(&analysis));
    sort_diagnostics(&mut analysis.diagnostics);
    analysis
}

fn lexical_diagnostic(error: crate::lexer::LexError) -> Diagnostic {
    Diagnostic::error(&error.code, error.message, error.primary)
}

fn parse_diagnostic(error: ParseError) -> Diagnostic {
    Diagnostic::error(&error.code, error.message, error.primary)
}

fn resolve_imports(
    sources: &SourceSet,
    modules: &BTreeMap<String, Module>,
    module_symbols: &mut BTreeMap<String, BTreeMap<String, FunctionKey>>,
    edges: &mut Vec<ModuleEdges>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (source_id, module) in modules {
        let symbols = module_symbols.entry(source_id.clone()).or_default();
        let mut imported_names = BTreeSet::new();
        let mut duplicate_spans = BTreeSet::new();
        for import in &module.imports {
            for imported in &import.names {
                if builtin_name(&imported.name) {
                    diagnostics.push(Diagnostic::error(
                        "UBI0011",
                        "Reserved prelude name",
                        imported.span.clone(),
                    ));
                    continue;
                }
                if !imported_names.insert(imported.name.clone()) {
                    diagnostics.push(Diagnostic::error(
                        "UBI0011",
                        format!("Duplicate imported name: {}", imported.name),
                        imported.span.clone(),
                    ));
                    duplicate_spans.insert((imported.span.start, imported.span.end));
                }
            }
        }
        for import in &module.imports {
            let target_id = match resolve_import_id(source_id, &import.path) {
                Ok(target_id) => target_id,
                Err(()) => {
                    diagnostics.push(Diagnostic::error(
                        "UBI0012",
                        format!("Invalid relative module path: {}", import.path),
                        import.path_span.clone(),
                    ));
                    continue;
                }
            };
            if sources.get(&target_id).is_none() || !modules.contains_key(&target_id) {
                diagnostics.push(Diagnostic::error(
                    "UBI0012",
                    format!("Module not supplied: {target_id}"),
                    import.path_span.clone(),
                ));
                continue;
            }
            edges.push(ModuleEdges {
                from: source_id.clone(),
                to: target_id.clone(),
                span: import.path_span.clone(),
            });

            for imported in &import.names {
                if duplicate_spans.contains(&(imported.span.start, imported.span.end)) {
                    continue;
                }
                match modules[&target_id].declarations.iter().find(
                    |declaration| match declaration {
                        Declaration::Function(value) => {
                            value.name.name == imported.name && value.exported
                        }
                        Declaration::Record(value) => {
                            value.name.name == imported.name && value.exported
                        }
                        Declaration::Enum(value) => {
                            value.name.name == imported.name && value.exported
                        }
                    },
                ) {
                    Some(_) => {
                        symbols.insert(
                            imported.name.clone(),
                            (target_id.clone(), imported.name.clone()),
                        );
                    }
                    _ => diagnostics.push(Diagnostic::error(
                        "UBI0012",
                        format!("Module does not export `{}`", imported.name),
                        imported.span.clone(),
                    )),
                }
            }
        }
    }
}

fn report_import_cycles<'a>(
    source_ids: impl Iterator<Item = &'a String>,
    edges: &[ModuleEdges],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ids: Vec<String> = source_ids.cloned().collect();
    let positions: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    let mut reachable = vec![vec![false; ids.len()]; ids.len()];
    let mut self_edges = BTreeSet::new();
    for (index, row) in reachable.iter_mut().enumerate() {
        row[index] = true;
    }
    for edge in edges {
        let (Some(&from), Some(&to)) = (
            positions.get(edge.from.as_str()),
            positions.get(edge.to.as_str()),
        ) else {
            continue;
        };
        reachable[from][to] = true;
        if from == to {
            self_edges.insert(from);
        }
    }
    for pivot in 0..ids.len() {
        let pivot_row = reachable[pivot].clone();
        for row in &mut reachable {
            if !row[pivot] {
                continue;
            }
            for (is_reachable, through_pivot) in row.iter_mut().zip(&pivot_row) {
                *is_reachable |= *through_pivot;
            }
        }
    }

    let mut assigned = BTreeSet::new();
    for (index, reachable_from_index) in reachable.iter().enumerate() {
        if assigned.contains(&index) {
            continue;
        }
        let component: BTreeSet<usize> = reachable_from_index
            .iter()
            .enumerate()
            .filter(|(other, is_reachable)| **is_reachable && reachable[*other][index])
            .map(|(other, _)| other)
            .collect();
        assigned.extend(component.iter().copied());
        let cyclic = component.len() > 1 || self_edges.contains(&index);
        if !cyclic {
            continue;
        }

        let mut cycle_edges: Vec<&ModuleEdges> = edges
            .iter()
            .filter(|edge| {
                let (Some(&from), Some(&to)) = (
                    positions.get(edge.from.as_str()),
                    positions.get(edge.to.as_str()),
                ) else {
                    return false;
                };
                component.contains(&from) && component.contains(&to)
            })
            .collect();
        cycle_edges.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.span.start.cmp(&right.span.start))
                .then_with(|| left.span.end.cmp(&right.span.end))
        });
        let Some(primary) = cycle_edges.first() else {
            continue;
        };
        let mut diagnostic =
            Diagnostic::error("UBI0012", "Import cycle detected", primary.span.clone());
        diagnostic.related = cycle_edges
            .iter()
            .skip(1)
            .map(|edge| RelatedDiagnostic {
                span: edge.span.clone(),
                message: "Other import edge in this cycle".to_owned(),
            })
            .collect();
        diagnostics.push(diagnostic);
    }
}

fn generic_parameters(
    key: &FunctionKey,
    names: &[TypeParameter],
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValueType> {
    let mut seen = BTreeSet::new();
    names
        .iter()
        .map(|parameter| {
            if !seen.insert(parameter.name.clone()) {
                diagnostics.push(Diagnostic::error(
                    "UBI0011",
                    "Duplicate type parameter",
                    parameter.span.clone(),
                ));
            }
            if matches!(
                parameter.name.as_str(),
                "List"
                    | "Option"
                    | "Result"
                    | "Map"
                    | "Console"
                    | "Storage"
                    | "Http"
                    | "HostError"
                    | "int"
                    | "float"
                    | "bool"
                    | "string"
                    | "unit"
            ) {
                diagnostics.push(Diagnostic::error(
                    "UBI0020",
                    "Reserved type parameter name",
                    parameter.span.clone(),
                ));
            }
            let equatable = match &parameter.bound {
                Some(bound) if bound.name == "Equatable" => true,
                Some(bound) => {
                    diagnostics.push(Diagnostic::error(
                        "UBI0020",
                        format!("Unknown type parameter constraint: {}", bound.name),
                        bound.span.clone(),
                    ));
                    false
                }
                None => false,
            };
            intern_type(
                types,
                TypeInfo::Parameter {
                    key: key.clone(),
                    name: parameter.name.clone(),
                    equatable,
                },
            )
        })
        .collect()
}

fn collect_records(
    modules: &BTreeMap<String, Module>,
    module_symbols: &mut BTreeMap<String, BTreeMap<String, FunctionKey>>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<RecordInfo>, BTreeMap<FunctionKey, usize>) {
    let mut records = Vec::new();
    let mut keys = BTreeMap::new();
    let result_key = (String::new(), "Result".to_owned());
    let result_t = intern_type(
        types,
        TypeInfo::Parameter {
            key: result_key.clone(),
            name: "T".to_owned(),
            equatable: false,
        },
    );
    let result_e = intern_type(
        types,
        TypeInfo::Parameter {
            key: result_key.clone(),
            name: "E".to_owned(),
            equatable: false,
        },
    );
    types.push(TypeInfo::Enum {
        key: result_key,
        exported: true,
        arguments: vec![result_t, result_e],
        variants: BTreeMap::from([
            ("Ok".to_owned(), vec![result_t]),
            ("Err".to_owned(), vec![result_e]),
        ]),
    });
    types.push(TypeInfo::Enum {
        key: (String::new(), "HostError".to_owned()),
        exported: true,
        arguments: vec![],
        variants: BTreeMap::from([
            ("Failure".to_owned(), vec![ValueType::String]),
            ("Decode".to_owned(), vec![ValueType::String]),
            ("Cancelled".to_owned(), vec![ValueType::String]),
        ]),
    });
    for (source_id, module) in modules {
        let symbols = module_symbols.entry(source_id.clone()).or_default();
        for declaration in &module.declarations {
            let name = match declaration {
                Declaration::Function(value) => &value.name,
                Declaration::Record(value) => &value.name,
                Declaration::Enum(value) => &value.name,
            };
            if symbols.contains_key(&name.name)
                || builtin_name(&name.name)
                || matches!(
                    name.name.as_str(),
                    "List"
                        | "Option"
                        | "Result"
                        | "Map"
                        | "Console"
                        | "Storage"
                        | "Http"
                        | "HostError"
                )
            {
                diagnostics.push(Diagnostic::error(
                    "UBI0011",
                    format!("Duplicate or reserved declaration: {}", name.name),
                    name.span.clone(),
                ));
                continue;
            }
            let key = (source_id.clone(), name.name.clone());
            symbols.insert(name.name.clone(), key.clone());
            if let Declaration::Record(record) = declaration {
                let arguments =
                    generic_parameters(&key, &record.type_parameters, types, diagnostics);
                keys.insert(key.clone(), records.len());
                records.push(RecordInfo {
                    key: key.clone(),
                    exported: record.exported,
                    arguments,
                    fields: BTreeMap::new(),
                });
            }
            if let Declaration::Enum(value) = declaration {
                let arguments =
                    generic_parameters(&key, &value.type_parameters, types, diagnostics);
                types.push(TypeInfo::Enum {
                    key,
                    exported: value.exported,
                    arguments,
                    variants: BTreeMap::new(),
                });
            }
        }
    }
    let templates: Vec<_> = records
        .iter()
        .enumerate()
        .map(|(id, r)| (id, r.key.clone()))
        .collect();
    for (id, key) in &templates {
        let declaration = modules[&key.0]
            .declarations
            .iter()
            .find_map(|d| match d {
                Declaration::Record(v) if v.name.name == key.1 => Some(v),
                _ => None,
            })
            .unwrap();
        let parameters = records[*id].arguments.clone();
        let mut fields = BTreeMap::new();
        for field in &declaration.fields {
            let ty = type_from_syntax(
                &field.ty,
                &key.0,
                module_symbols,
                &keys,
                &parameters,
                &mut records,
                types,
                diagnostics,
            );
            if fields.insert(field.name.name.clone(), ty).is_some() {
                diagnostics.push(Diagnostic::error(
                    "UBI0030",
                    "Duplicate record field",
                    field.name.span.clone(),
                ));
            }
        }
        records[*id].fields = fields;
    }
    let enums: Vec<_> = types
        .iter()
        .enumerate()
        .filter_map(|(id, info)| match info {
            TypeInfo::Enum { key, .. } if !key.0.is_empty() => Some((id, key.clone())),
            _ => None,
        })
        .collect();
    for (id, key) in &enums {
        // Only declaration templates are populated from syntax.
        let template = enum_type(types, key).unwrap();
        if *id != template {
            continue;
        }
        let declaration = modules[&key.0]
            .declarations
            .iter()
            .find_map(|d| match d {
                Declaration::Enum(v) if v.name.name == key.1 => Some(v),
                _ => None,
            })
            .unwrap();
        let TypeInfo::Enum { arguments, .. } = &types[*id] else {
            unreachable!()
        };
        let parameters = arguments.clone();
        let mut variants = BTreeMap::new();
        for variant in &declaration.variants {
            let payloads = variant
                .payloads
                .iter()
                .map(|ty| {
                    type_from_syntax(
                        ty,
                        &key.0,
                        module_symbols,
                        &keys,
                        &parameters,
                        &mut records,
                        types,
                        diagnostics,
                    )
                })
                .collect();
            if variants
                .insert(variant.name.name.clone(), payloads)
                .is_some()
            {
                diagnostics.push(Diagnostic::error(
                    "UBI0011",
                    "Duplicate enum variant",
                    variant.name.span.clone(),
                ));
            }
        }
        let TypeInfo::Enum {
            variants: target, ..
        } = &mut types[*id]
        else {
            unreachable!()
        };
        *target = variants;
    }
    if let Err(error) = refresh_nominals(&keys, &mut records, types) {
        if error == SubstitutionError::Limit {
            if let Some(module) = modules.values().next() {
                if let Some(decl) = module.declarations.first() {
                    let span = match decl {
                        Declaration::Function(v) => &v.span,
                        Declaration::Record(v) => &v.span,
                        Declaration::Enum(v) => &v.span,
                    };
                    diagnostics.push(substitution_diagnostic(error, span));
                }
            }
        }
    }
    for (id, key) in templates {
        if !records[id].exported {
            continue;
        }
        let declaration = modules[&key.0]
            .declarations
            .iter()
            .find_map(|d| match d {
                Declaration::Record(v) if v.name.name == key.1 => Some(v),
                _ => None,
            })
            .unwrap();
        for field in &declaration.fields {
            check_public_type(
                records[id].fields[&field.name.name],
                &field.ty.span,
                &records,
                types,
                diagnostics,
            );
        }
    }
    for (id, key) in enums {
        let TypeInfo::Enum {
            exported: true,
            variants,
            ..
        } = &types[id]
        else {
            continue;
        };
        if id != enum_type(types, &key).unwrap() {
            continue;
        }
        let declaration = modules[&key.0]
            .declarations
            .iter()
            .find_map(|d| match d {
                Declaration::Enum(v) if v.name.name == key.1 => Some(v),
                _ => None,
            })
            .unwrap();
        for variant in &declaration.variants {
            for (ty, syntax) in variants[&variant.name.name].iter().zip(&variant.payloads) {
                check_public_type(*ty, &syntax.span, &records, types, diagnostics);
            }
        }
    }
    (records, keys)
}

fn check_public_type(
    ty: ValueType,
    span: &Span,
    records: &[RecordInfo],
    types: &[TypeInfo],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut pending = vec![ty];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match ty {
            ValueType::Tuple(id) => {
                if let TypeInfo::Tuple(elements) = &types[id] {
                    pending.extend(elements.iter().copied());
                }
            }
            ValueType::Record(id) => {
                if !records[id].exported {
                    diagnostics.push(Diagnostic::error(
                        "UBI0013",
                        "Private record type exposed by exported signature",
                        span.clone(),
                    ));
                }
                pending.extend(records[id].arguments.iter().copied());
            }
            ValueType::Enum(id) => {
                let TypeInfo::Enum {
                    exported,
                    arguments,
                    ..
                } = &types[id]
                else {
                    unreachable!()
                };
                if !exported {
                    diagnostics.push(Diagnostic::error(
                        "UBI0013",
                        "Private enum type exposed by exported signature",
                        span.clone(),
                    ));
                }
                pending.extend(arguments.iter().copied());
            }
            ValueType::List(id) | ValueType::Option(id) => {
                let (TypeInfo::List(inner) | TypeInfo::Option(inner)) = types[id] else {
                    unreachable!()
                };
                pending.push(inner);
            }
            ValueType::Map(id) => {
                if let TypeInfo::Map { key, value } = types[id] {
                    pending.extend([key, value]);
                }
            }
            ValueType::Function(id) => {
                if let TypeInfo::Function {
                    parameters,
                    return_type,
                } = &types[id]
                {
                    pending.extend(parameters.iter().copied());
                    pending.push(*return_type);
                }
            }
            ValueType::Range => diagnostics.push(Diagnostic::error(
                "UBI0020",
                "Range values cannot be exported",
                span.clone(),
            )),
            _ => {}
        }
    }
}

fn collect_signatures<'a>(
    modules: &'a BTreeMap<String, Module>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &mut Vec<RecordInfo>,
    record_keys: &BTreeMap<FunctionKey, usize>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    BTreeMap<FunctionKey, &'a Function>,
    BTreeMap<FunctionKey, Signature>,
) {
    let mut functions = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    for (source_id, module) in modules {
        for declaration in &module.declarations {
            let Declaration::Function(function) = declaration else {
                continue;
            };
            let key = (source_id.clone(), function.name.name.clone());
            if record_keys.contains_key(&key)
                || functions.contains_key(&key)
                || module_symbols[source_id].get(&function.name.name) != Some(&key)
            {
                continue;
            }
            let type_parameters =
                generic_parameters(&key, &function.type_parameters, types, diagnostics);
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    let ty = type_from_syntax(
                        &parameter.ty,
                        source_id,
                        module_symbols,
                        record_keys,
                        &type_parameters,
                        records,
                        types,
                        diagnostics,
                    );
                    if function.exported {
                        check_public_type(ty, &parameter.ty.span, records, types, diagnostics);
                    }
                    ty
                })
                .collect();
            let return_type = function.return_type.as_ref().map(|ty| {
                let value = type_from_syntax(
                    ty,
                    source_id,
                    module_symbols,
                    record_keys,
                    &type_parameters,
                    records,
                    types,
                    diagnostics,
                );
                if function.exported {
                    check_public_type(value, &ty.span, records, types, diagnostics);
                }
                value
            });
            if (function.exported || function.asynchronous || !type_parameters.is_empty())
                && return_type.is_none()
            {
                diagnostics.push(Diagnostic::error(
                    "UBI0021",
                    "Exported, async, and generic functions require an explicit return annotation",
                    function.name.span.clone(),
                ));
            }
            functions.insert(key.clone(), function);
            signatures.insert(
                key,
                Signature {
                    asynchronous: function.asynchronous,
                    type_parameters,
                    parameters,
                    return_type,
                    name_span: function.name.span.clone(),
                },
            );
        }
    }
    (functions, signatures)
}

#[allow(clippy::too_many_arguments)]
fn type_from_syntax(
    ty: &Type,
    module_id: &str,
    symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    record_keys: &BTreeMap<FunctionKey, usize>,
    parameters: &[ValueType],
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> ValueType {
    let result = (|| {
        for parameter in parameters {
            let ValueType::Parameter(id) = parameter else {
                continue;
            };
            if matches!(&types[*id],TypeInfo::Parameter{name,..} if name==&ty.name) {
                return ty.arguments.is_empty().then_some(*parameter);
            }
        }
        let arguments: Vec<_> = ty
            .arguments
            .iter()
            .map(|arg| {
                type_from_syntax(
                    arg,
                    module_id,
                    symbols,
                    record_keys,
                    parameters,
                    records,
                    types,
                    diagnostics,
                )
            })
            .collect();
        if ty.name == "$tuple" {
            return Some(intern_type(types, TypeInfo::Tuple(arguments)));
        }
        if ty.name == "fn" {
            let (return_type, parameters) = arguments.split_last()?;
            return Some(intern_type(
                types,
                TypeInfo::Function {
                    parameters: parameters.to_vec(),
                    return_type: *return_type,
                },
            ));
        }
        if ty.name == "Map" {
            if arguments.len() != 2 {
                return None;
            }
            if !matches!(
                arguments[0],
                ValueType::Int | ValueType::Bool | ValueType::String | ValueType::Error
            ) {
                return None;
            }
            return Some(intern_type(
                types,
                TypeInfo::Map {
                    key: arguments[0],
                    value: arguments[1],
                },
            ));
        }
        if matches!(ty.name.as_str(), "List" | "Option") {
            return (arguments.len() == 1).then(|| {
                intern_type(
                    types,
                    if ty.name == "List" {
                        TypeInfo::List(arguments[0])
                    } else {
                        TypeInfo::Option(arguments[0])
                    },
                )
            });
        }
        let primitive = match ty.name.as_str() {
            "int" => Some(ValueType::Int),
            "float" => Some(ValueType::Float),
            "bool" => Some(ValueType::Bool),
            "string" => Some(ValueType::String),
            "unit" => Some(ValueType::Unit),
            "Console" => Some(ValueType::Capability(CapabilityKind::Console)),
            "Storage" => Some(ValueType::Capability(CapabilityKind::Storage)),
            "Http" => Some(ValueType::Capability(CapabilityKind::Http)),
            _ => None,
        };
        if let Some(primitive) = primitive {
            return arguments.is_empty().then_some(primitive);
        }
        let key = if matches!(ty.name.as_str(), "Result" | "HostError") {
            Some((String::new(), ty.name.clone()))
        } else {
            symbols
                .get(module_id)
                .and_then(|s| s.get(&ty.name))
                .cloned()
        }?;
        let (template, params) = if let Some(id) = record_keys.get(&key) {
            (ValueType::Record(*id), records[*id].arguments.clone())
        } else {
            let id = enum_type(types, &key)?;
            let TypeInfo::Enum { arguments, .. } = &types[id] else {
                unreachable!()
            };
            (ValueType::Enum(id), arguments.clone())
        };
        if params.len() != arguments.len() {
            return None;
        }
        let mapping = parameter_mapping(&params, &arguments);
        match substitute_type(template, &mapping, records, types) {
            Ok(value) => Some(value),
            Err(error) => {
                diagnostics.push(substitution_diagnostic(error, &ty.span));
                Some(ValueType::Error)
            }
        }
    })();
    result.unwrap_or_else(|| {
        diagnostics.push(Diagnostic::error(
            "UBI0020",
            format!("Unknown type or invalid type arguments: {}", ty.name),
            ty.span.clone(),
        ));
        ValueType::Error
    })
}

fn contains_parameters(
    ty: ValueType,
    parameters: &BTreeSet<usize>,
    records: &[RecordInfo],
    types: &[TypeInfo],
) -> bool {
    let mut pending = vec![ty];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match ty {
            ValueType::Tuple(id) => {
                if let TypeInfo::Tuple(elements) = &types[id] {
                    pending.extend(elements.iter().copied());
                }
            }
            ValueType::Parameter(id) if parameters.contains(&id) => return true,
            ValueType::Record(id) => pending.extend(records[id].arguments.iter().copied()),
            ValueType::Enum(id) => {
                if let TypeInfo::Enum { arguments, .. } = &types[id] {
                    pending.extend(arguments.iter().copied());
                }
            }
            ValueType::List(id) | ValueType::Option(id) => {
                if let TypeInfo::List(inner) | TypeInfo::Option(inner) = types[id] {
                    pending.push(inner);
                }
            }
            ValueType::Map(id) => {
                if let TypeInfo::Map { key, value } = types[id] {
                    pending.extend([key, value]);
                }
            }
            ValueType::Function(id) => {
                if let TypeInfo::Function {
                    parameters,
                    return_type,
                } = &types[id]
                {
                    pending.extend(parameters.iter().copied());
                    pending.push(*return_type);
                }
            }
            _ => {}
        }
    }
    false
}

fn needs_context(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Await(operand) => needs_context(operand),
        ExprKind::List(values) => values.is_empty() || values.iter().any(needs_context),
        ExprKind::Tuple(values) => values.iter().any(needs_context),
        ExprKind::Member { .. }
        | ExprKind::Call { .. }
        | ExprKind::Record { .. }
        | ExprKind::Name(_) => true,
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            then_branch
                .tail
                .as_ref()
                .is_some_and(|tail| needs_context(tail))
                || needs_context(else_branch)
        }
        ExprKind::Block(block) => block.tail.as_ref().is_some_and(|tail| needs_context(tail)),
        _ => false,
    }
}

fn parameter_mapping(
    parameters: &[ValueType],
    arguments: &[ValueType],
) -> BTreeMap<usize, ValueType> {
    parameters
        .iter()
        .zip(arguments)
        .filter_map(|(p, a)| {
            if let ValueType::Parameter(id) = p {
                Some((*id, *a))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn infer_type_arguments(
    template: ValueType,
    actual: ValueType,
    mapping: &mut BTreeMap<usize, ValueType>,
    records: &[RecordInfo],
    types: &[TypeInfo],
) -> bool {
    let mut pending = vec![(template, actual)];
    let mut visited = BTreeSet::new();
    while let Some((template, actual)) = pending.pop() {
        if !visited.insert((template, actual)) {
            continue;
        }
        if actual == ValueType::Error || actual == ValueType::Never || template == ValueType::Error
        {
            continue;
        }
        if let ValueType::Parameter(id) = template {
            if let Some(previous) = mapping.get(&id) {
                if *previous != actual {
                    return false;
                }
            } else {
                mapping.insert(id, actual);
            }
            continue;
        }
        if template == actual {
            continue;
        }
        match (template, actual) {
            (ValueType::Tuple(a), ValueType::Tuple(b)) => {
                let (TypeInfo::Tuple(a), TypeInfo::Tuple(b)) = (&types[a], &types[b]) else {
                    return false;
                };
                if a.len() != b.len() {
                    return false;
                }
                pending.extend(a.iter().copied().zip(b.iter().copied()));
            }
            (ValueType::List(a), ValueType::List(b))
            | (ValueType::Option(a), ValueType::Option(b)) => {
                let (
                    TypeInfo::List(a) | TypeInfo::Option(a),
                    TypeInfo::List(b) | TypeInfo::Option(b),
                ) = (&types[a], &types[b])
                else {
                    return false;
                };
                pending.push((*a, *b));
            }
            (ValueType::Map(a), ValueType::Map(b)) => {
                let (TypeInfo::Map { key: ak, value: av }, TypeInfo::Map { key: bk, value: bv }) =
                    (&types[a], &types[b])
                else {
                    return false;
                };
                pending.extend([(*ak, *bk), (*av, *bv)]);
            }
            (ValueType::Record(a), ValueType::Record(b)) if records[a].key == records[b].key => {
                pending.extend(
                    records[a]
                        .arguments
                        .iter()
                        .copied()
                        .zip(records[b].arguments.iter().copied()),
                )
            }
            (ValueType::Enum(a), ValueType::Enum(b)) => {
                let (
                    TypeInfo::Enum {
                        key: a_key,
                        arguments: a,
                        ..
                    },
                    TypeInfo::Enum {
                        key: b_key,
                        arguments: b,
                        ..
                    },
                ) = (&types[a], &types[b])
                else {
                    return false;
                };
                if a_key != b_key {
                    return false;
                }
                pending.extend(a.iter().copied().zip(b.iter().copied()));
            }
            (ValueType::Function(a), ValueType::Function(b)) => {
                let (
                    TypeInfo::Function {
                        parameters: a,
                        return_type: ar,
                    },
                    TypeInfo::Function {
                        parameters: b,
                        return_type: br,
                    },
                ) = (&types[a], &types[b])
                else {
                    return false;
                };
                if a.len() != b.len() {
                    return false;
                }
                pending.push((*ar, *br));
                pending.extend(a.iter().copied().zip(b.iter().copied()));
            }
            _ => return false,
        }
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SubstitutionError {
    Limit,
    Constraint,
}

fn substitution_diagnostic(error: SubstitutionError, span: &Span) -> Diagnostic {
    match error {
        SubstitutionError::Limit => Diagnostic::error(
            "UBI0090",
            "Generic type expansion limit exceeded",
            span.clone(),
        ),
        SubstitutionError::Constraint => Diagnostic::error(
            "UBI0020",
            "Type argument does not satisfy Equatable constraint",
            span.clone(),
        ),
    }
}

fn substitute_type(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
) -> Result<ValueType, SubstitutionError> {
    for (id, actual) in mapping {
        if matches!(
            types.get(*id),
            Some(TypeInfo::Parameter {
                equatable: true,
                ..
            })
        ) && !is_equatable(*actual, records, types)
        {
            return Err(SubstitutionError::Constraint);
        }
    }
    substitute_type_unchecked(ty, mapping, records, types)
}

fn substitute_type_unchecked(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
) -> Result<ValueType, SubstitutionError> {
    substitute_inner(ty, mapping, records, types, 0, &mut BTreeMap::new())
        .map_err(|()| SubstitutionError::Limit)
}

fn substitute_inner(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
    depth: usize,
    cache: &mut BTreeMap<(ValueType, usize), ValueType>,
) -> Result<ValueType, ()> {
    if depth > 32 || records.len() + types.len() > 4096 {
        return Err(());
    }
    if let Some(result) = cache.get(&(ty, depth)) {
        return Ok(*result);
    }
    let result = match ty {
        ValueType::Tuple(id) => {
            let TypeInfo::Tuple(elements) = types[id].clone() else {
                return Err(());
            };
            let elements = elements
                .into_iter()
                .map(|element| substitute_inner(element, mapping, records, types, depth + 1, cache))
                .collect::<Result<_, _>>()?;
            intern_type(types, TypeInfo::Tuple(elements))
        }
        ValueType::Parameter(id) => mapping.get(&id).copied().unwrap_or(ty),
        ValueType::Map(id) => {
            let TypeInfo::Map { key, value } = types[id] else {
                return Err(());
            };
            let key = substitute_inner(key, mapping, records, types, depth + 1, cache)?;
            let value = substitute_inner(value, mapping, records, types, depth + 1, cache)?;
            intern_type(types, TypeInfo::Map { key, value })
        }
        ValueType::List(id) | ValueType::Option(id) => {
            let (TypeInfo::List(inner) | TypeInfo::Option(inner)) = types[id] else {
                unreachable!()
            };
            let inner = substitute_inner(inner, mapping, records, types, depth + 1, cache)?;
            intern_type(
                types,
                if matches!(ty, ValueType::List(_)) {
                    TypeInfo::List(inner)
                } else {
                    TypeInfo::Option(inner)
                },
            )
        }
        ValueType::Function(id) => {
            let TypeInfo::Function {
                parameters,
                return_type,
            } = types[id].clone()
            else {
                unreachable!()
            };
            let parameters = parameters
                .into_iter()
                .map(|p| substitute_inner(p, mapping, records, types, depth + 1, cache))
                .collect::<Result<_, _>>()?;
            let return_type =
                substitute_inner(return_type, mapping, records, types, depth + 1, cache)?;
            intern_type(
                types,
                TypeInfo::Function {
                    parameters,
                    return_type,
                },
            )
        }
        ValueType::Record(id) => {
            let record = records[id].clone();
            let arguments = record
                .arguments
                .iter()
                .map(|a| substitute_inner(*a, mapping, records, types, depth + 1, cache))
                .collect::<Result<Vec<_>, _>>()?;
            if arguments == record.arguments {
                return Ok(ty);
            }
            if let Some(id) = records
                .iter()
                .position(|r| r.key == record.key && r.arguments == arguments)
            {
                return Ok(ValueType::Record(id));
            }
            let id = records.len();
            records.push(RecordInfo {
                key: record.key,
                exported: record.exported,
                arguments,
                fields: BTreeMap::new(),
            });
            let fields = record
                .fields
                .into_iter()
                .map(|(name, ty)| {
                    Ok((
                        name,
                        substitute_inner(ty, mapping, records, types, depth + 1, cache)?,
                    ))
                })
                .collect::<Result<_, ()>>()?;
            records[id].fields = fields;
            ValueType::Record(id)
        }
        ValueType::Enum(id) => {
            let TypeInfo::Enum {
                key,
                exported,
                arguments,
                variants,
            } = types[id].clone()
            else {
                unreachable!()
            };
            let args = arguments
                .iter()
                .map(|a| substitute_inner(*a, mapping, records, types, depth + 1, cache))
                .collect::<Result<Vec<_>, _>>()?;
            if args == arguments {
                return Ok(ty);
            }
            if let Some(id) = types.iter().position(
                |t| matches!(t,TypeInfo::Enum{key:k,arguments:a,..} if k==&key && a==&args),
            ) {
                return Ok(ValueType::Enum(id));
            }
            let id = types.len();
            types.push(TypeInfo::Enum {
                key,
                exported,
                arguments: args,
                variants: BTreeMap::new(),
            });
            let variants = variants
                .into_iter()
                .map(|(name, payloads)| {
                    Ok((
                        name,
                        payloads
                            .into_iter()
                            .map(|p| substitute_inner(p, mapping, records, types, depth + 1, cache))
                            .collect::<Result<_, _>>()?,
                    ))
                })
                .collect::<Result<_, ()>>()?;
            let TypeInfo::Enum {
                variants: target, ..
            } = &mut types[id]
            else {
                unreachable!()
            };
            *target = variants;
            ValueType::Enum(id)
        }
        _ => ty,
    };
    if records.len() + types.len() > 4096 {
        return Err(());
    }
    cache.insert((ty, depth), result);
    Ok(result)
}

#[cfg(test)]
pub(crate) fn substitute_type_existing(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &[RecordInfo],
    types: &[TypeInfo],
) -> Option<ValueType> {
    substitute_existing_inner(ty, mapping, records, types, &mut BTreeMap::new())
}

#[cfg(test)]
fn substitute_existing_inner(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &[RecordInfo],
    types: &[TypeInfo],
    cache: &mut BTreeMap<ValueType, ValueType>,
) -> Option<ValueType> {
    if let Some(result) = cache.get(&ty) {
        return Some(*result);
    }
    let result = match ty {
        ValueType::Tuple(id) => {
            let TypeInfo::Tuple(elements) = &types[id] else {
                return None;
            };
            let elements = elements
                .iter()
                .map(|element| substitute_existing_inner(*element, mapping, records, types, cache))
                .collect::<Option<Vec<_>>>()?;
            let info = TypeInfo::Tuple(elements);
            ValueType::Tuple(types.iter().position(|t| t == &info)?)
        }
        ValueType::Parameter(id) => mapping.get(&id).copied().unwrap_or(ty),
        ValueType::Map(id) => {
            let TypeInfo::Map { key, value } = types[id] else {
                return None;
            };
            let key = substitute_existing_inner(key, mapping, records, types, cache)?;
            let value = substitute_existing_inner(value, mapping, records, types, cache)?;
            ValueType::Map(
                types
                    .iter()
                    .position(|ty| *ty == TypeInfo::Map { key, value })?,
            )
        }
        ValueType::List(id) | ValueType::Option(id) => {
            let (TypeInfo::List(inner) | TypeInfo::Option(inner)) = types[id] else {
                return None;
            };
            let inner = substitute_existing_inner(inner, mapping, records, types, cache)?;
            let info = if matches!(ty, ValueType::List(_)) {
                TypeInfo::List(inner)
            } else {
                TypeInfo::Option(inner)
            };
            let id = types.iter().position(|t| t == &info)?;
            if matches!(ty, ValueType::List(_)) {
                ValueType::List(id)
            } else {
                ValueType::Option(id)
            }
        }
        ValueType::Function(id) => {
            let TypeInfo::Function {
                parameters,
                return_type,
            } = &types[id]
            else {
                return None;
            };
            let parameters: Vec<ValueType> = parameters
                .iter()
                .map(|p| substitute_existing_inner(*p, mapping, records, types, cache))
                .collect::<Option<_>>()?;
            let return_type =
                substitute_existing_inner(*return_type, mapping, records, types, cache)?;
            ValueType::Function(types.iter().position(|t| {
                t == &TypeInfo::Function {
                    parameters: parameters.clone(),
                    return_type,
                }
            })?)
        }
        ValueType::Record(id) => {
            let record = &records[id];
            let arguments = record
                .arguments
                .iter()
                .map(|p| substitute_existing_inner(*p, mapping, records, types, cache))
                .collect::<Option<Vec<_>>>()?;
            ValueType::Record(
                records
                    .iter()
                    .position(|r| r.key == record.key && r.arguments == arguments)?,
            )
        }
        ValueType::Enum(id) => {
            let TypeInfo::Enum { key, arguments, .. } = &types[id] else {
                return None;
            };
            let arguments = arguments
                .iter()
                .map(|p| substitute_existing_inner(*p, mapping, records, types, cache))
                .collect::<Option<Vec<_>>>()?;
            ValueType::Enum(types.iter().position(
                |t| matches!(t,TypeInfo::Enum{key:k,arguments:a,..} if k==key && a==&arguments),
            )?)
        }
        _ => ty,
    };
    cache.insert(ty, result);
    Some(result)
}

fn substitute_nominal_type(
    ty: ValueType,
    mapping: &BTreeMap<usize, ValueType>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
    constraint_failed: &mut bool,
) -> Result<ValueType, SubstitutionError> {
    match substitute_type(ty, mapping, records, types) {
        Err(SubstitutionError::Constraint) => {
            *constraint_failed = true;
            substitute_type_unchecked(ty, mapping, records, types)
        }
        result => result,
    }
}

fn refresh_nominals(
    keys: &BTreeMap<FunctionKey, usize>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
) -> Result<(), SubstitutionError> {
    let mut record_id = 0;
    let mut enum_id = 0;
    let mut constraint_failed = false;
    while record_id < records.len() || enum_id < types.len() {
        if records.len() + types.len() > 4096 {
            return Err(SubstitutionError::Limit);
        }
        if record_id < records.len() {
            let record = records[record_id].clone();
            let template = keys[&record.key];
            if template != record_id {
                let mapping = parameter_mapping(&records[template].arguments, &record.arguments);
                let fields = records[template]
                    .fields
                    .clone()
                    .into_iter()
                    .map(|(name, ty)| {
                        Ok((
                            name,
                            substitute_nominal_type(
                                ty,
                                &mapping,
                                records,
                                types,
                                &mut constraint_failed,
                            )?,
                        ))
                    })
                    .collect::<Result<_, SubstitutionError>>()?;
                records[record_id].fields = fields;
            }
            record_id += 1;
        }
        if enum_id < types.len() {
            if let TypeInfo::Enum { key, arguments, .. } = types[enum_id].clone() {
                let template = enum_type(types, &key).unwrap();
                if template != enum_id {
                    let TypeInfo::Enum {
                        arguments: params,
                        variants,
                        ..
                    } = types[template].clone()
                    else {
                        unreachable!()
                    };
                    let mapping = parameter_mapping(&params, &arguments);
                    let variants = variants
                        .into_iter()
                        .map(|(name, payloads)| {
                            Ok((
                                name,
                                payloads
                                    .into_iter()
                                    .map(|ty| {
                                        substitute_nominal_type(
                                            ty,
                                            &mapping,
                                            records,
                                            types,
                                            &mut constraint_failed,
                                        )
                                    })
                                    .collect::<Result<_, _>>()?,
                            ))
                        })
                        .collect::<Result<_, SubstitutionError>>()?;
                    let TypeInfo::Enum {
                        variants: target, ..
                    } = &mut types[enum_id]
                    else {
                        unreachable!()
                    };
                    *target = variants;
                }
            }
            enum_id += 1;
        }
    }
    if constraint_failed {
        Err(SubstitutionError::Constraint)
    } else {
        Ok(())
    }
}

fn materialize_calls(
    signatures: &BTreeMap<FunctionKey, Signature>,
    expression_types: &BTreeMap<(FunctionKey, Span), ValueType>,
    references: &BTreeMap<(FunctionKey, Span), FunctionKey>,
    records: &mut Vec<RecordInfo>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut pending: Vec<_> = signatures
        .iter()
        .filter(|(_, s)| s.type_parameters.is_empty())
        .map(|(k, _)| (k.clone(), BTreeMap::new(), 0usize))
        .collect();
    let mut visited = BTreeSet::new();
    while let Some((key, mapping, depth)) = pending.pop() {
        if !visited.insert((key.clone(), mapping.clone())) {
            continue;
        }
        if depth > 32 || visited.len() > 4096 {
            diagnostics.push(Diagnostic::error(
                "UBI0090",
                "Generic call expansion limit exceeded",
                signatures[&key].name_span.clone(),
            ));
            return;
        }
        for ((owner, span), ty) in expression_types {
            if owner != &key {
                continue;
            }
            let actual = match substitute_type(*ty, &mapping, records, types) {
                Ok(ty) => ty,
                Err(error) => {
                    diagnostics.push(substitution_diagnostic(error, span));
                    return;
                }
            };
            if let Some(target) = references.get(&(key.clone(), span.clone())) {
                let signature = &signatures[target];
                if signature.type_parameters.is_empty() {
                    continue;
                }
                let TypeInfo::Function {
                    parameters,
                    return_type,
                } = &types[match actual {
                    ValueType::Function(id) => id,
                    _ => continue,
                }]
                else {
                    continue;
                };
                let mut inferred = BTreeMap::new();
                for (template, actual) in signature.parameters.iter().zip(parameters) {
                    infer_type_arguments(*template, *actual, &mut inferred, records, types);
                }
                if let Some(template) = signature.return_type {
                    infer_type_arguments(template, *return_type, &mut inferred, records, types);
                }
                pending.push((target.clone(), inferred, depth + 1));
            }
        }
    }
}

fn build_call_graph(
    functions: &BTreeMap<FunctionKey, &Function>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
) -> BTreeMap<FunctionKey, Vec<FunctionKey>> {
    let mut graph = BTreeMap::new();
    for (key, function) in functions {
        let mut calls = Vec::new();
        let mut scopes = vec![function
            .parameters
            .iter()
            .map(|parameter| parameter.name.name.clone())
            .collect::<HashSet<_>>()];
        collect_block_calls(
            &function.body,
            module_symbols.get(&key.0),
            &mut scopes,
            &mut calls,
        );
        calls.retain(|target| functions.contains_key(target));
        calls.sort();
        calls.dedup();
        graph.insert(key.clone(), calls);
    }
    graph
}

fn collect_block_calls(
    block: &Block,
    symbols: Option<&BTreeMap<String, FunctionKey>>,
    scopes: &mut Vec<HashSet<String>>,
    calls: &mut Vec<FunctionKey>,
) {
    scopes.push(HashSet::new());
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { name, value, .. } => {
                collect_expression_calls(value, symbols, scopes, calls);
                scopes.last_mut().unwrap().insert(name.name.clone());
            }
            StatementKind::LetPattern { pattern, value, .. } => {
                collect_expression_calls(value, symbols, scopes, calls);
                let mut names = HashSet::new();
                collect_pattern_names(pattern, &mut names);
                scopes.last_mut().unwrap().extend(names);
            }
            StatementKind::Assign { value, .. } => {
                collect_expression_calls(value, symbols, scopes, calls);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    collect_expression_calls(value, symbols, scopes, calls);
                }
            }
            StatementKind::Expression(expression) => {
                collect_expression_calls(expression, symbols, scopes, calls);
            }
            StatementKind::While { condition, body } => {
                collect_expression_calls(condition, symbols, scopes, calls);
                collect_block_calls(body, symbols, scopes, calls);
            }
            StatementKind::For {
                name,
                iterable,
                body,
            } => {
                collect_expression_calls(iterable, symbols, scopes, calls);
                scopes.push(HashSet::from([name.name.clone()]));
                collect_block_calls(body, symbols, scopes, calls);
                scopes.pop();
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
    if let Some(tail) = &block.tail {
        collect_expression_calls(tail, symbols, scopes, calls);
    }
    scopes.pop();
}

fn collect_expression_calls(
    expression: &Expr,
    symbols: Option<&BTreeMap<String, FunctionKey>>,
    scopes: &mut Vec<HashSet<String>>,
    calls: &mut Vec<FunctionKey>,
) {
    match &expression.kind {
        ExprKind::Name(name) => {
            if !scopes.iter().rev().any(|scope| scope.contains(&name.name)) {
                if let Some(key) = symbols.and_then(|map| map.get(&name.name)) {
                    calls.push(key.clone());
                }
            }
        }
        ExprKind::List(values) | ExprKind::Tuple(values) => {
            for value in values {
                collect_expression_calls(value, symbols, scopes, calls);
            }
        }
        ExprKind::Arrow { parameters, body } => {
            scopes.push(parameters.iter().map(|p| p.name.name.clone()).collect());
            collect_expression_calls(body, symbols, scopes, calls);
            scopes.pop();
        }
        ExprKind::Match { subject, arms } => {
            collect_expression_calls(subject, symbols, scopes, calls);
            for arm in arms {
                let mut bindings = HashSet::new();
                collect_pattern_names(&arm.pattern, &mut bindings);
                scopes.push(bindings);
                if let Some(guard) = &arm.guard {
                    collect_expression_calls(guard, symbols, scopes, calls);
                }
                collect_expression_calls(&arm.body, symbols, scopes, calls);
                scopes.pop();
            }
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Propagate(operand)
        | ExprKind::Await(operand) => {
            collect_expression_calls(operand, symbols, scopes, calls);
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expression_calls(left, symbols, scopes, calls);
            collect_expression_calls(right, symbols, scopes, calls);
        }
        ExprKind::Call { callee, arguments } => {
            if let ExprKind::Name(name) = &callee.kind {
                let is_local = scopes.iter().rev().any(|scope| scope.contains(&name.name));
                if !is_local {
                    if let Some(key) = symbols.and_then(|map| map.get(&name.name)) {
                        calls.push(key.clone());
                    }
                }
            } else {
                collect_expression_calls(callee, symbols, scopes, calls);
            }
            for argument in arguments {
                collect_expression_calls(argument, symbols, scopes, calls);
            }
        }
        ExprKind::Member { object, .. } => {
            collect_expression_calls(object, symbols, scopes, calls);
        }
        ExprKind::Record { base, fields, .. } => {
            if let Some(base) = base {
                collect_expression_calls(base, symbols, scopes, calls);
            }
            for (_, value) in fields {
                collect_expression_calls(value, symbols, scopes, calls);
            }
        }
        ExprKind::Index { object, index } => {
            collect_expression_calls(object, symbols, scopes, calls);
            collect_expression_calls(index, symbols, scopes, calls);
        }
        ExprKind::Block(block) => collect_block_calls(block, symbols, scopes, calls),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expression_calls(condition, symbols, scopes, calls);
            collect_block_calls(then_branch, symbols, scopes, calls);
            collect_expression_calls(else_branch, symbols, scopes, calls);
        }
        ExprKind::Integer { .. }
        | ExprKind::Float { .. }
        | ExprKind::String(_)
        | ExprKind::Bool(_)
        | ExprKind::Unit => {}
    }
}

fn strongly_connected_components<K>(graph: &BTreeMap<K, Vec<K>>) -> Vec<Vec<K>>
where
    K: Clone + Ord,
{
    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::with_capacity(graph.len());
    for root in graph.keys() {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut stack = vec![(root.clone(), 0usize)];
        while !stack.is_empty() {
            let next = {
                let (node, edge_index) = stack.last_mut().unwrap();
                let neighbors = graph.get(node).map(Vec::as_slice).unwrap_or_default();
                if *edge_index < neighbors.len() {
                    let neighbor = neighbors[*edge_index].clone();
                    *edge_index += 1;
                    Some(neighbor)
                } else {
                    None
                }
            };
            if let Some(neighbor) = next {
                if visited.insert(neighbor.clone()) {
                    stack.push((neighbor, 0));
                }
            } else {
                let (node, _) = stack.pop().unwrap();
                finish_order.push(node);
            }
        }
    }

    let mut reverse = graph
        .keys()
        .cloned()
        .map(|node| (node, Vec::new()))
        .collect::<BTreeMap<K, Vec<K>>>();
    for (node, neighbors) in graph {
        for neighbor in neighbors {
            reverse
                .entry(neighbor.clone())
                .or_default()
                .push(node.clone());
        }
    }
    for neighbors in reverse.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }

    visited.clear();
    let mut components = Vec::new();
    for root in finish_order.into_iter().rev() {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component.push(node.clone());
            if let Some(neighbors) = reverse.get(&node) {
                for neighbor in neighbors.iter().rev() {
                    if visited.insert(neighbor.clone()) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn check_function(
    key: &FunctionKey,
    function: &Function,
    signature: &Signature,
    signatures: &BTreeMap<FunctionKey, Signature>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &mut Vec<RecordInfo>,
    record_keys: &BTreeMap<FunctionKey, usize>,
    types: &mut Vec<TypeInfo>,
) -> (
    Option<ValueType>,
    BTreeMap<Span, ValueType>,
    BTreeMap<Span, FunctionKey>,
    Vec<Diagnostic>,
) {
    let mut checker = FunctionChecker {
        module_id: &key.0,
        type_parameters: signature.type_parameters.clone(),
        reference_targets: BTreeMap::new(),
        propagation_allowed: function.return_type.is_some(),
        signatures,
        module_symbols,
        records,
        record_keys,
        types,
        expected_expr: None,
        loop_depth: 0,
        expected_return: signature.return_type,
        inferred_return: None,
        scopes: vec![BTreeMap::new()],
        expression_types: BTreeMap::new(),
        diagnostics: Vec::new(),
    };
    for (index, parameter) in function.parameters.iter().enumerate() {
        let ty = signature
            .parameters
            .get(index)
            .copied()
            .unwrap_or(ValueType::Error);
        let scope = checker.scopes.last_mut().unwrap();
        if scope.contains_key(&parameter.name.name) {
            checker.diagnostics.push(Diagnostic::error(
                "UBI0011",
                format!("Duplicate parameter: {}", parameter.name.name),
                parameter.name.span.clone(),
            ));
        } else {
            scope.insert(parameter.name.name.clone(), Binding { ty, mutable: false });
        }
    }

    checker.expected_expr = signature.return_type;
    if let Some(body_type) = checker.check_block(&function.body) {
        let completion_span = function
            .body
            .tail
            .as_ref()
            .map_or(&function.body.span, |expression| &expression.span)
            .clone();
        checker.record_return(body_type, completion_span);
    }
    (
        checker.inferred_return,
        checker.expression_types,
        checker.reference_targets,
        checker.diagnostics,
    )
}

#[derive(Clone, Copy)]
struct Binding {
    ty: ValueType,
    mutable: bool,
}

struct FunctionChecker<'a> {
    propagation_allowed: bool,
    type_parameters: Vec<ValueType>,
    reference_targets: BTreeMap<Span, FunctionKey>,
    types: &'a mut Vec<TypeInfo>,
    expected_expr: Option<ValueType>,
    loop_depth: usize,
    module_id: &'a str,
    signatures: &'a BTreeMap<FunctionKey, Signature>,
    module_symbols: &'a BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &'a mut Vec<RecordInfo>,
    record_keys: &'a BTreeMap<FunctionKey, usize>,
    expected_return: Option<ValueType>,
    inferred_return: Option<ValueType>,
    scopes: Vec<BTreeMap<String, Binding>>,
    expression_types: BTreeMap<Span, ValueType>,
    diagnostics: Vec<Diagnostic>,
}

impl FunctionChecker<'_> {
    fn check_block(&mut self, block: &Block) -> Option<ValueType> {
        let expected = self.expected_expr.take();
        self.scopes.push(BTreeMap::new());
        let mut completes = true;
        for statement in &block.statements {
            if !completes {
                break;
            }
            match &statement.kind {
                StatementKind::Let {
                    name,
                    mutable,
                    annotation,
                    value,
                } => {
                    let bound_type = annotation.as_ref().map(|ty| {
                        type_from_syntax(
                            ty,
                            self.module_id,
                            self.module_symbols,
                            self.record_keys,
                            &self.type_parameters,
                            self.records,
                            self.types,
                            &mut self.diagnostics,
                        )
                    });
                    let value_type = self.check_expr_expected(value, bound_type);
                    let Some(value_type) = value_type else {
                        completes = false;
                        continue;
                    };
                    if let Some(bound_type) = bound_type {
                        self.require_same_type(
                            bound_type,
                            value_type,
                            &value.span,
                            "Binding initializer has the wrong type",
                        );
                    }
                    let scope = self.scopes.last_mut().unwrap();
                    if scope.contains_key(&name.name) {
                        self.diagnostics.push(Diagnostic::error(
                            "UBI0011",
                            format!("Duplicate binding: {}", name.name),
                            name.span.clone(),
                        ));
                    } else {
                        scope.insert(
                            name.name.clone(),
                            Binding {
                                ty: bound_type.unwrap_or(value_type),
                                mutable: *mutable,
                            },
                        );
                    }
                }
                StatementKind::LetPattern {
                    pattern,
                    annotation,
                    value,
                } => {
                    let expected = annotation.as_ref().map(|ty| {
                        type_from_syntax(
                            ty,
                            self.module_id,
                            self.module_symbols,
                            self.record_keys,
                            &self.type_parameters,
                            self.records,
                            self.types,
                            &mut self.diagnostics,
                        )
                    });
                    let actual = self.check_expr_expected(value, expected);
                    let Some(actual) = actual else {
                        completes = false;
                        continue;
                    };
                    if let Some(expected) = expected {
                        self.require_same_type(
                            expected,
                            actual,
                            &value.span,
                            "Binding initializer has the wrong type",
                        );
                    }
                    self.check_let_pattern(pattern, expected.unwrap_or(actual));
                }
                StatementKind::Assign { target, value } => {
                    let target_type = self.check_assignment_target(target);
                    let value_type = self.check_expr_expected(value, target_type);
                    match (target_type, value_type) {
                        (Some(target_type), Some(value_type)) => self.require_same_type(
                            target_type,
                            value_type,
                            &value.span,
                            "Assignment value has the wrong type",
                        ),
                        (_, None) => completes = false,
                        _ => {}
                    }
                }
                StatementKind::Return(value) => {
                    if let Some(value) = value {
                        if let Some(return_type) =
                            self.check_expr_expected(value, self.expected_return)
                        {
                            self.record_return(return_type, value.span.clone());
                        }
                    } else {
                        self.record_return(ValueType::Unit, statement.span.clone());
                    }
                    completes = false;
                }
                StatementKind::Expression(expression) => {
                    completes = self.check_expr(expression).is_some();
                }
                StatementKind::While { condition, body } => {
                    let ty = self.check_expr_expected(condition, Some(ValueType::Bool));
                    if let Some(ty) = ty {
                        self.require_same_type(
                            ValueType::Bool,
                            ty,
                            &condition.span,
                            "While condition must be bool",
                        );
                    }
                    self.loop_depth += 1;
                    if let Some(ty) = self.check_block(body) {
                        self.require_same_type(
                            ValueType::Unit,
                            ty,
                            &body.span,
                            "Loop body must be unit",
                        );
                    }
                    self.loop_depth -= 1;
                    completes &= ty.is_some();
                }
                StatementKind::For {
                    name,
                    iterable,
                    body,
                } => {
                    let ty = self.check_expr(iterable);
                    let item = match ty {
                        Some(ValueType::List(id)) => match self.types[id] {
                            TypeInfo::List(inner) => inner,
                            _ => unreachable!(),
                        },
                        Some(ValueType::String) => ValueType::String,
                        Some(ValueType::Range) => ValueType::Int,
                        _ => {
                            self.error(&iterable.span, "For requires List, string, or range");
                            ValueType::Error
                        }
                    };
                    self.scopes.push(BTreeMap::from([(
                        name.name.clone(),
                        Binding {
                            ty: item,
                            mutable: false,
                        },
                    )]));
                    self.loop_depth += 1;
                    if let Some(ty) = self.check_block(body) {
                        self.require_same_type(
                            ValueType::Unit,
                            ty,
                            &body.span,
                            "Loop body must be unit",
                        );
                    }
                    self.loop_depth -= 1;
                    self.scopes.pop();
                    completes &= ty.is_some();
                }
                StatementKind::Break | StatementKind::Continue => {
                    if self.loop_depth == 0 {
                        self.error(
                            &statement.span,
                            "Break and continue require an enclosing loop in this function",
                        );
                    }
                    completes = false;
                }
            }
        }

        let result = if completes {
            if let Some(tail) = &block.tail {
                self.check_expr_expected(tail, expected)
            } else {
                Some(ValueType::Unit)
            }
        } else {
            None
        };
        self.scopes.pop();
        result
    }

    fn check_expr(&mut self, expression: &Expr) -> Option<ValueType> {
        let result = self.check_expr_inner(expression);
        self.expression_types
            .insert(expression.span.clone(), result.unwrap_or(ValueType::Never));
        result
    }

    fn check_expr_expected(
        &mut self,
        expression: &Expr,
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let previous = self.expected_expr;
        self.expected_expr = expected;
        let result = self.check_expr(expression);
        self.expected_expr = previous;
        result
    }

    fn error(&mut self, span: &Span, message: &str) {
        self.diagnostics
            .push(Diagnostic::error("UBI0020", message, span.clone()));
    }

    fn check_expr_inner(&mut self, expression: &Expr) -> Option<ValueType> {
        let expected = self.expected_expr.take();
        match &expression.kind {
            ExprKind::List(values) => self.check_list(expression, values, expected),
            ExprKind::Tuple(values) => self.check_tuple(values, expected),
            ExprKind::Await(operand) => self.check_expr_expected(operand, expected),
            ExprKind::Arrow { parameters, body } => self.check_arrow(parameters, body, expected),
            ExprKind::Match { subject, arms } => {
                self.check_match(expression, subject, arms, expected)
            }
            ExprKind::Integer {
                value,
                literal_span,
            } => Some(self.check_integer(value, literal_span, false, literal_span)),
            ExprKind::Float {
                value,
                literal_span,
            } => Some(self.check_float(value, literal_span)),
            ExprKind::String(_) => Some(ValueType::String),
            ExprKind::Bool(_) => Some(ValueType::Bool),
            ExprKind::Unit => Some(ValueType::Unit),
            ExprKind::Name(name) => {
                let ty = self.check_name(name, expected);
                if let Some(target) = self.reference_targets.get(&name.span).cloned() {
                    self.reference_targets
                        .insert(expression.span.clone(), target);
                }
                Some(ty)
            }
            ExprKind::Unary { operator, operand } => {
                if *operator == crate::lexer::Symbol::Minus {
                    if let ExprKind::Integer {
                        value,
                        literal_span,
                    } = &operand.kind
                    {
                        if operand.span == *literal_span {
                            let magnitude = value.parse::<u64>().unwrap_or(u64::MAX);
                            if magnitude > i32::MAX as u64 {
                                let ty =
                                    self.check_integer(value, literal_span, true, &expression.span);
                                self.expression_types.insert(operand.span.clone(), ty);
                                return Some(ty);
                            }
                        }
                    }
                    if let ExprKind::Float {
                        value,
                        literal_span,
                    } = &operand.kind
                    {
                        let span = if operand.span == *literal_span {
                            &expression.span
                        } else {
                            literal_span
                        };
                        let ty = self.check_float(value, span);
                        self.expression_types.insert(operand.span.clone(), ty);
                        return Some(ty);
                    }
                }
                let operand_type = self.check_expr(operand)?;
                match (operator, operand_type) {
                    (_, ValueType::Error) => Some(ValueType::Error),
                    (crate::lexer::Symbol::Minus, ty @ (ValueType::Int | ValueType::Float)) => {
                        Some(ty)
                    }
                    (crate::lexer::Symbol::Bang, ValueType::Bool) => Some(ValueType::Bool),
                    (_, ty) => {
                        self.diagnostics.push(Diagnostic::error(
                            "UBI0020",
                            format!("Invalid operand for unary operator: {}", ty.name()),
                            operand.span.clone(),
                        ));
                        Some(ValueType::Error)
                    }
                }
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_type = self.check_expr(left);
                let right_expected = if matches!(
                    operator,
                    crate::lexer::Symbol::EqualEqual | crate::lexer::Symbol::BangEqual
                ) {
                    left_type
                } else {
                    None
                };
                let right_type = self.check_expr_expected(right, right_expected);
                let (Some(left_type), Some(right_type)) = (left_type, right_type) else {
                    return None;
                };
                Some(self.binary_type(*operator, left_type, right_type, left, right))
            }
            ExprKind::Call { callee, arguments } => {
                self.check_call(expression, callee, arguments, expected)
            }
            ExprKind::Member { object, name } => {
                if let Some(id) = self.enum_namespace(object) {
                    let TypeInfo::Enum {
                        arguments: parameters,
                        variants,
                        ..
                    } = &self.types[id]
                    else {
                        unreachable!()
                    };
                    let parameters = parameters.clone();
                    match variants.get(&name.name) {
                        Some(payloads) if payloads.is_empty() => {
                            let mut mapping = BTreeMap::new();
                            if let Some(expected) = expected {
                                if !infer_type_arguments(
                                    ValueType::Enum(id),
                                    expected,
                                    &mut mapping,
                                    self.records,
                                    self.types,
                                ) {
                                    self.error(&expression.span, "Enum has the wrong nominal type");
                                }
                            }
                            if !self.resolved(&parameters, &mapping) {
                                self.error(
                                    &expression.span,
                                    "Payload-free enum variant requires an expected type",
                                );
                                return Some(ValueType::Error);
                            }
                            return Some(self.substitute(
                                ValueType::Enum(id),
                                &mapping,
                                &expression.span,
                            ));
                        }
                        Some(_) => {
                            self.error(&name.span, "Enum payload variant requires arguments")
                        }
                        None => self.error(&name.span, "Unknown enum variant"),
                    }
                    return Some(ValueType::Error);
                }
                if is_option_member(object, "Option") && name.name == "None" {
                    if matches!(expected, Some(ValueType::Option(_))) {
                        return expected;
                    }
                    self.error(
                        &expression.span,
                        "Option.None requires an expected Option type",
                    );
                    return Some(ValueType::Error);
                }
                let ty = self.check_expr(object)?;
                if ty == ValueType::Error {
                    return Some(ty);
                }
                if let ValueType::Record(id) = ty {
                    if let Some(field) = self.records[id].fields.get(&name.name) {
                        return Some(*field);
                    }
                }
                self.diagnostics.push(Diagnostic::error(
                    "UBI0030",
                    "Invalid record field access",
                    name.span.clone(),
                ));
                Some(ValueType::Error)
            }
            ExprKind::Record { name, base, fields } => {
                self.check_record(expression, name, base.as_deref(), fields, expected)
            }
            ExprKind::Index { object, index } => {
                let object_ty = self.check_expr(object);
                let index_ty = self.check_expr(index);
                let (Some(object_ty), Some(index_ty)) = (object_ty, index_ty) else {
                    return None;
                };
                self.require_same_type(ValueType::Int, index_ty, &index.span, "Index must be int");
                let item = match object_ty {
                    ValueType::List(id) => match self.types[id] {
                        TypeInfo::List(inner) => inner,
                        _ => unreachable!(),
                    },
                    ValueType::String => ValueType::String,
                    _ => {
                        self.error(&object.span, "Indexing requires List or string");
                        ValueType::Error
                    }
                };
                Some(intern_type(self.types, TypeInfo::Option(item)))
            }
            ExprKind::Propagate(operand) => {
                let ty = self.check_expr(operand)?;
                let result_args = |ty: ValueType, types: &[TypeInfo]| {
                    if let ValueType::Enum(id) = ty {
                        if let TypeInfo::Enum { key, arguments, .. } = &types[id] {
                            (key.0.is_empty() && key.1 == "Result").then(|| arguments.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                let operand_args = result_args(ty, self.types);
                let return_args = self
                    .expected_return
                    .and_then(|ty| result_args(ty, self.types));
                if let (Some(operand_args), Some(return_args)) = (operand_args, return_args) {
                    if self.propagation_allowed && operand_args[1] == return_args[1] {
                        return Some(operand_args[0]);
                    }
                }
                self.diagnostics.push(Diagnostic::error("UBI0033","Propagation requires a named Result-returning function with identical error type",Span { source_id: expression.span.source_id.clone(), start: expression.span.end-1, end: expression.span.end }));
                Some(ValueType::Error)
            }
            ExprKind::Block(block) => {
                self.expected_expr = expected;
                self.check_block(block)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_type = self.check_expr(condition);
                if let Some(condition_type) = condition_type {
                    self.require_same_type(
                        ValueType::Bool,
                        condition_type,
                        &condition.span,
                        "If condition must be bool",
                    );
                }
                self.expected_expr = expected;
                let then_type = self.check_block(then_branch);
                let else_type = self.check_expr_expected(else_branch, expected.or(then_type));
                condition_type?;
                match (then_type, else_type) {
                    (None, None) => None,
                    (Some(ty), None) | (None, Some(ty)) => Some(ty),
                    (Some(left), Some(right)) => {
                        if left != ValueType::Error && right != ValueType::Error && left != right {
                            self.diagnostics.push(Diagnostic::error(
                                "UBI0020",
                                format!(
                                    "If branches have different types: {} and {}",
                                    left.name(),
                                    right.name()
                                ),
                                else_branch.span.clone(),
                            ));
                            Some(ValueType::Error)
                        } else if left == ValueType::Error || right == ValueType::Error {
                            Some(ValueType::Error)
                        } else {
                            Some(left)
                        }
                    }
                }
            }
        }
    }

    fn check_name(&mut self, name: &Identifier, expected: Option<ValueType>) -> ValueType {
        if let Some(ty) = self.lookup_local(&name.name) {
            return ty;
        }
        let target = self
            .module_symbols
            .get(self.module_id)
            .and_then(|s| s.get(&name.name))
            .cloned();
        if let Some((target, signature)) = target.and_then(|key| {
            self.signatures
                .get(&key)
                .cloned()
                .map(|signature| (key, signature))
        }) {
            let template = intern_type(
                self.types,
                TypeInfo::Function {
                    parameters: signature.parameters.clone(),
                    return_type: signature.return_type.unwrap_or(ValueType::Error),
                },
            );
            let mut mapping = BTreeMap::new();
            if let Some(expected) = expected {
                if !infer_type_arguments(template, expected, &mut mapping, self.records, self.types)
                {
                    self.error(&name.span, "Function reference has the wrong type");
                }
            }
            if !signature.type_parameters.is_empty()
                && !self.resolved(&signature.type_parameters, &mapping)
            {
                self.error(
                    &name.span,
                    "Generic function requires an established function type",
                );
                return ValueType::Error;
            }
            let result = self.substitute(template, &mapping, &name.span);
            self.reference_targets.insert(name.span.clone(), target);
            return result;
        }
        self.diagnostics.push(Diagnostic::error(
            "UBI0010",
            format!("Unknown name: {}", name.name),
            name.span.clone(),
        ));
        ValueType::Error
    }

    fn substitute(
        &mut self,
        ty: ValueType,
        mapping: &BTreeMap<usize, ValueType>,
        span: &Span,
    ) -> ValueType {
        match substitute_type(ty, mapping, self.records, self.types) {
            Ok(value) => value,
            Err(error) => {
                self.diagnostics.push(substitution_diagnostic(error, span));
                ValueType::Error
            }
        }
    }

    fn substitute_for_hint(
        &mut self,
        ty: ValueType,
        mapping: &BTreeMap<usize, ValueType>,
        span: &Span,
    ) -> ValueType {
        match substitute_type_unchecked(ty, mapping, self.records, self.types) {
            Ok(value) => value,
            Err(error) => {
                self.diagnostics.push(substitution_diagnostic(error, span));
                ValueType::Error
            }
        }
    }

    fn resolved(&self, parameters: &[ValueType], mapping: &BTreeMap<usize, ValueType>) -> bool {
        parameters
            .iter()
            .all(|p| matches!(p,ValueType::Parameter(id) if mapping.contains_key(id)))
    }

    fn hint(
        &mut self,
        ty: ValueType,
        parameters: &[ValueType],
        mapping: &BTreeMap<usize, ValueType>,
        span: &Span,
    ) -> Option<ValueType> {
        let value = self.substitute_for_hint(ty, mapping, span);
        let unresolved: BTreeSet<_> = parameters
            .iter()
            .filter_map(|p| {
                if let ValueType::Parameter(id) = p {
                    (!mapping.contains_key(id)).then_some(*id)
                } else {
                    None
                }
            })
            .collect();
        (!contains_parameters(value, &unresolved, self.records, self.types)).then_some(value)
    }

    fn generic_values(
        &mut self,
        values: &[&Expr],
        templates: &[ValueType],
        parameters: &[ValueType],
        mapping: &mut BTreeMap<usize, ValueType>,
    ) -> bool {
        let mut completes = true;
        let mut pending: Vec<usize> = (0..values.len()).collect();
        loop {
            let mut deferred = Vec::new();
            for index in &pending {
                let value = values[*index];
                let hint = templates
                    .get(*index)
                    .and_then(|ty| self.hint(*ty, parameters, mapping, &value.span));
                let speculative = hint.is_none() && needs_context(value);
                let diagnostic_count = self.diagnostics.len();
                let old_types = speculative.then(|| self.expression_types.clone());
                let old_references = speculative.then(|| self.reference_targets.clone());
                let actual = self.check_expr_expected(value, hint);
                if speculative && self.diagnostics.len() > diagnostic_count {
                    self.diagnostics.truncate(diagnostic_count);
                    self.expression_types = old_types.unwrap();
                    self.reference_targets = old_references.unwrap();
                    deferred.push(*index);
                    continue;
                }
                if let (Some(template), Some(actual)) = (templates.get(*index), actual) {
                    if !infer_type_arguments(*template, actual, mapping, self.records, self.types) {
                        self.error(&value.span, "Conflicting generic type arguments");
                    }
                }
                completes &= actual.is_some();
            }
            if deferred.is_empty() {
                break;
            }
            if deferred.len() == pending.len() {
                for index in deferred {
                    let value = values[index];
                    let hint = templates
                        .get(index)
                        .and_then(|ty| self.hint(*ty, parameters, mapping, &value.span));
                    let actual = self.check_expr_expected(value, hint);
                    if let (Some(template), Some(actual)) = (templates.get(index), actual) {
                        if !infer_type_arguments(
                            *template,
                            actual,
                            mapping,
                            self.records,
                            self.types,
                        ) {
                            self.error(&value.span, "Conflicting generic type arguments");
                        }
                    }
                    completes &= actual.is_some();
                }
                break;
            }
            pending = deferred;
        }
        completes
    }

    fn check_record(
        &mut self,
        expression: &Expr,
        name: &Identifier,
        base: Option<&Expr>,
        fields: &[(Identifier, Expr)],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let id = if self.lookup_local(&name.name).is_none() {
            self.module_symbols
                .get(self.module_id)
                .and_then(|s| s.get(&name.name))
                .and_then(|k| self.record_keys.get(k))
                .copied()
        } else {
            None
        };
        let Some(id) = id else {
            self.diagnostics.push(Diagnostic::error(
                "UBI0010",
                format!("Unknown record: {}", name.name),
                name.span.clone(),
            ));
            return Some(ValueType::Error);
        };
        let record = self.records[id].clone();
        let mut mapping = BTreeMap::new();
        if let Some(expected) = expected {
            if !infer_type_arguments(
                ValueType::Record(id),
                expected,
                &mut mapping,
                self.records,
                self.types,
            ) {
                self.error(&expression.span, "Record has the wrong nominal type");
            }
        }
        let mut completes = true;
        if let Some(base) = base {
            let hint = self.hint(
                ValueType::Record(id),
                &record.arguments,
                &mapping,
                &base.span,
            );
            let actual = self.check_expr_expected(base, hint);
            if let Some(actual) = actual {
                if !infer_type_arguments(
                    ValueType::Record(id),
                    actual,
                    &mut mapping,
                    self.records,
                    self.types,
                ) {
                    self.error(&base.span, "Record update base has the wrong type");
                }
            }
            completes &= actual.is_some();
        }
        let mut seen = BTreeSet::new();
        let mut values = Vec::new();
        let mut templates = Vec::new();
        for (field, value) in fields {
            if !seen.insert(field.name.clone()) || !record.fields.contains_key(&field.name) {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0030",
                    "Unknown or duplicate record field",
                    field.span.clone(),
                ));
            }
            values.push(value);
            templates.push(
                record
                    .fields
                    .get(&field.name)
                    .copied()
                    .unwrap_or(ValueType::Error),
            );
        }
        completes &= self.generic_values(&values, &templates, &record.arguments, &mut mapping);
        if base.is_none() && record.fields.keys().any(|field| !seen.contains(field)) {
            self.diagnostics.push(Diagnostic::error(
                "UBI0030",
                "Missing record field",
                expression.span.clone(),
            ));
        }
        if !self.resolved(&record.arguments, &mapping) {
            self.error(&expression.span, "Unresolved record type arguments");
            return Some(ValueType::Error);
        }
        completes.then(|| self.substitute(ValueType::Record(id), &mapping, &expression.span))
    }

    fn check_call(
        &mut self,
        call: &Expr,
        callee: &Expr,
        arguments: &[Expr],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        if let ExprKind::Name(name) = &callee.kind {
            if self.lookup_local(&name.name).is_none() {
                if let Some((target, signature)) = self
                    .module_symbols
                    .get(self.module_id)
                    .and_then(|s| s.get(&name.name))
                    .and_then(|key| self.signatures.get(key).map(|s| (key.clone(), s.clone())))
                {
                    if !signature.type_parameters.is_empty() {
                        self.arity(call, arguments, signature.parameters.len());
                        let mut mapping = BTreeMap::new();
                        if let (Some(template), Some(actual)) = (signature.return_type, expected) {
                            if !infer_type_arguments(
                                template,
                                actual,
                                &mut mapping,
                                self.records,
                                self.types,
                            ) {
                                self.error(&call.span, "Generic result has the wrong type");
                            }
                        }
                        let completes = self.generic_values(
                            &arguments.iter().collect::<Vec<_>>(),
                            &signature.parameters,
                            &signature.type_parameters,
                            &mut mapping,
                        );
                        if !self.resolved(&signature.type_parameters, &mapping) {
                            self.error(&call.span, "Unresolved function type arguments");
                            return Some(ValueType::Error);
                        }
                        let template = intern_type(
                            self.types,
                            TypeInfo::Function {
                                parameters: signature.parameters.clone(),
                                return_type: signature.return_type.unwrap_or(ValueType::Error),
                            },
                        );
                        let function_type = self.substitute(template, &mapping, &callee.span);
                        self.expression_types
                            .insert(callee.span.clone(), function_type);
                        self.reference_targets.insert(callee.span.clone(), target);
                        let return_type = match function_type {
                            ValueType::Function(id) => match self.types[id] {
                                TypeInfo::Function { return_type, .. } => return_type,
                                _ => unreachable!(),
                            },
                            _ => ValueType::Error,
                        };
                        return completes.then_some(return_type);
                    }
                }
            }
        }
        if let ExprKind::Member { object, name } = &callee.kind {
            if let Some(id) = self.enum_namespace(object) {
                let TypeInfo::Enum {
                    arguments: parameters,
                    variants,
                    ..
                } = self.types[id].clone()
                else {
                    unreachable!()
                };
                let Some(payloads) = variants.get(&name.name) else {
                    self.error(&name.span, "Unknown enum variant");
                    for value in arguments {
                        self.check_expr(value);
                    }
                    return Some(ValueType::Error);
                };
                if payloads.is_empty() {
                    self.error(&callee.span, "Payload-free enum variant cannot be called");
                }
                self.arity(call, arguments, payloads.len());
                let mut mapping = BTreeMap::new();
                if let Some(expected) = expected {
                    if !infer_type_arguments(
                        ValueType::Enum(id),
                        expected,
                        &mut mapping,
                        self.records,
                        self.types,
                    ) {
                        self.error(&call.span, "Enum result has the wrong type");
                    }
                }
                let completes = self.generic_values(
                    &arguments.iter().collect::<Vec<_>>(),
                    payloads,
                    &parameters,
                    &mut mapping,
                );
                if !self.resolved(&parameters, &mapping) {
                    self.error(&call.span, "Unresolved enum type arguments");
                    return Some(ValueType::Error);
                }
                return completes
                    .then(|| self.substitute(ValueType::Enum(id), &mapping, &call.span));
            }
            if is_option_member(object, "Option") && name.name == "Some" {
                self.arity(call, arguments, 1);
                let inner_expected = expected.and_then(|ty| self.option_inner(ty));
                let Some(value) = arguments.first() else {
                    return Some(ValueType::Error);
                };
                let inner = self.check_expr_expected(value, inner_expected)?;
                if !self.aggregate_allowed(inner, &value.span) {
                    return Some(ValueType::Error);
                }
                for value in arguments.iter().skip(1) {
                    self.check_expr(value);
                }
                return Some(intern_type(self.types, TypeInfo::Option(inner)));
            }
        }
        if let ExprKind::Name(name) = &callee.kind {
            if self.lookup_local(&name.name).is_none() && builtin_name(&name.name) {
                return self.check_builtin(call, &name.name, arguments, expected);
            }
        }
        let callee_ty = self.check_expr(callee);
        let Some(callee_ty) = callee_ty else {
            for argument in arguments {
                self.check_expr(argument);
            }
            return None;
        };
        let ValueType::Function(id) = callee_ty else {
            for argument in arguments {
                self.check_expr(argument);
            }
            if callee_ty != ValueType::Error {
                self.error(&callee.span, "Call requires a function value");
            }
            return Some(ValueType::Error);
        };
        let TypeInfo::Function {
            parameters,
            return_type,
        } = self.types[id].clone()
        else {
            unreachable!()
        };
        self.arity(call, arguments, parameters.len());
        let mut completes = true;
        for (index, argument) in arguments.iter().enumerate() {
            let expected = parameters.get(index).copied();
            let actual = self.check_expr_expected(argument, expected);
            if let (Some(expected), Some(actual)) = (expected, actual) {
                self.require_same_type(
                    expected,
                    actual,
                    &argument.span,
                    "Function argument has the wrong type",
                );
            }
            completes &= actual.is_some();
        }
        completes.then_some(return_type)
    }

    fn arity(&mut self, call: &Expr, arguments: &[Expr], count: usize) {
        if arguments.len() != count {
            self.diagnostics.push(Diagnostic::error(
                "UBI0023",
                format!("Expected {count} arguments, found {}", arguments.len()),
                call.span.clone(),
            ));
        }
    }

    fn enum_namespace(&self, object: &Expr) -> Option<usize> {
        let ExprKind::Name(name) = &object.kind else {
            return None;
        };
        if self.lookup_local(&name.name).is_some() {
            return None;
        }
        self.enum_id(&name.name)
    }

    fn enum_id(&self, name: &str) -> Option<usize> {
        if matches!(name, "Result" | "HostError") {
            return enum_type(self.types, &(String::new(), name.to_owned()));
        }
        if name == "Result" {
            return enum_type(self.types, &(String::new(), "Result".to_owned()));
        }
        self.module_symbols
            .get(self.module_id)
            .and_then(|symbols| symbols.get(name))
            .and_then(|key| enum_type(self.types, key))
    }

    fn option_inner(&self, ty: ValueType) -> Option<ValueType> {
        if let ValueType::Option(id) = ty {
            if let TypeInfo::Option(inner) = self.types[id] {
                return Some(inner);
            }
        }
        None
    }

    fn list_inner(&self, ty: ValueType) -> Option<ValueType> {
        if let ValueType::List(id) = ty {
            if let TypeInfo::List(inner) = self.types[id] {
                return Some(inner);
            }
        }
        None
    }

    fn aggregate_allowed(&mut self, ty: ValueType, span: &Span) -> bool {
        if ty == ValueType::Range {
            self.error(span, "Range values cannot be stored in aggregates");
            false
        } else {
            true
        }
    }

    fn check_tuple(&mut self, values: &[Expr], expected: Option<ValueType>) -> Option<ValueType> {
        let expected_elements = expected.and_then(|ty| {
            if let ValueType::Tuple(id) = ty {
                if let TypeInfo::Tuple(elements) = &self.types[id] {
                    return Some(elements.clone());
                }
            }
            None
        });
        let mut elements = Vec::new();
        let mut completes = true;
        for (index, value) in values.iter().enumerate() {
            let expected = expected_elements
                .as_ref()
                .and_then(|elements| elements.get(index))
                .copied();
            if let Some(actual) = self.check_expr_expected(value, expected) {
                self.aggregate_allowed(actual, &value.span);
                if let Some(expected) = expected {
                    self.require_same_type(
                        expected,
                        actual,
                        &value.span,
                        "Tuple element has the wrong type",
                    );
                }
                elements.push(actual);
            } else {
                completes = false;
            }
        }
        completes.then(|| intern_type(self.types, TypeInfo::Tuple(elements)))
    }

    fn check_list(
        &mut self,
        expression: &Expr,
        values: &[Expr],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let mut inner = expected.and_then(|ty| self.list_inner(ty));
        let mut completes = true;
        for value in values {
            let actual = self.check_expr_expected(value, inner);
            if let Some(actual) = actual {
                self.aggregate_allowed(actual, &value.span);
                if let Some(expected) = inner {
                    self.require_same_type(
                        expected,
                        actual,
                        &value.span,
                        "List elements must have the same type",
                    );
                } else {
                    inner = Some(actual);
                }
            } else {
                completes = false;
            }
        }
        let Some(inner) = inner else {
            self.error(
                &expression.span,
                "Empty list requires an expected List type",
            );
            return Some(ValueType::Error);
        };
        completes.then(|| intern_type(self.types, TypeInfo::List(inner)))
    }

    fn check_arrow(
        &mut self,
        parameters: &[Parameter],
        body: &Expr,
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let previous_scopes = self.scopes.clone();
        for scope in &mut self.scopes {
            for binding in scope.values_mut() {
                binding.mutable = false;
            }
        }
        self.scopes.push(BTreeMap::new());
        let mut parameter_types = Vec::new();
        for parameter in parameters {
            let ty = type_from_syntax(
                &parameter.ty,
                self.module_id,
                self.module_symbols,
                self.record_keys,
                &self.type_parameters,
                self.records,
                self.types,
                &mut self.diagnostics,
            );
            parameter_types.push(ty);
            if self
                .scopes
                .last_mut()
                .unwrap()
                .insert(parameter.name.name.clone(), Binding { ty, mutable: false })
                .is_some()
            {
                self.error(&parameter.name.span, "Duplicate closure parameter");
            }
        }
        let previous_propagation = std::mem::replace(&mut self.propagation_allowed, false);
        let previous_return = self.expected_return.take();
        let previous_inferred = self.inferred_return.take();
        let previous_loop = std::mem::replace(&mut self.loop_depth, 0);
        let expected_return = expected.and_then(|ty| match ty {
            ValueType::Function(id) => match self.types[id] {
                TypeInfo::Function { return_type, .. } => Some(return_type),
                _ => None,
            },
            _ => None,
        });
        self.expected_return = expected_return;
        if let Some(ty) = self.check_expr_expected(body, expected_return) {
            self.record_return(ty, body.span.clone());
        }
        let return_type = expected_return
            .or(self.inferred_return)
            .unwrap_or(ValueType::Error);
        self.propagation_allowed = previous_propagation;
        self.expected_return = previous_return;
        self.inferred_return = previous_inferred;
        self.loop_depth = previous_loop;
        self.scopes = previous_scopes;
        Some(intern_type(
            self.types,
            TypeInfo::Function {
                parameters: parameter_types,
                return_type,
            },
        ))
    }

    fn check_match(
        &mut self,
        expression: &Expr,
        subject: &Expr,
        arms: &[MatchArm],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let subject_type = self.check_expr(subject);
        let subject_ty = subject_type.unwrap_or(ValueType::Error);
        let mut result = expected;
        let mut completes = false;
        for arm in arms {
            self.scopes.push(BTreeMap::new());
            self.check_pattern(&arm.pattern, subject_ty);
            if let Some(guard) = &arm.guard {
                if let Some(ty) = self.check_expr(guard) {
                    self.require_same_type(
                        ValueType::Bool,
                        ty,
                        &guard.span,
                        "Match guard must be bool",
                    );
                }
            }
            if let Some(ty) = self.check_expr_expected(&arm.body, result) {
                if let Some(expected) = result {
                    self.require_same_type(
                        expected,
                        ty,
                        &arm.body.span,
                        "Match arms must have the same type",
                    );
                } else {
                    result = Some(ty);
                }
                completes = true;
            }
            self.scopes.pop();
        }
        let patterns: Vec<&Pattern> = arms
            .iter()
            .filter(|arm| arm.guard.is_none())
            .map(|arm| &arm.pattern)
            .collect();
        if subject_ty != ValueType::Error {
            match patterns_exhaustive(subject_ty, &patterns, self.types) {
                Ok(true) => {}
                Ok(false) => self.error(&expression.span, "Match patterns are not exhaustive"),
                Err(()) => self.diagnostics.push(Diagnostic::error(
                    "UBI0090",
                    "Match coverage resource limit exceeded",
                    expression.span.clone(),
                )),
            }
        }
        if completes && subject_type.is_some() {
            result
        } else {
            None
        }
    }

    fn check_let_pattern(&mut self, pattern: &Pattern, expected: ValueType) {
        match &pattern.kind {
            PatternKind::Wildcard => {}
            PatternKind::Binding(name) => {
                let scope = self.scopes.last_mut().unwrap();
                if scope.contains_key(&name.name) {
                    self.diagnostics.push(Diagnostic::error(
                        "UBI0011",
                        "Duplicate pattern binding",
                        name.span.clone(),
                    ));
                } else {
                    scope.insert(
                        name.name.clone(),
                        Binding {
                            ty: expected,
                            mutable: false,
                        },
                    );
                }
            }
            PatternKind::Tuple(patterns) => {
                let elements = match expected {
                    ValueType::Unit if patterns.is_empty() => Some(Vec::new()),
                    ValueType::Tuple(id) => match &self.types[id] {
                        TypeInfo::Tuple(elements) if elements.len() == patterns.len() => {
                            Some(elements.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(elements) = elements {
                    for (pattern, ty) in patterns.iter().zip(elements) {
                        self.check_let_pattern(pattern, ty);
                    }
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        "UBI0031",
                        "Tuple pattern requires a tuple of the same arity",
                        pattern.span.clone(),
                    ));
                }
            }
            _ => self.diagnostics.push(Diagnostic::error(
                "UBI0031",
                "Let patterns must be irrefutable names, wildcards or tuples",
                pattern.span.clone(),
            )),
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, expected: ValueType) {
        match &pattern.kind {
            PatternKind::Tuple(patterns) => {
                let elements = match expected {
                    ValueType::Unit if patterns.is_empty() => Some(Vec::new()),
                    ValueType::Tuple(id) => match &self.types[id] {
                        TypeInfo::Tuple(elements) if elements.len() == patterns.len() => {
                            Some(elements.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(elements) = elements {
                    for (pattern, ty) in patterns.iter().zip(elements) {
                        self.check_pattern(pattern, ty);
                    }
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        "UBI0031",
                        "Tuple pattern requires a tuple of the same arity",
                        pattern.span.clone(),
                    ));
                }
            }
            PatternKind::Wildcard => {}
            PatternKind::Binding(name) => {
                if self
                    .scopes
                    .last_mut()
                    .unwrap()
                    .insert(
                        name.name.clone(),
                        Binding {
                            ty: expected,
                            mutable: false,
                        },
                    )
                    .is_some()
                {
                    self.error(&name.span, "Duplicate pattern binding");
                }
            }
            PatternKind::Integer(raw) => {
                let valid = raw.parse::<i32>().is_ok();
                if !valid {
                    self.error(
                        &pattern.span,
                        "Pattern integer is outside signed 32-bit range",
                    );
                }
                self.require_same_type(
                    ValueType::Int,
                    expected,
                    &pattern.span,
                    "Pattern has the wrong type",
                );
            }
            PatternKind::String(_) => self.require_same_type(
                ValueType::String,
                expected,
                &pattern.span,
                "Pattern has the wrong type",
            ),
            PatternKind::Bool(_) => self.require_same_type(
                ValueType::Bool,
                expected,
                &pattern.span,
                "Pattern has the wrong type",
            ),
            PatternKind::Some(inner) => {
                if let Some(ty) = self.option_inner(expected) {
                    self.check_pattern(inner, ty);
                } else {
                    self.error(&pattern.span, "Option pattern requires Option subject");
                }
            }
            PatternKind::None => {
                if self.option_inner(expected).is_none() {
                    self.error(&pattern.span, "Option pattern requires Option subject");
                }
            }
            PatternKind::Variant {
                qualifier,
                name,
                payloads,
                called,
            } => {
                let template = self.enum_id(&qualifier.name);
                let id = if let (Some(template), ValueType::Enum(actual)) = (template, expected) {
                    let (TypeInfo::Enum { key: a, .. }, TypeInfo::Enum { key: b, .. }) =
                        (&self.types[template], &self.types[actual])
                    else {
                        unreachable!()
                    };
                    (a == b).then_some(actual)
                } else {
                    None
                };
                if id.is_none() {
                    self.error(&qualifier.span, "Enum pattern has the wrong nominal type");
                }
                let declared = id.and_then(|id| {
                    if let TypeInfo::Enum { variants, .. } = &self.types[id] {
                        variants.get(&name.name).cloned()
                    } else {
                        None
                    }
                });
                match &declared {
                    Some(types)
                        if types.len() == payloads.len() && *called == !types.is_empty() => {}
                    Some(_) => {
                        self.error(&pattern.span, "Enum pattern has the wrong payload arity")
                    }
                    None => self.error(&name.span, "Unknown enum pattern variant"),
                }
                for (index, payload) in payloads.iter().enumerate() {
                    let ty = declared
                        .as_ref()
                        .and_then(|types| types.get(index))
                        .copied()
                        .unwrap_or(ValueType::Error);
                    self.check_pattern(payload, ty);
                }
            }
        }
    }

    fn check_builtin(
        &mut self,
        call: &Expr,
        name: &str,
        arguments: &[Expr],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        use ValueType::{Bool, Error, Float, Int, Range, String};
        if name.starts_with("map") && name != "map" {
            return self.check_map_builtin(call, name, arguments, expected);
        }
        if crate::effects::host_effect(name).is_some() {
            return self.check_host_builtin(call, name, arguments);
        }
        let count = match name {
            "range" | "append" | "map" | "filter" | "find" | "contains" | "startsWith"
            | "endsWith" | "split" | "join" | "min" | "max" | "pow" => 2,
            "set" | "fold" | "replace" | "slice" | "clamp" => 3,
            _ => 1,
        };
        self.arity(call, arguments, count);
        if arguments.len() != count {
            for arg in arguments {
                self.check_expr(arg);
            }
            return Some(Error);
        }
        let list_expected = if matches!(name, "append" | "filter") {
            expected
        } else if name == "set" {
            expected.and_then(|ty| self.option_inner(ty))
        } else if name == "join" {
            Some(intern_type(self.types, TypeInfo::List(String)))
        } else {
            None
        };
        let mut checked = Vec::new();
        let mut completes = true;
        for (index, arg) in arguments.iter().enumerate() {
            let hint = if index == 0 {
                list_expected
            } else if name == "append" || (name == "set" && index == 2) {
                checked.first().copied().and_then(|ty| self.list_inner(ty))
            } else if name == "fold" && index == 1 {
                expected
            } else if (matches!(name, "map" | "filter" | "find") && index == 1)
                || (name == "fold" && index == 2)
            {
                let item = checked.first().copied().and_then(|ty| self.list_inner(ty));
                let callback_return = if name == "map" {
                    expected.and_then(|ty| self.list_inner(ty)).or_else(|| {
                        // A named generic callback can infer its output from its
                        // established inputs. Error means no output constraint.
                        matches!(&arg.kind, ExprKind::Name(_)).then_some(Error)
                    })
                } else if name == "fold" {
                    checked.get(1).copied()
                } else {
                    Some(Bool)
                };
                match (item, callback_return) {
                    (Some(item), Some(return_type)) => {
                        let parameters = if name == "fold" {
                            vec![checked[1], item]
                        } else {
                            vec![item]
                        };
                        Some(intern_type(
                            self.types,
                            TypeInfo::Function {
                                parameters,
                                return_type,
                            },
                        ))
                    }
                    _ => None,
                }
            } else {
                None
            };
            let ty = self.check_expr_expected(arg, hint);
            completes &= ty.is_some();
            checked.push(ty.unwrap_or(Error));
        }
        if !completes {
            return None;
        }
        let a = checked[0];
        let item = self.list_inner(a);
        let check = |this: &mut Self, index: usize, ty: ValueType| {
            this.require_same_type(
                ty,
                checked[index],
                &arguments[index].span,
                "Prelude argument has the wrong type",
            )
        };
        let result = match name {
            "range" => {
                check(self, 0, Int);
                check(self, 1, Int);
                Range
            }
            "length" => {
                if a != String && item.is_none() && !matches!(a, ValueType::Map(_)) && a != Error {
                    self.error(&arguments[0].span, "Length requires string, List, or Map");
                }
                Int
            }
            "append" | "set" => {
                let Some(item) = item else {
                    self.error(&arguments[0].span, "Collection helper requires List");
                    return Some(Error);
                };
                if name == "append" {
                    check(self, 1, item);
                    a
                } else {
                    check(self, 1, Int);
                    check(self, 2, item);
                    intern_type(self.types, TypeInfo::Option(a))
                }
            }
            "map" | "filter" | "find" | "fold" => {
                let Some(item) = item else {
                    self.error(&arguments[0].span, "Collection helper requires List");
                    return Some(Error);
                };
                let index = if name == "fold" { 2 } else { 1 };
                let ValueType::Function(id) = checked[index] else {
                    self.error(&arguments[index].span, "Callback must be a function");
                    return Some(Error);
                };
                let TypeInfo::Function {
                    parameters,
                    return_type,
                } = self.types[id].clone()
                else {
                    unreachable!()
                };
                let required = if name == "fold" {
                    vec![checked[1], item]
                } else {
                    vec![item]
                };
                if parameters != required {
                    self.error(
                        &arguments[index].span,
                        "Callback parameters have the wrong types",
                    );
                }
                if name == "filter" || name == "find" {
                    self.require_same_type(
                        Bool,
                        return_type,
                        &arguments[index].span,
                        "Predicate must return bool",
                    );
                }
                if name == "fold" {
                    self.require_same_type(
                        checked[1],
                        return_type,
                        &arguments[index].span,
                        "Fold callback must return accumulator type",
                    );
                    checked[1]
                } else if name == "filter" {
                    a
                } else if name == "find" {
                    intern_type(self.types, TypeInfo::Option(item))
                } else {
                    self.aggregate_allowed(return_type, &arguments[index].span);
                    intern_type(self.types, TypeInfo::List(return_type))
                }
            }
            "contains" | "startsWith" | "endsWith" => {
                check(self, 0, String);
                check(self, 1, String);
                Bool
            }
            "trim" => {
                check(self, 0, String);
                String
            }
            "split" => {
                check(self, 0, String);
                check(self, 1, String);
                intern_type(self.types, TypeInfo::List(String))
            }
            "join" => {
                let ty = intern_type(self.types, TypeInfo::List(String));
                check(self, 0, ty);
                check(self, 1, String);
                String
            }
            "replace" => {
                for i in 0..3 {
                    check(self, i, String);
                }
                String
            }
            "slice" => {
                check(self, 0, String);
                check(self, 1, Int);
                check(self, 2, Int);
                String
            }
            "abs" | "min" | "max" | "clamp" => {
                if !matches!(a, Int | Float | Error) {
                    self.error(&arguments[0].span, "Math requires int or float");
                }
                for i in 1..count {
                    check(self, i, a);
                }
                a
            }
            "floor" | "ceil" | "round" | "sqrt" | "sin" | "cos" | "tan" | "log" | "exp" | "pow" => {
                for i in 0..count {
                    check(self, i, Float);
                }
                Float
            }
            "toFloat" => {
                check(self, 0, Int);
                Float
            }
            "toInt" => {
                check(self, 0, Float);
                intern_type(self.types, TypeInfo::Option(Int))
            }
            "parseInt" => {
                check(self, 0, String);
                intern_type(self.types, TypeInfo::Option(Int))
            }
            "parseFloat" => {
                check(self, 0, String);
                intern_type(self.types, TypeInfo::Option(Float))
            }
            "toString" => {
                if !matches!(a, Int | Bool | String | Error) {
                    self.error(&arguments[0].span, "ToString accepts int, bool, or string");
                }
                String
            }
            _ => unreachable!(),
        };
        Some(result)
    }
    fn check_assignment_target(&mut self, target: &Expr) -> Option<ValueType> {
        let ExprKind::Name(name) = &target.kind else {
            return Some(ValueType::Error);
        };
        if let Some(ty) = self.lookup_local(&name.name) {
            if !self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(&name.name))
                .unwrap()
                .mutable
            {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0022",
                    format!("Cannot assign to immutable binding: {}", name.name),
                    name.span.clone(),
                ));
            }
            return Some(ty);
        }
        if self
            .module_symbols
            .get(self.module_id)
            .is_some_and(|symbols| symbols.contains_key(&name.name))
        {
            self.diagnostics.push(Diagnostic::error(
                "UBI0022",
                format!("Cannot assign to immutable declaration: {}", name.name),
                name.span.clone(),
            ));
            return Some(ValueType::Error);
        }
        self.diagnostics.push(Diagnostic::error(
            "UBI0010",
            format!("Unknown name: {}", name.name),
            name.span.clone(),
        ));
        None
    }

    fn lookup_local(&self, name: &str) -> Option<ValueType> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(|binding| binding.ty))
    }

    fn binary_type(
        &mut self,
        operator: crate::lexer::Symbol,
        left_type: ValueType,
        right_type: ValueType,
        left: &Expr,
        right: &Expr,
    ) -> ValueType {
        use crate::lexer::Symbol;
        use ValueType::{Bool, Error, Float, Int, String};

        if left_type == Error || right_type == Error {
            return Error;
        }
        match operator {
            Symbol::AndAnd | Symbol::OrOr => {
                self.require_same_type(Bool, left_type, &left.span, "Boolean operator needs bool");
                self.require_same_type(
                    Bool,
                    right_type,
                    &right.span,
                    "Boolean operator needs bool",
                );
                if left_type == Bool && right_type == Bool {
                    Bool
                } else {
                    Error
                }
            }
            Symbol::EqualEqual | Symbol::BangEqual => {
                if !is_equatable(left_type, self.records, self.types)
                    || !is_equatable(right_type, self.records, self.types)
                {
                    self.error(&right.span, "Operands must have equatable types");
                    Error
                } else if left_type != right_type {
                    self.mismatch(
                        right,
                        left_type,
                        right_type,
                        "Equality operands must have the same type",
                    );
                    Error
                } else {
                    Bool
                }
            }
            Symbol::Less | Symbol::LessEqual | Symbol::Greater | Symbol::GreaterEqual => {
                if left_type != right_type {
                    self.mismatch(
                        right,
                        left_type,
                        right_type,
                        "Ordering operands must have the same numeric type",
                    );
                    Error
                } else if matches!(left_type, Int | Float) {
                    Bool
                } else {
                    self.mismatch(
                        right,
                        left_type,
                        right_type,
                        "Ordering requires numeric operands",
                    );
                    Error
                }
            }
            Symbol::Plus if left_type == String && right_type == String => String,
            Symbol::Plus | Symbol::Minus | Symbol::Star | Symbol::Slash | Symbol::Percent => {
                if left_type != right_type {
                    self.mismatch(
                        right,
                        left_type,
                        right_type,
                        "Arithmetic operands must have the same type",
                    );
                    return Error;
                }
                let valid = match operator {
                    Symbol::Percent => left_type == Int,
                    Symbol::Plus | Symbol::Minus | Symbol::Star | Symbol::Slash => {
                        matches!(left_type, Int | Float)
                    }
                    _ => false,
                };
                if valid {
                    left_type
                } else {
                    self.mismatch(right, left_type, right_type, "Invalid arithmetic operand");
                    Error
                }
            }
            _ => Error,
        }
    }

    fn require_same_type(
        &mut self,
        expected: ValueType,
        actual: ValueType,
        span: &Span,
        message: &str,
    ) {
        if expected != ValueType::Error && actual != ValueType::Error && expected != actual {
            self.diagnostics.push(Diagnostic::error(
                "UBI0020",
                format!(
                    "{message}: expected {}, found {}",
                    expected.name(),
                    actual.name()
                ),
                span.clone(),
            ));
        }
    }

    fn check_map_builtin(
        &mut self,
        call: &Expr,
        name: &str,
        arguments: &[Expr],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        let count = match name {
            "mapEmpty" => 0,
            "mapSet" => 3,
            "mapGet" | "mapHas" | "mapRemove" => 2,
            _ => 1,
        };
        self.arity(call, arguments, count);
        if arguments.len() != count {
            for argument in arguments {
                self.check_expr(argument);
            }
            return Some(ValueType::Error);
        }
        if name == "mapEmpty" {
            if matches!(expected, Some(ValueType::Map(_))) {
                return expected;
            }
            self.error(&call.span, "Empty map requires an expected Map type");
            return Some(ValueType::Error);
        }
        let key = intern_type(
            self.types,
            TypeInfo::Parameter {
                key: (String::new(), "$map".to_owned()),
                name: "K".to_owned(),
                equatable: false,
            },
        );
        let value = intern_type(
            self.types,
            TypeInfo::Parameter {
                key: (String::new(), "$map".to_owned()),
                name: "V".to_owned(),
                equatable: false,
            },
        );
        let parameters = [key, value];
        let map = intern_type(self.types, TypeInfo::Map { key, value });
        let result = match name {
            "mapGet" => intern_type(self.types, TypeInfo::Option(value)),
            "mapHas" => ValueType::Bool,
            "mapKeys" => intern_type(self.types, TypeInfo::List(key)),
            "mapValues" => intern_type(self.types, TypeInfo::List(value)),
            _ => map,
        };
        let mut mapping = BTreeMap::new();
        if let Some(expected) = expected {
            if !infer_type_arguments(result, expected, &mut mapping, self.records, self.types) {
                self.error(&call.span, "Map result has the wrong type");
            }
        }
        let templates = [map, key, value];
        let values: Vec<_> = arguments.iter().collect();
        let completes =
            self.generic_values(&values, &templates[..count], &parameters, &mut mapping);
        if !self.resolved(&parameters, &mapping) {
            self.error(&call.span, "Cannot infer map key/value types");
            return Some(ValueType::Error);
        }
        let concrete_key = self.substitute(key, &mapping, &call.span);
        let concrete_value = self.substitute(value, &mapping, &call.span);
        if !matches!(
            concrete_key,
            ValueType::Int | ValueType::Bool | ValueType::String | ValueType::Error
        ) {
            self.error(&call.span, "Map keys must be int, bool, or string");
        }
        self.aggregate_allowed(concrete_value, &call.span);
        completes.then(|| self.substitute(result, &mapping, &call.span))
    }

    fn check_host_builtin(
        &mut self,
        call: &Expr,
        name: &str,
        arguments: &[Expr],
    ) -> Option<ValueType> {
        let count = if matches!(name, "storageSet" | "storageSetAsync") {
            3
        } else {
            2
        };
        let kind = if name == "consoleLog" {
            CapabilityKind::Console
        } else if name == "httpGet" {
            CapabilityKind::Http
        } else {
            CapabilityKind::Storage
        };
        if arguments.len() != count {
            self.diagnostics.push(Diagnostic::error(
                "UBI0040",
                "Host operation requires its capability and typed arguments",
                call.span.clone(),
            ));
            self.arity(call, arguments, count);
        }
        let mut completes = true;
        for (index, argument) in arguments.iter().enumerate() {
            let expected = if index == 0 {
                ValueType::Capability(kind)
            } else {
                ValueType::String
            };
            let ty = self.check_expr_expected(argument, Some(expected));
            completes &= ty.is_some();
            if index == 0 && ty.is_some_and(|ty| ty != expected && ty != ValueType::Error) {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0040",
                    "Host operation requires the matching capability",
                    argument.span.clone(),
                ));
            } else if let Some(ty) = ty {
                self.require_same_type(
                    expected,
                    ty,
                    &argument.span,
                    "Host argument has the wrong type",
                );
            }
        }
        let ok = if matches!(name, "storageGet" | "storageGetAsync") {
            intern_type(self.types, TypeInfo::Option(ValueType::String))
        } else if name == "httpGet" {
            ValueType::String
        } else {
            ValueType::Unit
        };
        let error = ValueType::Enum(
            enum_type(self.types, &(String::new(), "HostError".to_owned())).unwrap(),
        );
        let id = enum_type(self.types, &(String::new(), "Result".to_owned())).unwrap();
        let TypeInfo::Enum {
            arguments: parameters,
            ..
        } = &self.types[id]
        else {
            unreachable!()
        };
        let mapping = parameter_mapping(parameters, &[ok, error]);
        completes.then(|| self.substitute(ValueType::Enum(id), &mapping, &call.span))
    }

    fn mismatch(&mut self, expression: &Expr, left: ValueType, right: ValueType, message: &str) {
        self.diagnostics.push(Diagnostic::error(
            "UBI0020",
            format!("{message}: {} and {}", left.name(), right.name()),
            expression.span.clone(),
        ));
    }

    fn record_return(&mut self, ty: ValueType, span: Span) {
        if ty == ValueType::Error {
            return;
        }
        if let Some(expected) = self.expected_return {
            self.require_same_type(expected, ty, &span, "Function return has the wrong type");
            return;
        }
        match self.inferred_return {
            None => self.inferred_return = Some(ty),
            Some(inferred) if inferred != ty => self.diagnostics.push(Diagnostic::error(
                "UBI0020",
                format!(
                    "Function returns both {} and {}",
                    inferred.name(),
                    ty.name()
                ),
                span,
            )),
            _ => {}
        }
    }

    fn check_integer(
        &mut self,
        raw: &str,
        literal_span: &Span,
        negative: bool,
        error_span: &Span,
    ) -> ValueType {
        let Ok(value) = raw.parse::<u64>() else {
            self.diagnostics.push(Diagnostic::error(
                "UBI0004",
                "Integer literal is outside the signed 32-bit range",
                error_span.clone(),
            ));
            return ValueType::Error;
        };
        let max = if negative {
            2_147_483_648
        } else {
            2_147_483_647
        };
        if value > max {
            self.diagnostics.push(Diagnostic::error(
                "UBI0004",
                "Integer literal is outside the signed 32-bit range",
                if negative {
                    error_span.clone()
                } else {
                    literal_span.clone()
                },
            ));
            ValueType::Error
        } else {
            ValueType::Int
        }
    }

    fn check_float(&mut self, raw: &str, error_span: &Span) -> ValueType {
        match raw.parse::<f64>() {
            Ok(value) if value.is_finite() => ValueType::Float,
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0004",
                    "Float literal is outside the representable range",
                    error_span.clone(),
                ));
                ValueType::Error
            }
        }
    }
}

fn is_option_member(expression: &Expr, name: &str) -> bool {
    matches!(&expression.kind, ExprKind::Name(value) if value.name == name)
}

pub(crate) fn enum_type(types: &[TypeInfo], target: &FunctionKey) -> Option<usize> {
    types
        .iter()
        .position(|info| matches!(info, TypeInfo::Enum { key, .. } if key == target))
}

fn collect_pattern_names(pattern: &Pattern, names: &mut HashSet<String>) {
    match &pattern.kind {
        PatternKind::Binding(name) => {
            names.insert(name.name.clone());
        }
        PatternKind::Some(inner) => collect_pattern_names(inner, names),
        PatternKind::Tuple(payloads) | PatternKind::Variant { payloads, .. } => {
            for payload in payloads {
                collect_pattern_names(payload, names);
            }
        }
        _ => {}
    }
}

fn patterns_exhaustive(
    ty: ValueType,
    patterns: &[&Pattern],
    types: &[TypeInfo],
) -> Result<bool, ()> {
    // Specialize whole rows: checking payload columns separately loses correlations.
    // An explicit stack also bounds compiler stack usage for wide enum payloads.
    let rows = patterns
        .iter()
        .map(|pattern| vec![Some(*pattern)])
        .collect::<Vec<_>>();
    let mut pending = vec![(vec![ty], rows)];
    let mut budget = 100_000usize;
    while let Some((columns, rows)) = pending.pop() {
        let work = 1 + rows.iter().map(|row| 1 + row.len()).sum::<usize>();
        budget = budget.checked_sub(work).ok_or(())?;
        if rows.is_empty() {
            return Ok(false);
        }
        if columns.is_empty()
            || rows
                .iter()
                .any(|row| row.iter().all(|pattern| catch_all(*pattern)))
        {
            continue;
        }
        let constructors = match columns[0] {
            ValueType::Unit => vec![("tuple", vec![])],
            ValueType::Tuple(id) => {
                let TypeInfo::Tuple(elements) = &types[id] else {
                    unreachable!()
                };
                vec![("tuple", elements.clone())]
            }
            ValueType::Bool => vec![("true", vec![]), ("false", vec![])],
            ValueType::Option(id) => {
                let TypeInfo::Option(inner) = types[id] else {
                    unreachable!()
                };
                vec![("Some", vec![inner]), ("None", vec![])]
            }
            ValueType::Enum(id) => {
                let TypeInfo::Enum { variants, .. } = &types[id] else {
                    unreachable!()
                };
                variants
                    .iter()
                    .map(|(tag, payloads)| (tag.as_str(), payloads.clone()))
                    .collect()
            }
            _ => vec![],
        };
        if constructors.is_empty() {
            let defaults = rows
                .into_iter()
                .filter(|row| catch_all(row[0]))
                .map(|row| row.into_iter().skip(1).collect())
                .collect();
            pending.push((columns.into_iter().skip(1).collect(), defaults));
        } else {
            for (tag, payload_types) in constructors {
                let mut specialized = Vec::new();
                for row in &rows {
                    budget = budget
                        .checked_sub(1 + row.len() + payload_types.len())
                        .ok_or(())?;
                    let mut payloads = if catch_all(row[0]) {
                        vec![None; payload_types.len()]
                    } else {
                        match &row[0].unwrap().kind {
                            PatternKind::Tuple(elements) if tag == "tuple" => {
                                elements.iter().map(Some).collect()
                            }
                            PatternKind::Bool(value)
                                if tag == if *value { "true" } else { "false" } =>
                            {
                                vec![]
                            }
                            PatternKind::None if tag == "None" => vec![],
                            PatternKind::Some(inner) if tag == "Some" => vec![Some(inner.as_ref())],
                            PatternKind::Variant { name, payloads, .. }
                                if name.name == tag && payloads.len() == payload_types.len() =>
                            {
                                payloads.iter().map(Some).collect()
                            }
                            _ => continue,
                        }
                    };
                    payloads.extend_from_slice(&row[1..]);
                    specialized.push(payloads);
                }
                let mut next_types = payload_types;
                next_types.extend_from_slice(&columns[1..]);
                pending.push((next_types, specialized));
            }
        }
    }
    Ok(true)
}

fn catch_all(pattern: Option<&Pattern>) -> bool {
    pattern.is_none_or(|pattern| {
        matches!(
            pattern.kind,
            PatternKind::Wildcard | PatternKind::Binding(_)
        )
    })
}
pub(crate) fn validate_instantiated_aggregates(
    records: &[RecordInfo],
    types: &[TypeInfo],
    record_keys: &BTreeMap<FunctionKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    span: &Span,
) {
    let invalid_record_arguments = records.iter().any(|record| {
        let Some(template_id) = record_keys.get(&record.key) else {
            return false;
        };
        records[*template_id]
            .arguments
            .iter()
            .zip(&record.arguments)
            .any(|(parameter, actual)| {
                matches!(
                    types.get(match parameter {
                        ValueType::Parameter(id) => *id,
                        _ => return false,
                    }),
                    Some(TypeInfo::Parameter {
                        equatable: true,
                        ..
                    })
                ) && !is_equatable(*actual, records, types)
            })
    });
    let invalid_enum_arguments = types.iter().enumerate().any(|(id, info)| {
        let TypeInfo::Enum { key, arguments, .. } = info else {
            return false;
        };
        let Some(template_id) = enum_type(types, key) else {
            return false;
        };
        if id == template_id {
            return false;
        }
        let TypeInfo::Enum {
            arguments: parameters,
            ..
        } = &types[template_id]
        else {
            unreachable!()
        };
        parameters.iter().zip(arguments).any(|(parameter, actual)| {
            matches!(
                types.get(match parameter {
                    ValueType::Parameter(id) => *id,
                    _ => return false,
                }),
                Some(TypeInfo::Parameter {
                    equatable: true,
                    ..
                })
            ) && !is_equatable(*actual, records, types)
        })
    });
    if invalid_record_arguments || invalid_enum_arguments {
        diagnostics.push(Diagnostic::error(
            "UBI0020",
            "Type argument does not satisfy Equatable constraint",
            span.clone(),
        ));
    }

    let mut pending = Vec::new();
    for record in records {
        pending.extend(record.fields.values().copied());
    }
    for info in types {
        match info {
            TypeInfo::Tuple(elements) => pending.extend(elements.iter().copied()),
            TypeInfo::List(inner) | TypeInfo::Option(inner) => pending.push(*inner),
            TypeInfo::Enum { variants, .. } => {
                pending.extend(variants.values().flatten().copied());
            }
            TypeInfo::Map { key, value } => {
                pending.extend([*key, *value]);
            }
            _ => {}
        }
    }
    let mut visited = BTreeSet::new();
    let mut forbidden_function = false;
    let mut forbidden_capability = false;
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match ty {
            ValueType::Range => forbidden_function = true,
            ValueType::Tuple(id) => {
                if let TypeInfo::Tuple(elements) = &types[id] {
                    pending.extend(elements.iter().copied());
                }
            }
            ValueType::Function(id) => {
                if let TypeInfo::Function {
                    parameters,
                    return_type,
                } = &types[id]
                {
                    pending.extend(parameters.iter().copied());
                    pending.push(*return_type);
                }
            }
            ValueType::Capability(_) => forbidden_capability = true,
            ValueType::Record(id) => {
                pending.extend(records[id].fields.values().copied());
            }
            ValueType::Enum(id) => {
                if let TypeInfo::Enum { variants, .. } = &types[id] {
                    pending.extend(variants.values().flatten().copied());
                }
            }
            ValueType::List(id) | ValueType::Option(id) => {
                if let TypeInfo::List(inner) | TypeInfo::Option(inner) = types[id] {
                    pending.push(inner);
                }
            }
            ValueType::Map(id) => {
                if let TypeInfo::Map { key, value } = types[id] {
                    pending.extend([key, value]);
                }
            }
            _ => {}
        }
    }
    if forbidden_function {
        diagnostics.push(Diagnostic::error(
            "UBI0020",
            "Range values cannot be stored in instantiated aggregates",
            span.clone(),
        ));
    }
    if forbidden_capability {
        diagnostics.push(Diagnostic::error(
            "UBI0040",
            "Capabilities cannot be stored in instantiated aggregates",
            span.clone(),
        ));
    }
}
