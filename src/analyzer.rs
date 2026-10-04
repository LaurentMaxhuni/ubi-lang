use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::diagnostics::{sort_diagnostics, Diagnostic, RelatedDiagnostic};
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Function, Identifier, MatchArm, Module, Parameter,
    ParseError, Pattern, PatternKind, StatementKind, Type,
};
use crate::source::{resolve_import_id, SourceSet};
use crate::span::Span;

pub(crate) type FunctionKey = (String, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueType {
    Int,
    Float,
    Bool,
    String,
    Unit,
    Record(usize),
    List(usize),
    Option(usize),
    Enum(usize),
    Function(usize),
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
            Self::Record(_) => "record",
            Self::List(_) => "List",
            Self::Option(_) => "Option",
            Self::Enum(_) => "enum",
            Self::Function(_) => "function",
            Self::Range => "range",
            Self::Never => "never",
            Self::Error => "<error>",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeInfo {
    List(ValueType),
    Option(ValueType),
    Enum {
        key: FunctionKey,
        exported: bool,
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
        TypeInfo::List(_) => ValueType::List(id),
        TypeInfo::Option(_) => ValueType::Option(id),
        TypeInfo::Enum { .. } => ValueType::Enum(id),
        TypeInfo::Function { .. } => ValueType::Function(id),
    }
}

pub(crate) fn builtin_name(name: &str) -> bool {
    matches!(
        name,
        "range"
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
    pub(crate) records: Vec<RecordInfo>,
    pub(crate) record_keys: BTreeMap<FunctionKey, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordInfo {
    pub(crate) key: FunctionKey,
    pub(crate) exported: bool,
    pub(crate) fields: BTreeMap<String, ValueType>,
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
            records: Vec::new(),
            record_keys: BTreeMap::new(),
        };
    }

    let mut types = Vec::new();
    let (records, record_keys) =
        collect_records(&modules, &mut module_symbols, &mut types, &mut diagnostics);
    let (functions, mut signatures) = collect_signatures(
        &modules,
        &module_symbols,
        &records,
        &record_keys,
        &mut types,
        &mut diagnostics,
    );
    let call_graph = build_call_graph(&functions, &module_symbols);
    let components = strongly_connected_components(&call_graph);
    let mut expression_types = BTreeMap::new();

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

            let (inferred_return, function_types, mut function_diagnostics) = check_function(
                &key,
                function,
                &signature,
                &signatures,
                &module_symbols,
                &records,
                &record_keys,
                &mut types,
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

    sort_diagnostics(&mut diagnostics);
    Analysis {
        types,
        diagnostics,
        modules,
        module_symbols,
        signatures,
        expression_types,
        records,
        record_keys,
    }
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

fn collect_records(
    modules: &BTreeMap<String, Module>,
    module_symbols: &mut BTreeMap<String, BTreeMap<String, FunctionKey>>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<RecordInfo>, BTreeMap<FunctionKey, usize>) {
    let mut records = Vec::new();
    let mut keys = BTreeMap::new();
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
                || matches!(name.name.as_str(), "List" | "Option" | "Result")
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
                keys.insert(key.clone(), records.len());
                records.push(RecordInfo {
                    key: key.clone(),
                    exported: record.exported,
                    fields: BTreeMap::new(),
                });
            }
            if let Declaration::Enum(value) = declaration {
                types.push(TypeInfo::Enum {
                    key,
                    exported: value.exported,
                    variants: BTreeMap::new(),
                });
            }
        }
    }
    for record in &mut records {
        let declaration = modules[&record.key.0]
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                Declaration::Record(value) if value.name.name == record.key.1 => Some(value),
                _ => None,
            })
            .unwrap();
        for field in &declaration.fields {
            let ty = type_from_syntax(
                &field.ty,
                &record.key.0,
                module_symbols,
                &keys,
                types,
                diagnostics,
            );
            if record.fields.insert(field.name.name.clone(), ty).is_some() {
                diagnostics.push(Diagnostic::error(
                    "UBI0030",
                    "Duplicate record field",
                    field.name.span.clone(),
                ));
            }
        }
    }
    let enums: Vec<_> = types
        .iter()
        .enumerate()
        .filter_map(|(id, info)| {
            if let TypeInfo::Enum { key, .. } = info {
                Some((id, key.clone()))
            } else {
                None
            }
        })
        .collect();
    for (id, key) in enums {
        let declaration = modules[&key.0]
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                Declaration::Enum(value) if value.name.name == key.1 => Some(value),
                _ => None,
            })
            .unwrap();
        let mut variants = BTreeMap::new();
        for variant in &declaration.variants {
            let payloads: Vec<_> = variant
                .payloads
                .iter()
                .map(|ty| type_from_syntax(ty, &key.0, module_symbols, &keys, types, diagnostics))
                .collect();
            if declaration.exported {
                for (ty, syntax) in payloads.iter().zip(&variant.payloads) {
                    check_public_type(*ty, &syntax.span, &records, types, diagnostics);
                }
            }
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
        } = &mut types[id]
        else {
            unreachable!()
        };
        *target = variants;
    }
    for record in &records {
        if !record.exported {
            continue;
        }
        let declaration = modules[&record.key.0]
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                Declaration::Record(value) if value.name.name == record.key.1 => Some(value),
                _ => None,
            })
            .unwrap();
        for field in &declaration.fields {
            let ty = record.fields[&field.name.name];
            check_public_type(ty, &field.ty.span, &records, types, diagnostics);
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
    match ty {
        ValueType::Enum(id) => {
            if matches!(
                types[id],
                TypeInfo::Enum {
                    exported: false,
                    ..
                }
            ) {
                diagnostics.push(Diagnostic::error(
                    "UBI0013",
                    "Private enum type exposed by exported signature",
                    span.clone(),
                ));
            }
        }
        ValueType::List(id) | ValueType::Option(id) => {
            if let TypeInfo::List(inner) | TypeInfo::Option(inner) = types[id] {
                check_public_type(inner, span, records, types, diagnostics);
            }
        }
        ValueType::Function(_) | ValueType::Range => diagnostics.push(Diagnostic::error(
            "UBI0020",
            "Function and range values cannot be exported",
            span.clone(),
        )),
        _ => {}
    }
    if let ValueType::Record(id) = ty {
        if !records[id].exported {
            diagnostics.push(Diagnostic::error(
                "UBI0013",
                "Private record type exposed by exported signature",
                span.clone(),
            ));
        }
    }
}

fn collect_signatures<'a>(
    modules: &'a BTreeMap<String, Module>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &[RecordInfo],
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
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    let ty = type_from_syntax(
                        &parameter.ty,
                        source_id,
                        module_symbols,
                        record_keys,
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
                    types,
                    diagnostics,
                );
                if function.exported {
                    check_public_type(value, &ty.span, records, types, diagnostics);
                }
                value
            });
            if function.exported && return_type.is_none() {
                diagnostics.push(Diagnostic::error(
                    "UBI0021",
                    "Exported functions require an explicit return annotation",
                    function.name.span.clone(),
                ));
            }
            functions.insert(key.clone(), function);
            signatures.insert(
                key,
                Signature {
                    parameters,
                    return_type,
                    name_span: function.name.span.clone(),
                },
            );
        }
    }
    (functions, signatures)
}

fn type_from_syntax(
    ty: &Type,
    module_id: &str,
    symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &BTreeMap<FunctionKey, usize>,
    types: &mut Vec<TypeInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) -> ValueType {
    if matches!(ty.name.as_str(), "List" | "Option") && ty.arguments.len() == 1 {
        let inner = type_from_syntax(
            &ty.arguments[0],
            module_id,
            symbols,
            records,
            types,
            diagnostics,
        );
        return intern_type(
            types,
            if ty.name == "List" {
                TypeInfo::List(inner)
            } else {
                TypeInfo::Option(inner)
            },
        );
    }
    if !ty.arguments.is_empty() || matches!(ty.name.as_str(), "List" | "Option") {
        diagnostics.push(Diagnostic::error(
            "UBI0020",
            "Invalid type arguments",
            ty.span.clone(),
        ));
        return ValueType::Error;
    }
    match ty.name.as_str() {
        "int" => ValueType::Int,
        "float" => ValueType::Float,
        "bool" => ValueType::Bool,
        "string" => ValueType::String,
        "unit" => ValueType::Unit,
        _ => {
            if let Some(id) = symbols
                .get(module_id)
                .and_then(|module| module.get(&ty.name))
                .and_then(|key| records.get(key))
            {
                return ValueType::Record(*id);
            }
            if let Some(id) = symbols
                .get(module_id)
                .and_then(|module| module.get(&ty.name))
                .and_then(|key| enum_type(types, key))
            {
                return ValueType::Enum(id);
            }
            diagnostics.push(Diagnostic::error(
                "UBI0020",
                format!("Unknown type: {}", ty.name),
                ty.span.clone(),
            ));
            ValueType::Error
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
        ExprKind::List(values) => {
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
        ExprKind::Unary { operand, .. } | ExprKind::Propagate(operand) => {
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

#[allow(clippy::too_many_arguments)]
fn check_function(
    key: &FunctionKey,
    function: &Function,
    signature: &Signature,
    signatures: &BTreeMap<FunctionKey, Signature>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &[RecordInfo],
    record_keys: &BTreeMap<FunctionKey, usize>,
    types: &mut Vec<TypeInfo>,
) -> (
    Option<ValueType>,
    BTreeMap<Span, ValueType>,
    Vec<Diagnostic>,
) {
    let mut checker = FunctionChecker {
        module_id: &key.0,
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
        checker.diagnostics,
    )
}

#[derive(Clone, Copy)]
struct Binding {
    ty: ValueType,
    mutable: bool,
}

struct FunctionChecker<'a> {
    types: &'a mut Vec<TypeInfo>,
    expected_expr: Option<ValueType>,
    loop_depth: usize,
    module_id: &'a str,
    signatures: &'a BTreeMap<FunctionKey, Signature>,
    module_symbols: &'a BTreeMap<String, BTreeMap<String, FunctionKey>>,
    records: &'a [RecordInfo],
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
            ExprKind::Name(name) => Some(self.check_name(name)),
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
                    let TypeInfo::Enum { variants, .. } = &self.types[id] else {
                        unreachable!()
                    };
                    match variants.get(&name.name) {
                        Some(payloads) if payloads.is_empty() => return Some(ValueType::Enum(id)),
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
                self.check_record(expression, name, base.as_deref(), fields)
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
            ExprKind::Propagate(_) => {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0003",
                    "Result propagation is not supported in Milestone 1",
                    expression.span.clone(),
                ));
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

    fn check_name(&mut self, name: &Identifier) -> ValueType {
        if let Some(ty) = self.lookup_local(&name.name) {
            return ty;
        }
        if let Some(signature) = self
            .module_symbols
            .get(self.module_id)
            .and_then(|symbols| symbols.get(&name.name))
            .and_then(|key| self.signatures.get(key))
        {
            return intern_type(
                self.types,
                TypeInfo::Function {
                    parameters: signature.parameters.clone(),
                    return_type: signature.return_type.unwrap_or(ValueType::Error),
                },
            );
        }
        self.diagnostics.push(Diagnostic::error(
            "UBI0010",
            format!("Unknown name: {}", name.name),
            name.span.clone(),
        ));
        ValueType::Error
    }

    fn check_record(
        &mut self,
        expression: &Expr,
        name: &Identifier,
        base: Option<&Expr>,
        fields: &[(Identifier, Expr)],
    ) -> Option<ValueType> {
        let id = if self.lookup_local(&name.name).is_none() {
            self.module_symbols
                .get(self.module_id)
                .and_then(|symbols| symbols.get(&name.name))
                .and_then(|key| self.record_keys.get(key))
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
        let record = &self.records[id];
        let mut completes = true;
        if let Some(base) = base {
            if let Some(ty) = self.check_expr(base) {
                self.require_same_type(
                    ValueType::Record(id),
                    ty,
                    &base.span,
                    "Record update base has the wrong type",
                );
            } else {
                completes = false;
            }
        }
        let mut seen = BTreeSet::new();
        for (field, value) in fields {
            if !seen.insert(field.name.clone()) || !record.fields.contains_key(&field.name) {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0030",
                    "Unknown or duplicate record field",
                    field.span.clone(),
                ));
            }
            let expected = record.fields.get(&field.name).copied();
            if let Some(ty) = self.check_expr_expected(value, expected) {
                if let Some(expected) = record.fields.get(&field.name) {
                    self.require_same_type(
                        *expected,
                        ty,
                        &value.span,
                        "Record field has the wrong type",
                    );
                }
            } else {
                completes = false;
            }
        }
        if base.is_none() && record.fields.keys().any(|field| !seen.contains(field)) {
            self.diagnostics.push(Diagnostic::error(
                "UBI0030",
                "Missing record field",
                expression.span.clone(),
            ));
        }
        completes.then_some(ValueType::Record(id))
    }

    fn check_call(
        &mut self,
        call: &Expr,
        callee: &Expr,
        arguments: &[Expr],
        expected: Option<ValueType>,
    ) -> Option<ValueType> {
        if let ExprKind::Member { object, name } = &callee.kind {
            if let Some(id) = self.enum_namespace(object) {
                let TypeInfo::Enum { variants, .. } = &self.types[id] else {
                    unreachable!()
                };
                let payloads = variants.get(&name.name).cloned();
                let Some(payloads) = payloads else {
                    self.error(&name.span, "Unknown enum variant");
                    for argument in arguments {
                        self.check_expr(argument);
                    }
                    return Some(ValueType::Error);
                };
                if payloads.is_empty() {
                    self.error(&callee.span, "Payload-free enum variant cannot be called");
                }
                self.arity(call, arguments, payloads.len());
                let mut completes = true;
                for (index, argument) in arguments.iter().enumerate() {
                    let expected = payloads.get(index).copied();
                    let actual = self.check_expr_expected(argument, expected);
                    if let (Some(expected), Some(actual)) = (expected, actual) {
                        self.require_same_type(
                            expected,
                            actual,
                            &argument.span,
                            "Enum payload has the wrong type",
                        );
                    }
                    completes &= actual.is_some();
                }
                return completes.then_some(ValueType::Enum(id));
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
        if matches!(ty, ValueType::Function(_) | ValueType::Range) {
            self.error(
                span,
                "Function and range values cannot be stored in aggregates",
            );
            false
        } else {
            true
        }
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

    fn check_pattern(&mut self, pattern: &Pattern, expected: ValueType) {
        match &pattern.kind {
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
                let id = self.enum_id(&qualifier.name);
                if id.map(ValueType::Enum) != Some(expected) {
                    self.error(
                        &qualifier.span,
                        "Enum pattern requires the same nominal enum subject",
                    );
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
                    expected.and_then(|ty| self.list_inner(ty))
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
                if a != String && item.is_none() && a != Error {
                    self.error(&arguments[0].span, "Length requires string or List");
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
                if !self.equatable(left_type) || !self.equatable(right_type) {
                    self.error(&right.span, "Function and range values cannot be compared");
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

    fn equatable(&self, ty: ValueType) -> bool {
        let mut pending = vec![ty];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            match ty {
                ValueType::Function(_) | ValueType::Range => return false,
                ValueType::List(id) | ValueType::Option(id) => match self.types[id] {
                    TypeInfo::List(inner) | TypeInfo::Option(inner) => pending.push(inner),
                    _ => unreachable!(),
                },
                ValueType::Record(id) if visited.insert((false, id)) => {
                    pending.extend(self.records[id].fields.values().copied());
                }
                ValueType::Enum(id) if visited.insert((true, id)) => {
                    let TypeInfo::Enum { variants, .. } = &self.types[id] else {
                        unreachable!()
                    };
                    pending.extend(variants.values().flatten().copied());
                }
                _ => {}
            }
        }
        true
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
        PatternKind::Variant { payloads, .. } => {
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
