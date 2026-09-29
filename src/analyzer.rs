use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::diagnostics::{sort_diagnostics, Diagnostic, RelatedDiagnostic};
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Function, Identifier, Module, ParseError, StatementKind,
    Type,
};
use crate::source::{resolve_import_id, SourceSet};
use crate::span::Span;

type FunctionKey = (String, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueType {
    Int,
    Float,
    Bool,
    String,
    Unit,
    Error,
}

impl ValueType {
    fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::String => "string",
            Self::Unit => "unit",
            Self::Error => "<error>",
        }
    }
}

#[derive(Debug, Clone)]
struct Signature {
    parameters: Vec<ValueType>,
    return_type: Option<ValueType>,
    name_span: Span,
}

#[derive(Debug, Clone)]
struct ModuleEdges {
    from: String,
    to: String,
    span: Span,
}

pub(crate) fn check(sources: &SourceSet) -> Vec<Diagnostic> {
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
        return diagnostics;
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
        return diagnostics;
    }

    let (functions, mut signatures) =
        collect_signatures(&modules, &mut module_symbols, &mut diagnostics);
    let call_graph = build_call_graph(&functions, &module_symbols);
    let components = strongly_connected_components(&call_graph);

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

            let (inferred_return, mut function_diagnostics) =
                check_function(&key, function, &signature, &signatures, &module_symbols);
            diagnostics.append(&mut function_diagnostics);
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
    diagnostics
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
                match find_function(&modules[&target_id], &imported.name) {
                    Some(function) if function.exported => {
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

fn find_function<'a>(module: &'a Module, name: &str) -> Option<&'a Function> {
    module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            Declaration::Function(function) if function.name.name == name => Some(function),
            _ => None,
        })
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
        for from in 0..ids.len() {
            if !reachable[from][pivot] {
                continue;
            }
            for to in 0..ids.len() {
                reachable[from][to] |= reachable[pivot][to];
            }
        }
    }

    let mut assigned = BTreeSet::new();
    for index in 0..ids.len() {
        if assigned.contains(&index) {
            continue;
        }
        let component: BTreeSet<usize> = (0..ids.len())
            .filter(|other| reachable[index][*other] && reachable[*other][index])
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

fn collect_signatures<'a>(
    modules: &'a BTreeMap<String, Module>,
    module_symbols: &mut BTreeMap<String, BTreeMap<String, FunctionKey>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    BTreeMap<FunctionKey, &'a Function>,
    BTreeMap<FunctionKey, Signature>,
) {
    let mut functions = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    for (source_id, module) in modules {
        let symbols = module_symbols.entry(source_id.clone()).or_default();
        for declaration in &module.declarations {
            let Declaration::Function(function) = declaration;
            if symbols.contains_key(&function.name.name) {
                diagnostics.push(Diagnostic::error(
                    "UBI0011",
                    format!("Duplicate declaration: {}", function.name.name),
                    function.name.span.clone(),
                ));
                continue;
            }
            let key = (source_id.clone(), function.name.name.clone());
            symbols.insert(function.name.name.clone(), key.clone());
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| type_from_syntax(&parameter.ty, diagnostics))
                .collect();
            let return_type = function
                .return_type
                .as_ref()
                .map(|ty| type_from_syntax(ty, diagnostics));
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

fn type_from_syntax(ty: &Type, diagnostics: &mut Vec<Diagnostic>) -> ValueType {
    match ty.name.as_str() {
        "int" => ValueType::Int,
        "float" => ValueType::Float,
        "bool" => ValueType::Bool,
        "string" => ValueType::String,
        "unit" => ValueType::Unit,
        _ => {
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
        | ExprKind::Unit
        | ExprKind::Name(_) => {}
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

fn check_function(
    key: &FunctionKey,
    function: &Function,
    signature: &Signature,
    signatures: &BTreeMap<FunctionKey, Signature>,
    module_symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
) -> (Option<ValueType>, Vec<Diagnostic>) {
    let mut checker = FunctionChecker {
        module_id: &key.0,
        signatures,
        module_symbols,
        expected_return: signature.return_type,
        inferred_return: None,
        scopes: vec![BTreeMap::new()],
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
            scope.insert(parameter.name.name.clone(), ty);
        }
    }

    if let Some(body_type) = checker.check_block(&function.body) {
        let completion_span = function
            .body
            .tail
            .as_ref()
            .map_or(&function.body.span, |expression| &expression.span)
            .clone();
        checker.record_return(body_type, completion_span);
    }
    (checker.inferred_return, checker.diagnostics)
}

struct FunctionChecker<'a> {
    module_id: &'a str,
    signatures: &'a BTreeMap<FunctionKey, Signature>,
    module_symbols: &'a BTreeMap<String, BTreeMap<String, FunctionKey>>,
    expected_return: Option<ValueType>,
    inferred_return: Option<ValueType>,
    scopes: Vec<BTreeMap<String, ValueType>>,
    diagnostics: Vec<Diagnostic>,
}

impl FunctionChecker<'_> {
    fn check_block(&mut self, block: &Block) -> Option<ValueType> {
        self.scopes.push(BTreeMap::new());
        let mut completes = true;
        for statement in &block.statements {
            if !completes {
                break;
            }
            match &statement.kind {
                StatementKind::Let {
                    name,
                    annotation,
                    value,
                } => {
                    let value_type = self.check_expr(value);
                    let Some(value_type) = value_type else {
                        completes = false;
                        continue;
                    };
                    let bound_type = annotation
                        .as_ref()
                        .map(|ty| type_from_syntax(ty, &mut self.diagnostics));
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
                        scope.insert(name.name.clone(), bound_type.unwrap_or(value_type));
                    }
                }
                StatementKind::Assign { target, value } => {
                    let target_type = self.check_assignment_target(target);
                    let value_type = self.check_expr(value);
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
                        if let Some(return_type) = self.check_expr(value) {
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
            }
        }

        let result = if completes {
            if let Some(tail) = &block.tail {
                self.check_expr(tail)
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
        match &expression.kind {
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
                            return Some(self.check_integer(
                                value,
                                literal_span,
                                true,
                                &expression.span,
                            ));
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
                        return Some(self.check_float(value, span));
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
                let right_type = self.check_expr(right);
                let (Some(left_type), Some(right_type)) = (left_type, right_type) else {
                    return None;
                };
                Some(self.binary_type(*operator, left_type, right_type, left, right))
            }
            ExprKind::Call { callee, arguments } => self.check_call(expression, callee, arguments),
            ExprKind::Member { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0003",
                    "Member access is not supported in Milestone 1",
                    expression.span.clone(),
                ));
                Some(ValueType::Error)
            }
            ExprKind::Index { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0003",
                    "Indexing is not supported in Milestone 1",
                    expression.span.clone(),
                ));
                Some(ValueType::Error)
            }
            ExprKind::Propagate(_) => {
                self.diagnostics.push(Diagnostic::error(
                    "UBI0003",
                    "Result propagation is not supported in Milestone 1",
                    expression.span.clone(),
                ));
                Some(ValueType::Error)
            }
            ExprKind::Block(block) => self.check_block(block),
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
                let then_type = self.check_block(then_branch);
                let else_type = self.check_expr(else_branch);
                if condition_type.is_none() {
                    return None;
                }
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
        if self
            .module_symbols
            .get(self.module_id)
            .is_some_and(|symbols| symbols.contains_key(&name.name))
        {
            self.diagnostics.push(Diagnostic::error(
                "UBI0003",
                "Function values are not supported in Milestone 1",
                name.span.clone(),
            ));
            return ValueType::Error;
        }
        self.diagnostics.push(Diagnostic::error(
            "UBI0010",
            format!("Unknown name: {}", name.name),
            name.span.clone(),
        ));
        ValueType::Error
    }

    fn check_call(&mut self, call: &Expr, callee: &Expr, arguments: &[Expr]) -> Option<ValueType> {
        let ExprKind::Name(name) = &callee.kind else {
            for argument in arguments {
                self.check_expr(argument);
            }
            self.diagnostics.push(Diagnostic::error(
                "UBI0003",
                "Only named function calls are supported in Milestone 1",
                callee.span.clone(),
            ));
            return Some(ValueType::Error);
        };

        if self.lookup_local(&name.name).is_some() {
            for argument in arguments {
                self.check_expr(argument);
            }
            self.diagnostics.push(Diagnostic::error(
                "UBI0003",
                "Function values are not supported in Milestone 1",
                name.span.clone(),
            ));
            return Some(ValueType::Error);
        }

        let key = self
            .module_symbols
            .get(self.module_id)
            .and_then(|symbols| symbols.get(&name.name))
            .cloned();
        let Some(key) = key else {
            for argument in arguments {
                self.check_expr(argument);
            }
            self.diagnostics.push(Diagnostic::error(
                "UBI0010",
                format!("Unknown name: {}", name.name),
                name.span.clone(),
            ));
            return Some(ValueType::Error);
        };
        let signature = self.signatures.get(&key).cloned();
        let Some(signature) = signature else {
            for argument in arguments {
                self.check_expr(argument);
            }
            return Some(ValueType::Error);
        };
        if arguments.len() != signature.parameters.len() {
            self.diagnostics.push(Diagnostic::error(
                "UBI0023",
                format!(
                    "Expected {} arguments, found {}",
                    signature.parameters.len(),
                    arguments.len()
                ),
                call.span.clone(),
            ));
        }
        let mut completes = true;
        for (index, argument) in arguments.iter().enumerate() {
            let argument_type = self.check_expr(argument);
            if let (Some(actual), Some(expected)) =
                (argument_type, signature.parameters.get(index).copied())
            {
                self.require_same_type(
                    expected,
                    actual,
                    &argument.span,
                    "Function argument has the wrong type",
                );
            }
            completes &= argument_type.is_some();
        }
        if !completes {
            return None;
        }
        Some(signature.return_type.unwrap_or(ValueType::Error))
    }

    fn check_assignment_target(&mut self, target: &Expr) -> Option<ValueType> {
        let ExprKind::Name(name) = &target.kind else {
            return Some(ValueType::Error);
        };
        if let Some(ty) = self.lookup_local(&name.name) {
            self.diagnostics.push(Diagnostic::error(
                "UBI0022",
                format!("Cannot assign to immutable binding: {}", name.name),
                name.span.clone(),
            ));
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
            .find_map(|scope| scope.get(name).copied())
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
                if left_type != right_type {
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
