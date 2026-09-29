use std::collections::{BTreeMap, BTreeSet};

use crate::analyzer::{Analysis, FunctionKey, ValueType};
use crate::diagnostics::Diagnostic;
use crate::lexer::Symbol;
use crate::parser::{
    Block, Declaration, Expr, ExprKind as AstExprKind, Module, StatementKind as AstStatementKind,
};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Program {
    pub(crate) modules: BTreeMap<String, ModuleIr>,
    pub(crate) functions: BTreeMap<FunctionKey, FunctionIr>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleIr {
    pub(crate) imports: BTreeMap<String, FunctionKey>,
    pub(crate) functions: Vec<FunctionKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FunctionIr {
    pub(crate) key: FunctionKey,
    pub(crate) exported: bool,
    pub(crate) parameters: Vec<ParameterIr>,
    pub(crate) return_type: ValueType,
    pub(crate) body: BlockIr,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParameterIr {
    pub(crate) name: String,
    pub(crate) ty: ValueType,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockIr {
    pub(crate) statements: Vec<StatementIr>,
    pub(crate) tail: Option<Box<ExprIr>>,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StatementIr {
    pub(crate) kind: StatementKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StatementKind {
    Let { name: String, value: ExprIr },
    Return(Option<ExprIr>),
    Expression(ExprIr),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExprIr {
    pub(crate) kind: ExprKind,
    pub(crate) ty: ValueType,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ExprKind {
    Integer(i32),
    Float(f64),
    String(String),
    Bool(bool),
    Unit,
    Local(String),
    Call {
        target: FunctionKey,
        arguments: Vec<ExprIr>,
    },
    Unary {
        operator: Symbol,
        operand: Box<ExprIr>,
    },
    Binary {
        left: Box<ExprIr>,
        operator: Symbol,
        right: Box<ExprIr>,
    },
    Block(Box<BlockIr>),
    If {
        condition: Box<ExprIr>,
        then_branch: Box<BlockIr>,
        else_branch: Box<ExprIr>,
    },
}

pub(crate) fn lower(analysis: &Analysis) -> Result<Program, Diagnostic> {
    if let Some(diagnostic) = analysis.diagnostics.first() {
        return Err(diagnostic.clone());
    }

    let mut modules = BTreeMap::new();
    let mut functions = BTreeMap::new();
    for (module_id, module) in &analysis.modules {
        let mut module_functions = Vec::new();
        for declaration in &module.declarations {
            let Declaration::Function(function) = declaration;
            let key = (module_id.clone(), function.name.name.clone());
            let signature = analysis
                .signatures
                .get(&key)
                .ok_or_else(|| invariant(&function.span))?;
            let return_type = signature
                .return_type
                .filter(|ty| *ty != ValueType::Error)
                .ok_or_else(|| invariant(&function.span))?;
            let parameters = function
                .parameters
                .iter()
                .zip(&signature.parameters)
                .map(|(parameter, ty)| ParameterIr {
                    name: parameter.name.name.clone(),
                    ty: *ty,
                    span: parameter.span.clone(),
                })
                .collect();
            let mut lowerer = FunctionLowerer {
                key: &key,
                module_id,
                module_symbols: &analysis.module_symbols,
                expression_types: &analysis.expression_types,
                scopes: vec![function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.name.name.clone())
                    .collect()],
            };
            let body = lowerer.lower_block(&function.body)?;
            module_functions.push(key.clone());
            functions.insert(
                key.clone(),
                FunctionIr {
                    key,
                    exported: function.exported,
                    parameters,
                    return_type,
                    body,
                    span: function.span.clone(),
                },
            );
        }

        let imports = imported_symbols(module_id, module, &analysis.module_symbols)?;
        modules.insert(
            module_id.clone(),
            ModuleIr {
                imports,
                functions: module_functions,
            },
        );
    }

    Ok(Program { modules, functions })
}

fn imported_symbols(
    module_id: &str,
    module: &Module,
    symbols: &BTreeMap<String, BTreeMap<String, FunctionKey>>,
) -> Result<BTreeMap<String, FunctionKey>, Diagnostic> {
    let Some(resolved) = symbols.get(module_id) else {
        return match module.imports.first() {
            Some(import) => Err(invariant(&import.span)),
            None => Ok(BTreeMap::new()),
        };
    };
    let mut imports = BTreeMap::new();
    for import in &module.imports {
        for name in &import.names {
            let target = resolved
                .get(&name.name)
                .cloned()
                .ok_or_else(|| invariant(&name.span))?;
            imports.insert(name.name.clone(), target);
        }
    }
    Ok(imports)
}

struct FunctionLowerer<'a> {
    key: &'a FunctionKey,
    module_id: &'a str,
    module_symbols: &'a BTreeMap<String, BTreeMap<String, FunctionKey>>,
    expression_types: &'a BTreeMap<(FunctionKey, Span), ValueType>,
    scopes: Vec<BTreeSet<String>>,
}

impl FunctionLowerer<'_> {
    fn lower_block(&mut self, block: &Block) -> Result<BlockIr, Diagnostic> {
        self.scopes.push(BTreeSet::new());
        let mut statements = Vec::new();
        let mut completes = true;
        for statement in &block.statements {
            if !completes {
                break;
            }
            let kind = match &statement.kind {
                AstStatementKind::Let { name, value, .. } => {
                    let value = self.lower_expr(value)?;
                    if value.ty == ValueType::Never {
                        completes = false;
                    } else {
                        self.scopes.last_mut().unwrap().insert(name.name.clone());
                    }
                    StatementKind::Let {
                        name: name.name.clone(),
                        value,
                    }
                }
                AstStatementKind::Return(value) => {
                    let value = value
                        .as_ref()
                        .map(|expression| self.lower_expr(expression))
                        .transpose()?;
                    completes = false;
                    StatementKind::Return(value)
                }
                AstStatementKind::Expression(expression) => {
                    let expression = self.lower_expr(expression)?;
                    completes = expression.ty != ValueType::Never;
                    StatementKind::Expression(expression)
                }
                AstStatementKind::Assign { target, .. } => {
                    return Err(invariant(&target.span));
                }
            };
            statements.push(StatementIr {
                kind,
                span: statement.span.clone(),
            });
        }
        let tail = if completes {
            block
                .tail
                .as_ref()
                .map(|expression| self.lower_expr(expression).map(Box::new))
                .transpose()?
        } else {
            None
        };
        self.scopes.pop();
        Ok(BlockIr {
            statements,
            tail,
            span: block.span.clone(),
        })
    }

    fn lower_expr(&mut self, expression: &Expr) -> Result<ExprIr, Diagnostic> {
        let ty = *self
            .expression_types
            .get(&(self.key.clone(), expression.span.clone()))
            .ok_or_else(|| invariant(&expression.span))?;
        let kind = match &expression.kind {
            AstExprKind::Integer { value, .. } => ExprKind::Integer(
                value
                    .parse::<i32>()
                    .map_err(|_| invariant(&expression.span))?,
            ),
            AstExprKind::Float { value, .. } => ExprKind::Float(
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invariant(&expression.span))?,
            ),
            AstExprKind::String(value) => ExprKind::String(value.clone()),
            AstExprKind::Bool(value) => ExprKind::Bool(*value),
            AstExprKind::Unit => ExprKind::Unit,
            AstExprKind::Name(name) => {
                if !self.is_local(&name.name) {
                    return Err(invariant(&name.span));
                }
                ExprKind::Local(name.name.clone())
            }
            AstExprKind::Unary { operator, operand } => {
                let direct_negative = if *operator == Symbol::Minus {
                    match &operand.kind {
                        AstExprKind::Integer {
                            value,
                            literal_span,
                        } if operand.span == *literal_span => {
                            let magnitude = value
                                .parse::<u64>()
                                .map_err(|_| invariant(&expression.span))?;
                            Some(ExprKind::Integer(if magnitude == 2_147_483_648 {
                                i32::MIN
                            } else {
                                -(i32::try_from(magnitude)
                                    .map_err(|_| invariant(&expression.span))?)
                            }))
                        }
                        AstExprKind::Float {
                            value,
                            literal_span,
                        } if operand.span == *literal_span => Some(ExprKind::Float(
                            -value
                                .parse::<f64>()
                                .map_err(|_| invariant(&expression.span))?,
                        )),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(value) = direct_negative {
                    value
                } else {
                    ExprKind::Unary {
                        operator: *operator,
                        operand: Box::new(self.lower_expr(operand)?),
                    }
                }
            }
            AstExprKind::Binary {
                left,
                operator,
                right,
            } => ExprKind::Binary {
                left: Box::new(self.lower_expr(left)?),
                operator: *operator,
                right: Box::new(self.lower_expr(right)?),
            },
            AstExprKind::Call { callee, arguments } => {
                let AstExprKind::Name(name) = &callee.kind else {
                    return Err(invariant(&callee.span));
                };
                let target = self
                    .module_symbols
                    .get(self.module_id)
                    .and_then(|symbols| symbols.get(&name.name))
                    .cloned()
                    .ok_or_else(|| invariant(&name.span))?;
                ExprKind::Call {
                    target,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.lower_expr(argument))
                        .collect::<Result<_, _>>()?,
                }
            }
            AstExprKind::Block(block) => ExprKind::Block(Box::new(self.lower_block(block)?)),
            AstExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => ExprKind::If {
                condition: Box::new(self.lower_expr(condition)?),
                then_branch: Box::new(self.lower_block(then_branch)?),
                else_branch: Box::new(self.lower_expr(else_branch)?),
            },
            AstExprKind::Member { .. } | AstExprKind::Index { .. } | AstExprKind::Propagate(_) => {
                return Err(invariant(&expression.span))
            }
        };
        Ok(ExprIr {
            kind,
            ty,
            span: expression.span.clone(),
        })
    }

    fn is_local(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
}

fn invariant(span: &Span) -> Diagnostic {
    Diagnostic::error(
        "UBI0090",
        "Compiler invariant failed while lowering checked code",
        span.clone(),
    )
}
