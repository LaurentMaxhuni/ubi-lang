use std::collections::BTreeMap;
use std::rc::Rc;

use crate::analyzer::{Analysis, FunctionKey, Signature, ValueType};
use crate::diagnostics::Severity;
use crate::lexer::Symbol;
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Function, Module, Statement, StatementKind,
};

const MAX_STEPS: usize = 1_000_000;
const MAX_CALL_DEPTH: usize = 32;

#[derive(Debug, Clone)]
pub(crate) enum Value {
    Int(i32),
    Float(f64),
    Bool(bool),
    String(String),
    Unit,
    Record {
        type_id: usize,
        fields: Rc<BTreeMap<String, Value>>,
    },
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Unit, Self::Unit) => true,
            (
                Self::Record {
                    type_id: a,
                    fields: af,
                },
                Self::Record {
                    type_id: b,
                    fields: bf,
                },
            ) => {
                a == b
                    && af.len() == bf.len()
                    && af
                        .iter()
                        .zip(bf.iter())
                        .all(|((ak, av), (bk, bv))| ak == bk && av == bv)
            }
            _ => false,
        }
    }
}

impl Value {
    fn ty(&self) -> ValueType {
        match self {
            Self::Int(_) => ValueType::Int,
            Self::Float(_) => ValueType::Float,
            Self::Bool(_) => ValueType::Bool,
            Self::String(_) => ValueType::String,
            Self::Unit => ValueType::Unit,
            Self::Record { type_id, .. } => ValueType::Record(*type_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFault {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InvocationError {
    InvalidProgram,
    InternalInvariant,
    UnknownExport,
    NotExported,
    InvalidArguments,
    Runtime(RuntimeFault),
}

#[derive(Debug)]
enum Completion {
    Value(Value),
    Return(Value),
}

#[derive(Debug)]
enum EvalError {
    Invariant,
    Runtime(RuntimeFault),
}

pub(crate) fn invoke(
    analysis: &Analysis,
    key: &FunctionKey,
    arguments: Vec<Value>,
) -> Result<Value, InvocationError> {
    if analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Err(InvocationError::InvalidProgram);
    }
    let function = find_function(&analysis.modules, key).ok_or(InvocationError::UnknownExport)?;
    if !function.exported {
        return Err(InvocationError::NotExported);
    }
    let signature = analysis
        .signatures
        .get(key)
        .ok_or(InvocationError::InvalidProgram)?;
    if !valid_arguments(&arguments, signature, &analysis.records) {
        return Err(InvocationError::InvalidArguments);
    }

    Interpreter {
        analysis,
        remaining_steps: MAX_STEPS,
        call_depth: 0,
        scopes: Vec::new(),
    }
    .call_function(key, arguments)
    .map_err(|error| match error {
        EvalError::Invariant => InvocationError::InternalInvariant,
        EvalError::Runtime(fault) => InvocationError::Runtime(fault),
    })
}

fn valid_arguments(
    arguments: &[Value],
    signature: &Signature,
    records: &[crate::analyzer::RecordInfo],
) -> bool {
    arguments.len() == signature.parameters.len()
        && arguments
            .iter()
            .zip(&signature.parameters)
            .all(|(value, expected)| valid_value(value, *expected, records, 0))
}

fn valid_value(
    value: &Value,
    ty: ValueType,
    records: &[crate::analyzer::RecordInfo],
    depth: usize,
) -> bool {
    if value.ty() != ty {
        return false;
    }
    if let Value::Record { type_id, fields } = value {
        if depth >= 32 {
            return false;
        }
        let Some(record) = records.get(*type_id) else {
            return false;
        };
        return fields.len() == record.fields.len()
            && record.fields.iter().all(|(name, ty)| {
                fields
                    .get(name)
                    .is_some_and(|value| valid_value(value, *ty, records, depth + 1))
            });
    }
    true
}

fn record_depth(value: &Value) -> usize {
    match value {
        Value::Record { fields, .. } => 1 + fields.values().map(record_depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn find_function<'a>(
    modules: &'a BTreeMap<String, Module>,
    key: &FunctionKey,
) -> Option<&'a Function> {
    modules
        .get(&key.0)?
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            Declaration::Function(function) if function.name.name == key.1 => Some(function),
            _ => None,
        })
}

struct Interpreter<'a> {
    analysis: &'a Analysis,
    remaining_steps: usize,
    call_depth: usize,
    scopes: Vec<BTreeMap<String, Value>>,
}

impl Interpreter<'_> {
    fn call_function(
        &mut self,
        key: &FunctionKey,
        arguments: Vec<Value>,
    ) -> Result<Value, EvalError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(resource_fault());
        }
        self.step()?;
        let function = find_function(&self.analysis.modules, key)
            .cloned()
            .ok_or_else(invariant_error)?;
        let signature = self
            .analysis
            .signatures
            .get(key)
            .cloned()
            .ok_or_else(invariant_error)?;
        if !valid_arguments(&arguments, &signature, &self.analysis.records) {
            return Err(invariant_error());
        }

        let caller_scopes = std::mem::take(&mut self.scopes);
        self.call_depth += 1;
        self.scopes.push(
            function
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, value)| (parameter.name.name.clone(), value))
                .collect(),
        );
        let result = self.eval_block(&function.body);
        self.scopes = caller_scopes;
        self.call_depth -= 1;

        let value = match result? {
            Completion::Value(value) | Completion::Return(value) => value,
        };
        let Some(expected) = signature.return_type else {
            return Err(invariant_error());
        };
        if value.ty() != expected {
            return Err(invariant_error());
        }
        Ok(value)
    }

    fn eval_block(&mut self, block: &Block) -> Result<Completion, EvalError> {
        self.scopes.push(BTreeMap::new());
        let result = self.eval_block_in_scope(block);
        self.scopes.pop();
        result
    }

    fn eval_block_in_scope(&mut self, block: &Block) -> Result<Completion, EvalError> {
        for statement in &block.statements {
            self.step()?;
            match self.eval_statement(statement)? {
                Completion::Value(_) => {}
                Completion::Return(value) => return Ok(Completion::Return(value)),
            }
        }
        match &block.tail {
            Some(tail) => self.eval_expr(tail),
            None => Ok(Completion::Value(Value::Unit)),
        }
    }

    fn eval_statement(&mut self, statement: &Statement) -> Result<Completion, EvalError> {
        match &statement.kind {
            StatementKind::Let { name, value, .. } => match self.eval_expr(value)? {
                Completion::Value(value) => {
                    self.scopes
                        .last_mut()
                        .ok_or_else(invariant_error)?
                        .insert(name.name.clone(), value);
                    Ok(Completion::Value(Value::Unit))
                }
                Completion::Return(value) => Ok(Completion::Return(value)),
            },
            StatementKind::Assign { target, value } => {
                let ExprKind::Name(name) = &target.kind else {
                    return Err(invariant_error());
                };
                let scope = self
                    .scopes
                    .iter()
                    .rposition(|scope| scope.contains_key(&name.name))
                    .ok_or_else(invariant_error)?;
                match self.eval_expr(value)? {
                    Completion::Value(value) => {
                        self.scopes[scope].insert(name.name.clone(), value);
                        Ok(Completion::Value(Value::Unit))
                    }
                    Completion::Return(value) => Ok(Completion::Return(value)),
                }
            }
            StatementKind::Return(value) => {
                let value = match value {
                    Some(expression) => match self.eval_expr(expression)? {
                        Completion::Value(value) | Completion::Return(value) => value,
                    },
                    None => Value::Unit,
                };
                Ok(Completion::Return(value))
            }
            StatementKind::Expression(expression) => self.eval_expr(expression),
        }
    }

    fn eval_expr(&mut self, expression: &Expr) -> Result<Completion, EvalError> {
        self.step()?;
        match &expression.kind {
            ExprKind::Integer { value, .. } => value
                .parse::<i32>()
                .map(Value::Int)
                .map(Completion::Value)
                .map_err(|_| invariant_error()),
            ExprKind::Float { value, .. } => value
                .parse::<f64>()
                .map(Value::Float)
                .map(Completion::Value)
                .map_err(|_| invariant_error()),
            ExprKind::String(value) => Ok(Completion::Value(Value::String(value.clone()))),
            ExprKind::Bool(value) => Ok(Completion::Value(Value::Bool(*value))),
            ExprKind::Unit => Ok(Completion::Value(Value::Unit)),
            ExprKind::Name(name) => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(&name.name))
                .cloned()
                .map(Completion::Value)
                .ok_or_else(invariant_error),
            ExprKind::Unary { operator, operand } => {
                if *operator == Symbol::Minus {
                    if let Some(value) = self.eval_direct_negative_integer(operand)? {
                        return Ok(Completion::Value(value));
                    }
                }
                let value = match self.eval_expr(operand)? {
                    Completion::Value(value) => value,
                    Completion::Return(value) => return Ok(Completion::Return(value)),
                };
                Ok(Completion::Value(eval_unary(*operator, value)?))
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_value = match self.eval_expr(left)? {
                    Completion::Value(value) => value,
                    Completion::Return(value) => return Ok(Completion::Return(value)),
                };
                match (operator, &left_value) {
                    (Symbol::AndAnd, Value::Bool(false)) => {
                        return Ok(Completion::Value(Value::Bool(false)));
                    }
                    (Symbol::OrOr, Value::Bool(true)) => {
                        return Ok(Completion::Value(Value::Bool(true)));
                    }
                    (Symbol::AndAnd | Symbol::OrOr, Value::Bool(_)) => {}
                    (Symbol::AndAnd | Symbol::OrOr, _) => return Err(invariant_error()),
                    _ => {}
                }
                let right_value = match self.eval_expr(right)? {
                    Completion::Value(value) => value,
                    Completion::Return(value) => return Ok(Completion::Return(value)),
                };
                Ok(Completion::Value(eval_binary(
                    *operator,
                    left_value,
                    right_value,
                )?))
            }
            ExprKind::Call { callee, arguments } => {
                let ExprKind::Name(name) = &callee.kind else {
                    return Err(invariant_error());
                };
                let target = self
                    .analysis
                    .module_symbols
                    .get(&expression.span.source_id)
                    .and_then(|symbols| symbols.get(&name.name))
                    .cloned()
                    .ok_or_else(invariant_error)?;
                let mut values = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    match self.eval_expr(argument)? {
                        Completion::Value(value) => values.push(value),
                        Completion::Return(value) => return Ok(Completion::Return(value)),
                    }
                }
                self.call_function(&target, values).map(Completion::Value)
            }
            ExprKind::Block(block) => self.eval_block(block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = match self.eval_expr(condition)? {
                    Completion::Value(value) => value,
                    Completion::Return(value) => return Ok(Completion::Return(value)),
                };
                match condition {
                    Value::Bool(true) => self.eval_block(then_branch),
                    Value::Bool(false) => self.eval_expr(else_branch),
                    _ => Err(invariant_error()),
                }
            }
            ExprKind::Record { name, base, fields } => {
                let key = self
                    .analysis
                    .module_symbols
                    .get(&expression.span.source_id)
                    .and_then(|symbols| symbols.get(&name.name))
                    .ok_or_else(invariant_error)?;
                let type_id = *self
                    .analysis
                    .record_keys
                    .get(key)
                    .ok_or_else(invariant_error)?;
                let mut values = if let Some(base) = base {
                    match self.eval_expr(base)? {
                        Completion::Return(value) => return Ok(Completion::Return(value)),
                        Completion::Value(Value::Record { fields, .. }) => fields.as_ref().clone(),
                        _ => return Err(invariant_error()),
                    }
                } else {
                    BTreeMap::new()
                };
                for (field, expression) in fields {
                    match self.eval_expr(expression)? {
                        Completion::Value(value) => {
                            values.insert(field.name.clone(), value);
                        }
                        Completion::Return(value) => return Ok(Completion::Return(value)),
                    }
                }
                if values.values().map(record_depth).max().unwrap_or(0) >= 32 {
                    return Err(resource_fault());
                }
                Ok(Completion::Value(Value::Record {
                    type_id,
                    fields: Rc::new(values),
                }))
            }
            ExprKind::Member { object, name } => match self.eval_expr(object)? {
                Completion::Return(value) => Ok(Completion::Return(value)),
                Completion::Value(Value::Record { fields, .. }) => fields
                    .get(&name.name)
                    .cloned()
                    .map(Completion::Value)
                    .ok_or_else(invariant_error),
                _ => Err(invariant_error()),
            },
            ExprKind::Index { .. } | ExprKind::Propagate(_) => Err(invariant_error()),
        }
    }

    fn eval_direct_negative_integer(&mut self, operand: &Expr) -> Result<Option<Value>, EvalError> {
        let ExprKind::Integer {
            value,
            literal_span,
        } = &operand.kind
        else {
            return Ok(None);
        };
        if operand.span != *literal_span || value != "2147483648" {
            return Ok(None);
        }
        self.step()?;
        Ok(Some(Value::Int(i32::MIN)))
    }

    fn step(&mut self) -> Result<(), EvalError> {
        if self.remaining_steps == 0 {
            return Err(resource_fault());
        }
        self.remaining_steps -= 1;
        Ok(())
    }
}

fn eval_unary(operator: Symbol, value: Value) -> Result<Value, EvalError> {
    match (operator, value) {
        (Symbol::Minus, Value::Int(value)) => value
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(overflow_fault),
        (Symbol::Minus, Value::Float(value)) => Ok(Value::Float(-value)),
        (Symbol::Bang, Value::Bool(value)) => Ok(Value::Bool(!value)),
        _ => Err(invariant_error()),
    }
}

fn eval_binary(operator: Symbol, left: Value, right: Value) -> Result<Value, EvalError> {
    if matches!(operator, Symbol::EqualEqual | Symbol::BangEqual) {
        let equal = left == right;
        return Ok(Value::Bool(if operator == Symbol::EqualEqual {
            equal
        } else {
            !equal
        }));
    }
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => match operator {
            Symbol::Plus => left
                .checked_add(right)
                .map(Value::Int)
                .ok_or_else(overflow_fault),
            Symbol::Minus => left
                .checked_sub(right)
                .map(Value::Int)
                .ok_or_else(overflow_fault),
            Symbol::Star => left
                .checked_mul(right)
                .map(Value::Int)
                .ok_or_else(overflow_fault),
            Symbol::Slash if right == 0 => Err(division_by_zero_fault()),
            Symbol::Slash if left == i32::MIN && right == -1 => Err(overflow_fault()),
            Symbol::Slash => Ok(Value::Int(left / right)),
            Symbol::Percent if right == 0 => Err(division_by_zero_fault()),
            Symbol::Percent if left == i32::MIN && right == -1 => Ok(Value::Int(0)),
            Symbol::Percent => Ok(Value::Int(left % right)),
            Symbol::EqualEqual => Ok(Value::Bool(left == right)),
            Symbol::BangEqual => Ok(Value::Bool(left != right)),
            Symbol::Less => Ok(Value::Bool(left < right)),
            Symbol::LessEqual => Ok(Value::Bool(left <= right)),
            Symbol::Greater => Ok(Value::Bool(left > right)),
            Symbol::GreaterEqual => Ok(Value::Bool(left >= right)),
            _ => Err(invariant_error()),
        },
        (Value::Float(left), Value::Float(right)) => match operator {
            Symbol::Plus => Ok(Value::Float(left + right)),
            Symbol::Minus => Ok(Value::Float(left - right)),
            Symbol::Star => Ok(Value::Float(left * right)),
            Symbol::Slash => Ok(Value::Float(left / right)),
            Symbol::EqualEqual => Ok(Value::Bool(left == right)),
            Symbol::BangEqual => Ok(Value::Bool(left != right)),
            Symbol::Less => Ok(Value::Bool(left < right)),
            Symbol::LessEqual => Ok(Value::Bool(left <= right)),
            Symbol::Greater => Ok(Value::Bool(left > right)),
            Symbol::GreaterEqual => Ok(Value::Bool(left >= right)),
            _ => Err(invariant_error()),
        },
        (Value::Bool(left), Value::Bool(right)) => match operator {
            Symbol::AndAnd => Ok(Value::Bool(left && right)),
            Symbol::OrOr => Ok(Value::Bool(left || right)),
            Symbol::EqualEqual => Ok(Value::Bool(left == right)),
            Symbol::BangEqual => Ok(Value::Bool(left != right)),
            _ => Err(invariant_error()),
        },
        (Value::String(left), Value::String(right)) => match operator {
            Symbol::Plus => Ok(Value::String(left + &right)),
            Symbol::EqualEqual => Ok(Value::Bool(left == right)),
            Symbol::BangEqual => Ok(Value::Bool(left != right)),
            _ => Err(invariant_error()),
        },
        (Value::Unit, Value::Unit) => match operator {
            Symbol::EqualEqual => Ok(Value::Bool(true)),
            Symbol::BangEqual => Ok(Value::Bool(false)),
            _ => Err(invariant_error()),
        },
        _ => Err(invariant_error()),
    }
}

fn overflow_fault() -> EvalError {
    EvalError::Runtime(RuntimeFault {
        code: "UBI-R0001",
        message: "Integer overflow".to_owned(),
    })
}

fn division_by_zero_fault() -> EvalError {
    EvalError::Runtime(RuntimeFault {
        code: "UBI-R0002",
        message: "Integer division or remainder by zero".to_owned(),
    })
}

fn resource_fault() -> EvalError {
    EvalError::Runtime(RuntimeFault {
        code: "UBI-R0005",
        message: "Runtime execution limit exceeded".to_owned(),
    })
}

fn invariant_error() -> EvalError {
    EvalError::Invariant
}
