use std::collections::{BTreeMap, BTreeSet};

use crate::analyzer::{infer_type_arguments, Analysis, FunctionKey, TypeInfo, ValueType};
use crate::diagnostics::Diagnostic;
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Pattern, PatternKind, StatementKind, Type,
};
use crate::span::Span;

pub(crate) fn host_effect(name: &str) -> Option<&'static str> {
    match name {
        "consoleLog" => Some("console"),
        "storageGet" | "storageSet" | "storageRemove" | "storageGetAsync" | "storageSetAsync"
        | "storageRemoveAsync" => Some("storage"),
        "httpGet" => Some("http"),
        _ => None,
    }
}

fn async_host(name: &str) -> bool {
    matches!(
        name,
        "storageGetAsync" | "storageSetAsync" | "storageRemoveAsync" | "httpGet"
    )
}

pub(crate) fn check(analysis: &Analysis) -> Vec<Diagnostic> {
    let declared: BTreeMap<FunctionKey, BTreeSet<String>> = analysis
        .modules
        .iter()
        .flat_map(|(id, module)| {
            module.declarations.iter().filter_map(move |declaration| {
                let Declaration::Function(function) = declaration else {
                    return None;
                };
                Some((
                    (id.clone(), function.name.name.clone()),
                    function
                        .effects
                        .iter()
                        .map(|effect| effect.name.clone())
                        .collect(),
                ))
            })
        })
        .collect();
    let mut diagnostics = Vec::new();
    for (id, module) in &analysis.modules {
        for declaration in &module.declarations {
            match declaration {
                Declaration::Record(record) => {
                    for field in &record.fields {
                        reject_capability_type(&field.ty, &mut diagnostics);
                    }
                }
                Declaration::Enum(enumeration) => {
                    for payload in enumeration
                        .variants
                        .iter()
                        .flat_map(|variant| &variant.payloads)
                    {
                        reject_capability_type(payload, &mut diagnostics);
                    }
                }
                Declaration::Function(function) => {
                    let key = (id.clone(), function.name.name.clone());
                    let mut seen = BTreeSet::new();
                    for effect in &function.effects {
                        if !seen.insert(&effect.name) {
                            diagnostics.push(Diagnostic::error(
                                "UBI0011",
                                "Duplicate effect",
                                effect.span.clone(),
                            ));
                        } else if !matches!(effect.name.as_str(), "console" | "storage" | "http") {
                            diagnostics.push(Diagnostic::error(
                                "UBI0040",
                                "Unknown effect",
                                effect.span.clone(),
                            ));
                        }
                    }
                    if let Some(ty) = &function.return_type {
                        reject_capability_type(ty, &mut diagnostics);
                    }
                    if analysis
                        .signatures
                        .get(&key)
                        .and_then(|signature| signature.return_type)
                        .is_some_and(|ty| contains_capability(analysis, ty))
                    {
                        diagnostics.push(Diagnostic::error(
                            "UBI0040",
                            "Capabilities cannot be returned",
                            function.name.span.clone(),
                        ));
                    }
                    for parameter in &function.parameters {
                        for argument in &parameter.ty.arguments {
                            reject_capability_type(argument, &mut diagnostics);
                        }
                    }
                    let mut checker = Checker {
                        analysis,
                        declared: &declared,
                        key: &key,
                        diagnostics: &mut diagnostics,
                        asynchronous: function.asynchronous,
                        awaited: None,
                        scopes: vec![function
                            .parameters
                            .iter()
                            .map(|parameter| parameter.name.name.clone())
                            .collect()],
                    };
                    checker.block(&function.body, false);
                }
            }
        }
    }
    diagnostics
}

pub(crate) fn contains_capability(analysis: &Analysis, ty: ValueType) -> bool {
    let mut pending = vec![ty];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        match ty {
            ValueType::Capability(_) => return true,
            ValueType::Record(id) if seen.insert((false, id)) => {
                pending.extend(analysis.records[id].fields.values().copied())
            }
            ValueType::List(id)
            | ValueType::Option(id)
            | ValueType::Enum(id)
            | ValueType::Map(id)
                if seen.insert((true, id)) =>
            {
                match &analysis.types[id] {
                    TypeInfo::List(inner) | TypeInfo::Option(inner) => pending.push(*inner),
                    TypeInfo::Map { key, value } => pending.extend([*key, *value]),
                    TypeInfo::Enum { variants, .. } => {
                        pending.extend(variants.values().flatten().copied())
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    false
}

fn reject_capability_type(ty: &Type, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(ty.name.as_str(), "Console" | "Storage" | "Http") {
        diagnostics.push(Diagnostic::error(
            "UBI0040",
            "Capabilities cannot escape through aggregates or returns",
            ty.span.clone(),
        ));
    }
    for argument in &ty.arguments {
        reject_capability_type(argument, diagnostics);
    }
}

struct Checker<'a> {
    analysis: &'a Analysis,
    declared: &'a BTreeMap<FunctionKey, BTreeSet<String>>,
    key: &'a FunctionKey,
    diagnostics: &'a mut Vec<Diagnostic>,
    asynchronous: bool,
    awaited: Option<Span>,
    scopes: Vec<BTreeSet<String>>,
}

impl Checker<'_> {
    fn local(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn target(&self, expression: &Expr) -> Option<FunctionKey> {
        let ExprKind::Name(name) = &expression.kind else {
            return None;
        };
        if self.local(&name.name) {
            return None;
        }
        self.analysis
            .module_symbols
            .get(&self.key.0)?
            .get(&name.name)
            .cloned()
    }

    fn async_callee(&self, expression: &Expr) -> bool {
        if let Some(target) = self.target(expression) {
            return self
                .analysis
                .signatures
                .get(&target)
                .is_some_and(|signature| signature.asynchronous);
        }
        matches!(&expression.kind, ExprKind::Name(name) if !self.local(&name.name) && async_host(&name.name))
    }

    fn async_error(&mut self, expression: &Expr, message: &str) {
        self.diagnostics.push(Diagnostic::error(
            "UBI0041",
            message,
            expression.span.clone(),
        ));
    }

    fn ty(&self, expression: &Expr) -> Option<ValueType> {
        self.analysis
            .expression_types
            .get(&(self.key.clone(), expression.span.clone()))
            .copied()
    }

    fn error(&mut self, expression: &Expr, message: &str) {
        self.diagnostics.push(Diagnostic::error(
            "UBI0040",
            message,
            expression.span.clone(),
        ));
    }

    fn block(&mut self, block: &Block, arrow: bool) {
        self.scopes.push(BTreeSet::new());
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let {
                    name,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(ty) = annotation {
                        for argument in &ty.arguments {
                            reject_capability_type(argument, self.diagnostics);
                        }
                    }
                    self.expression(value, arrow, false);
                    self.scopes.last_mut().unwrap().insert(name.name.clone());
                }
                StatementKind::Assign { target, value } => {
                    self.expression(target, arrow, false);
                    self.expression(value, arrow, false);
                }
                StatementKind::Return(Some(value)) | StatementKind::Expression(value) => {
                    self.expression(value, arrow, false)
                }
                StatementKind::While { condition, body } => {
                    self.expression(condition, arrow, false);
                    self.block(body, arrow);
                }
                StatementKind::For {
                    name,
                    iterable,
                    body,
                } => {
                    self.expression(iterable, arrow, false);
                    self.scopes.push(BTreeSet::from([name.name.clone()]));
                    self.block(body, arrow);
                    self.scopes.pop();
                }
                _ => {}
            }
        }
        if let Some(tail) = &block.tail {
            self.expression(tail, arrow, false);
        }
        self.scopes.pop();
    }

    fn expression(&mut self, expression: &Expr, arrow: bool, direct_callee: bool) {
        if !direct_callee && self.async_callee(expression) {
            self.async_error(
                expression,
                "Async function values and aliases are unsupported",
            );
        }
        if arrow
            && self
                .ty(expression)
                .is_some_and(|ty| contains_capability(self.analysis, ty))
        {
            self.error(expression, "Arrows cannot capture or use capabilities");
        }
        let reference_target = self.target(expression).or_else(|| {
            self.analysis
                .reference_targets
                .get(&(self.key.clone(), expression.span.clone()))
                .cloned()
        });
        if let Some(target) = reference_target.as_ref() {
            let forbidden = self
                .analysis
                .signatures
                .get(target)
                .is_some_and(|signature| {
                    if signature.type_parameters.is_empty() {
                        return false;
                    }
                    let Some(ValueType::Function(id)) = self.ty(expression) else {
                        return false;
                    };
                    let TypeInfo::Function {
                        parameters,
                        return_type,
                    } = &self.analysis.types[id]
                    else {
                        return false;
                    };
                    let mut mapping = BTreeMap::new();
                    for (template, actual) in signature.parameters.iter().zip(parameters) {
                        infer_type_arguments(
                            *template,
                            *actual,
                            &mut mapping,
                            &self.analysis.records,
                            &self.analysis.types,
                        );
                    }
                    if let Some(template) = signature.return_type {
                        infer_type_arguments(
                            template,
                            *return_type,
                            &mut mapping,
                            &self.analysis.records,
                            &self.analysis.types,
                        );
                    }
                    signature.type_parameters.iter().any(|parameter| {
                        let ValueType::Parameter(id) = parameter else {
                            return false;
                        };
                        mapping
                            .get(id)
                            .is_some_and(|ty| contains_capability(self.analysis, *ty))
                    })
                });
            if forbidden {
                self.error(
                    expression,
                    "Capabilities cannot specialize generic function parameters",
                );
            }
            if !direct_callee
                && self
                    .declared
                    .get(target)
                    .is_some_and(|effects| !effects.is_empty())
            {
                self.error(expression, "Effectful function values are unsupported");
            }
        }
        match &expression.kind {
            ExprKind::Await(operand) => {
                if !self.asynchronous || arrow {
                    self.async_error(
                        expression,
                        "Await requires an async named function outside arrows",
                    );
                }
                if !matches!(&operand.kind, ExprKind::Call { callee, .. } if self.async_callee(callee))
                {
                    self.async_error(
                        operand,
                        "Await requires a direct async function or host call",
                    );
                }
                let previous = self.awaited.replace(operand.span.clone());
                self.expression(operand, arrow, false);
                self.awaited = previous;
            }
            ExprKind::Call { callee, arguments } => {
                if self.async_callee(callee)
                    && (arrow || self.awaited.as_ref() != Some(&expression.span))
                {
                    self.async_error(
                        expression,
                        "Async calls require immediate await outside arrows",
                    );
                }
                let target = self.target(callee);
                let needed = target
                    .as_ref()
                    .and_then(|target| self.declared.get(target))
                    .cloned()
                    .unwrap_or_default();
                let mut needed = needed;
                if let ExprKind::Name(name) = &callee.kind {
                    if !self.local(&name.name) && target.is_none() {
                        if let Some(effect) = host_effect(&name.name) {
                            needed.insert(effect.to_owned());
                        }
                    }
                }
                if !needed.is_empty() && (arrow || !needed.is_subset(&self.declared[self.key])) {
                    self.error(expression, "Call requires declared effects");
                }
                if let Some(signature) = target
                    .as_ref()
                    .and_then(|target| self.analysis.signatures.get(target))
                {
                    for (parameter, argument) in signature.parameters.iter().zip(arguments) {
                        if matches!(parameter, ValueType::Parameter(_))
                            && self
                                .ty(argument)
                                .is_some_and(|ty| contains_capability(self.analysis, ty))
                        {
                            self.error(
                                argument,
                                "Capabilities cannot pass through generic parameters",
                            );
                        }
                    }
                }
                self.expression(callee, arrow, matches!(callee.kind, ExprKind::Name(_)));
                for argument in arguments {
                    self.expression(argument, arrow, false);
                }
            }
            ExprKind::Arrow { parameters, body } => {
                for parameter in parameters {
                    reject_capability_type(&parameter.ty, self.diagnostics);
                }
                self.scopes.push(
                    parameters
                        .iter()
                        .map(|parameter| parameter.name.name.clone())
                        .collect(),
                );
                self.expression(body, true, false);
                self.scopes.pop();
            }
            ExprKind::List(values) => {
                for value in values {
                    if self
                        .ty(value)
                        .is_some_and(|ty| contains_capability(self.analysis, ty))
                    {
                        self.error(value, "Capabilities cannot be stored in lists");
                    }
                    self.expression(value, arrow, false);
                }
            }
            ExprKind::Record { base, fields, .. } => {
                if let Some(base) = base {
                    self.expression(base, arrow, false);
                }
                for (_, value) in fields {
                    if self
                        .ty(value)
                        .is_some_and(|ty| contains_capability(self.analysis, ty))
                    {
                        self.error(value, "Capabilities cannot be stored in records");
                    }
                    self.expression(value, arrow, false);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                if self
                    .ty(left)
                    .is_some_and(|ty| contains_capability(self.analysis, ty))
                    || self
                        .ty(right)
                        .is_some_and(|ty| contains_capability(self.analysis, ty))
                {
                    self.error(expression, "Capabilities cannot be compared or operated on");
                }
                self.expression(left, arrow, false);
                self.expression(right, arrow, false);
            }
            ExprKind::Unary { operand, .. } | ExprKind::Propagate(operand) => {
                self.expression(operand, arrow, false)
            }
            ExprKind::Member { object, .. } => self.expression(object, arrow, false),
            ExprKind::Index { object, index } => {
                self.expression(object, arrow, false);
                self.expression(index, arrow, false);
            }
            ExprKind::Block(block) => self.block(block, arrow),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expression(condition, arrow, false);
                self.block(then_branch, arrow);
                self.expression(else_branch, arrow, false);
            }
            ExprKind::Match { subject, arms } => {
                self.expression(subject, arrow, false);
                for arm in arms {
                    let mut bindings = BTreeSet::new();
                    pattern_bindings(&arm.pattern, &mut bindings);
                    self.scopes.push(bindings);
                    if let Some(guard) = &arm.guard {
                        self.expression(guard, arrow, false);
                    }
                    self.expression(&arm.body, arrow, false);
                    self.scopes.pop();
                }
            }
            _ => {}
        }
    }
}

fn pattern_bindings(pattern: &Pattern, bindings: &mut BTreeSet<String>) {
    match &pattern.kind {
        PatternKind::Binding(name) => {
            bindings.insert(name.name.clone());
        }
        PatternKind::Some(inner) => pattern_bindings(inner, bindings),
        PatternKind::Variant { payloads, .. } => {
            for payload in payloads {
                pattern_bindings(payload, bindings);
            }
        }
        _ => {}
    }
}
