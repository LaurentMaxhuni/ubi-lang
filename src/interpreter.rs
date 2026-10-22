use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::analyzer::{
    builtin_name, contains_function, infer_type_arguments, substitute_type_existing, Analysis,
    CapabilityKind, FunctionKey, Signature, TypeInfo, ValueType,
};
use crate::diagnostics::Severity;
use crate::lexer::Symbol;
use crate::parser::{
    Block, Declaration, Expr, ExprKind, Function, Module, Pattern, PatternKind, Statement,
    StatementKind,
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
    List {
        type_id: usize,
        elements: Rc<Vec<Value>>,
    },
    Tuple {
        type_id: usize,
        elements: Rc<Vec<Value>>,
    },
    Map {
        type_id: usize,
        entries: Rc<Vec<(Value, Value)>>,
    },
    Option {
        type_id: usize,
        value: Option<Rc<Value>>,
    },
    Enum {
        type_id: usize,
        tag: Rc<String>,
        values: Rc<Vec<Value>>,
    },
    Function {
        type_id: usize,
        function: Rc<FunctionValue>,
    },
    #[cfg_attr(not(test), allow(dead_code))] // Native pure exports receive no host authority.
    Capability(Rc<HostCapability>),
    Range {
        start: i32,
        end: i32,
    },
}

type HostCallback = dyn Fn(&str, &[String]) -> Result<String, ()>;

pub(crate) struct HostCapability {
    kind: CapabilityKind,
    callback: Box<HostCallback>,
}

impl HostCapability {
    #[cfg(test)]
    pub(crate) fn new(
        kind: CapabilityKind,
        callback: impl Fn(&str, &[String]) -> Result<String, ()> + 'static,
    ) -> Self {
        Self {
            kind,
            callback: Box::new(callback),
        }
    }
}

impl std::fmt::Debug for HostCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostCapability")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FunctionValue {
    Named(FunctionKey),
    Closure {
        parameters: Vec<String>,
        body: Box<Expr>,
        captures: BTreeMap<String, Value>,
        key: FunctionKey,
        type_arguments: BTreeMap<usize, ValueType>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AggregateKey {
    Record(usize, usize),
    List(usize, usize),
    Tuple(usize, usize),
    Map(usize, usize),
    Option(usize, usize),
    Enum(usize, usize, usize),
}

fn aggregate_key(value: &Value) -> Option<AggregateKey> {
    match value {
        Value::Record { type_id, fields } => {
            Some(AggregateKey::Record(*type_id, Rc::as_ptr(fields) as usize))
        }
        Value::List { type_id, elements } => {
            Some(AggregateKey::List(*type_id, Rc::as_ptr(elements) as usize))
        }
        Value::Tuple { type_id, elements } => {
            Some(AggregateKey::Tuple(*type_id, Rc::as_ptr(elements) as usize))
        }
        Value::Map { type_id, entries } => {
            Some(AggregateKey::Map(*type_id, Rc::as_ptr(entries) as usize))
        }
        Value::Option {
            type_id,
            value: Some(value),
        } => Some(AggregateKey::Option(*type_id, Rc::as_ptr(value) as usize)),
        Value::Enum {
            type_id,
            tag,
            values,
        } => Some(AggregateKey::Enum(
            *type_id,
            Rc::as_ptr(tag) as usize,
            Rc::as_ptr(values) as usize,
        )),
        _ => None,
    }
}

fn values_equal(
    left: &Value,
    right: &Value,
    cache: &mut BTreeMap<(AggregateKey, AggregateKey), bool>,
) -> bool {
    let pair = aggregate_key(left).zip(aggregate_key(right));
    if let Some(result) = pair.and_then(|pair| cache.get(&pair)) {
        return *result;
    }
    let equal = match (left, right) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Unit, Value::Unit) => true,
        (
            Value::Record {
                type_id: a,
                fields: af,
            },
            Value::Record {
                type_id: b,
                fields: bf,
            },
        ) => {
            a == b
                && af.len() == bf.len()
                && af.iter().all(|(name, value)| {
                    bf.get(name)
                        .is_some_and(|other| values_equal(value, other, cache))
                })
        }
        (
            Value::Tuple {
                type_id: a,
                elements: av,
            },
            Value::Tuple {
                type_id: b,
                elements: bv,
            },
        )
        | (
            Value::List {
                type_id: a,
                elements: av,
            },
            Value::List {
                type_id: b,
                elements: bv,
            },
        ) => {
            a == b
                && av.len() == bv.len()
                && av
                    .iter()
                    .zip(bv.iter())
                    .all(|(left, right)| values_equal(left, right, cache))
        }
        (
            Value::Map {
                type_id: a,
                entries: av,
            },
            Value::Map {
                type_id: b,
                entries: bv,
            },
        ) => {
            a == b
                && av.len() == bv.len()
                && av.iter().all(|(key, value)| {
                    bv.iter().any(|(other_key, other_value)| {
                        values_equal(key, other_key, cache)
                            && values_equal(value, other_value, cache)
                    })
                })
        }
        (
            Value::Option {
                type_id: a,
                value: av,
            },
            Value::Option {
                type_id: b,
                value: bv,
            },
        ) => {
            a == b
                && match (av, bv) {
                    (None, None) => true,
                    (Some(left), Some(right)) => values_equal(left, right, cache),
                    _ => false,
                }
        }
        (
            Value::Enum {
                type_id: a,
                tag: at,
                values: av,
            },
            Value::Enum {
                type_id: b,
                tag: bt,
                values: bv,
            },
        ) => {
            a == b
                && at == bt
                && av.len() == bv.len()
                && av
                    .iter()
                    .zip(bv.iter())
                    .all(|(left, right)| values_equal(left, right, cache))
        }
        _ => false,
    };
    if let Some(pair) = pair {
        cache.insert(pair, equal);
    }
    equal
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        values_equal(self, other, &mut BTreeMap::new())
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
            Self::List { type_id, .. } => ValueType::List(*type_id),
            Self::Tuple { type_id, .. } => ValueType::Tuple(*type_id),
            Self::Map { type_id, .. } => ValueType::Map(*type_id),
            Self::Option { type_id, .. } => ValueType::Option(*type_id),
            Self::Enum { type_id, .. } => ValueType::Enum(*type_id),
            Self::Function { type_id, .. } => ValueType::Function(*type_id),
            Self::Capability(capability) => ValueType::Capability(capability.kind),
            Self::Range { .. } => ValueType::Range,
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
    Break,
    Continue,
}
#[derive(Debug)]
enum EvalError {
    Invariant,
    Runtime(RuntimeFault),
}

// Every nested expression preserves nonlocal control flow through its caller.
macro_rules! value {
    ($expression:expr) => {
        match $expression? {
            Completion::Value(value) => value,
            completion => return Ok(completion),
        }
    };
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
    if !signature.type_parameters.is_empty()
        || signature
            .parameters
            .iter()
            .copied()
            .any(|ty| contains_function(ty, &analysis.records, &analysis.types))
        || signature
            .return_type
            .is_some_and(|ty| contains_function(ty, &analysis.records, &analysis.types))
    {
        return Err(InvocationError::InvalidArguments);
    }
    if !valid_arguments(&arguments, signature, analysis, true) {
        return Err(InvocationError::InvalidArguments);
    }
    Interpreter {
        analysis,
        remaining_steps: MAX_STEPS,
        call_depth: 0,
        scopes: Vec::new(),
        key: key.clone(),
        type_arguments: BTreeMap::new(),
    }
    .call_function(key, arguments, None)
    .map_err(|error| match error {
        EvalError::Invariant => InvocationError::InternalInvariant,
        EvalError::Runtime(fault) => InvocationError::Runtime(fault),
    })
}

fn valid_arguments(
    arguments: &[Value],
    signature: &Signature,
    analysis: &Analysis,
    host: bool,
) -> bool {
    arguments.len() == signature.parameters.len()
        && arguments
            .iter()
            .zip(&signature.parameters)
            .all(|(value, expected)| {
                if host {
                    valid_value(value, *expected, analysis, 0, true)
                } else {
                    value.ty() == *expected
                }
            })
}

fn valid_value(
    value: &Value,
    ty: ValueType,
    analysis: &Analysis,
    depth: usize,
    host: bool,
) -> bool {
    if value.ty() != ty {
        return false;
    }
    match value {
        Value::Record { type_id, fields } => {
            depth < 32
                && analysis.records.get(*type_id).is_some_and(|record| {
                    fields.len() == record.fields.len()
                        && record.fields.iter().all(|(name, ty)| {
                            fields.get(name).is_some_and(|value| {
                                valid_value(value, *ty, analysis, depth + 1, host)
                            })
                        })
                })
        }
        Value::List { type_id, elements } => {
            depth < 32
                && matches!(analysis.types.get(*type_id), Some(TypeInfo::List(ty))
            if elements.iter().all(|value| valid_value(value, *ty, analysis, depth + 1, host)))
        }
        Value::Tuple { type_id, elements } => {
            let Some(TypeInfo::Tuple(types)) = analysis.types.get(*type_id) else {
                return false;
            };
            depth < 32
                && elements.len() == types.len()
                && elements
                    .iter()
                    .zip(types)
                    .all(|(value, ty)| valid_value(value, *ty, analysis, depth + 1, host))
        }
        Value::Map { type_id, entries } => {
            let Some(TypeInfo::Map { key, value }) = analysis.types.get(*type_id) else {
                return false;
            };
            let mut keys = BTreeSet::new();
            depth < 32
                && entries.iter().all(|(entry_key, entry_value)| {
                    valid_value(entry_key, *key, analysis, depth + 1, host)
                        && valid_value(entry_value, *value, analysis, depth + 1, host)
                        && keys.insert(match entry_key {
                            Value::Int(value) => value.to_string(),
                            Value::Bool(value) => value.to_string(),
                            Value::String(value) => value.clone(),
                            _ => return false,
                        })
                })
        }
        Value::Option { type_id, value } => {
            depth < 32
                && matches!(analysis.types.get(*type_id), Some(TypeInfo::Option(ty))
            if value.as_ref().is_none_or(|value| valid_value(value, *ty, analysis, depth + 1, host)))
        }
        Value::Function { .. } | Value::Range { .. } => !host,
        Value::Enum {
            type_id,
            tag,
            values,
        } => {
            depth < 32
                && matches!(analysis.types.get(*type_id), Some(TypeInfo::Enum { variants, .. })
                if variants.get(tag.as_str()).is_some_and(|payloads| payloads.len() == values.len()
                    && payloads.iter().zip(values.iter()).all(|(ty, value)| valid_value(value, *ty, analysis, depth + 1, host))))
        }
        _ => true,
    }
}

fn aggregate_depth(value: &Value) -> usize {
    fn depth(value: &Value, cache: &mut BTreeMap<(u8, usize), usize>) -> usize {
        let key = match value {
            Value::Record { fields, .. } => Some((0, Rc::as_ptr(fields) as usize)),
            Value::List { elements, .. } => Some((1, Rc::as_ptr(elements) as usize)),
            Value::Tuple { elements, .. } => Some((2, Rc::as_ptr(elements) as usize)),
            Value::Map { entries, .. } => Some((3, Rc::as_ptr(entries) as usize)),
            Value::Option {
                value: Some(value), ..
            } => Some((4, Rc::as_ptr(value) as usize)),
            Value::Enum { values, .. } => Some((5, Rc::as_ptr(values) as usize)),
            _ => None,
        };
        if let Some(key) = key {
            if let Some(depth) = cache.get(&key) {
                return *depth;
            }
        }
        let depth = match value {
            Value::Record { fields, .. } => {
                1 + fields
                    .values()
                    .map(|value| depth(value, cache))
                    .max()
                    .unwrap_or(0)
            }
            Value::List { elements, .. } | Value::Tuple { elements, .. } => {
                1 + elements
                    .iter()
                    .map(|value| depth(value, cache))
                    .max()
                    .unwrap_or(0)
            }
            Value::Map { entries, .. } => {
                1 + entries
                    .iter()
                    .map(|(key, value)| depth(key, cache).max(depth(value, cache)))
                    .max()
                    .unwrap_or(0)
            }
            Value::Option { value, .. } => {
                1 + value.as_ref().map_or(0, |value| depth(value, cache))
            }
            Value::Enum { values, .. } => {
                1 + values
                    .iter()
                    .map(|value| depth(value, cache))
                    .max()
                    .unwrap_or(0)
            }
            _ => 0,
        };
        if let Some(key) = key {
            cache.insert(key, depth);
        }
        depth
    }

    depth(value, &mut BTreeMap::new())
}

pub(crate) fn decode_host_value(
    json: &serde_json::Value,
    ty: ValueType,
    analysis: &Analysis,
    depth: usize,
) -> Option<Value> {
    if depth >= 32
        && matches!(
            ty,
            ValueType::Record(_)
                | ValueType::List(_)
                | ValueType::Option(_)
                | ValueType::Enum(_)
                | ValueType::Map(_)
                | ValueType::Tuple(_)
        )
    {
        return None;
    }
    let value = match ty {
        ValueType::Int => Value::Int(i32::try_from(json.as_i64()?).ok()?),
        ValueType::Float => Value::Float(match json.as_str() {
            Some("NaN") => f64::NAN,
            Some("+Infinity") => f64::INFINITY,
            Some("-Infinity") => f64::NEG_INFINITY,
            Some("+0") => 0.0,
            Some("-0") => -0.0,
            Some(_) => return None,
            None => json.as_f64()?,
        }),
        ValueType::Bool => Value::Bool(json.as_bool()?),
        ValueType::String => Value::String(json.as_str()?.to_owned()),
        ValueType::Unit if json.is_null() => Value::Unit,
        ValueType::Record(type_id) => {
            let record = analysis.records.get(type_id)?;
            let object = json.as_object()?;
            if object.len() != record.fields.len() {
                return None;
            }
            let fields = record
                .fields
                .iter()
                .map(|(name, ty)| {
                    Some((
                        name.clone(),
                        decode_host_value(object.get(name)?, *ty, analysis, depth + 1)?,
                    ))
                })
                .collect::<Option<_>>()?;
            Value::Record {
                type_id,
                fields: Rc::new(fields),
            }
        }
        ValueType::List(type_id) => {
            let TypeInfo::List(inner) = analysis.types.get(type_id)? else {
                return None;
            };
            let elements = json
                .as_array()?
                .iter()
                .map(|element| decode_host_value(element, *inner, analysis, depth + 1))
                .collect::<Option<_>>()?;
            Value::List {
                type_id,
                elements: Rc::new(elements),
            }
        }
        ValueType::Tuple(type_id) => {
            let TypeInfo::Tuple(types) = analysis.types.get(type_id)? else {
                return None;
            };
            let elements = json.as_array()?;
            if elements.len() != types.len() {
                return None;
            }
            Value::Tuple {
                type_id,
                elements: Rc::new(
                    elements
                        .iter()
                        .zip(types)
                        .map(|(element, ty)| decode_host_value(element, *ty, analysis, depth + 1))
                        .collect::<Option<_>>()?,
                ),
            }
        }
        ValueType::Option(type_id) => {
            let TypeInfo::Option(inner) = analysis.types.get(type_id)? else {
                return None;
            };
            let object = json.as_object()?;
            let value = match object.get("tag")?.as_str()? {
                "None" if object.len() == 1 => None,
                "Some" if object.len() == 2 => Some(Rc::new(decode_host_value(
                    object.get("value")?,
                    *inner,
                    analysis,
                    depth + 1,
                )?)),
                _ => return None,
            };
            Value::Option { type_id, value }
        }
        ValueType::Enum(type_id) => {
            let TypeInfo::Enum { variants, .. } = analysis.types.get(type_id)? else {
                return None;
            };
            let object = json.as_object()?;
            if object.len() != 2 {
                return None;
            }
            let tag = object.get("tag")?.as_str()?;
            let payloads = object.get("values")?.as_array()?;
            let types = variants.get(tag)?;
            if types.len() != payloads.len() {
                return None;
            }
            let values = payloads
                .iter()
                .zip(types)
                .map(|(value, ty)| decode_host_value(value, *ty, analysis, depth + 1))
                .collect::<Option<_>>()?;
            Value::Enum {
                type_id,
                tag: Rc::new(tag.to_owned()),
                values: Rc::new(values),
            }
        }
        ValueType::Map(type_id) => {
            let TypeInfo::Map { key, value } = analysis.types.get(type_id)? else {
                return None;
            };
            let entries = json
                .as_array()?
                .iter()
                .map(|entry| {
                    let entry = entry.as_array()?;
                    if entry.len() != 2 {
                        return None;
                    }
                    Some((
                        decode_host_value(&entry[0], *key, analysis, depth + 1)?,
                        decode_host_value(&entry[1], *value, analysis, depth + 1)?,
                    ))
                })
                .collect::<Option<_>>()?;
            Value::Map {
                type_id,
                entries: Rc::new(entries),
            }
        }
        _ => return None,
    };
    valid_value(&value, ty, analysis, depth, true).then_some(value)
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
    key: FunctionKey,
    type_arguments: BTreeMap<usize, ValueType>,
}

impl Interpreter<'_> {
    fn call_function(
        &mut self,
        key: &FunctionKey,
        arguments: Vec<Value>,
        callable_type: Option<usize>,
    ) -> Result<Value, EvalError> {
        self.enter_call()?;
        let function = find_function(&self.analysis.modules, key)
            .cloned()
            .ok_or_else(invariant_error)?;
        let mut signature = self
            .analysis
            .signatures
            .get(key)
            .cloned()
            .ok_or_else(invariant_error)?;
        let mut type_arguments = BTreeMap::new();
        if !signature.type_parameters.is_empty() {
            let Some(TypeInfo::Function {
                parameters,
                return_type,
            }) = callable_type.and_then(|id| self.analysis.types.get(id))
            else {
                return Err(invariant_error());
            };
            if signature.parameters.len() != parameters.len()
                || !signature
                    .parameters
                    .iter()
                    .zip(parameters)
                    .all(|(template, actual)| {
                        infer_type_arguments(
                            *template,
                            *actual,
                            &mut type_arguments,
                            &self.analysis.records,
                            &self.analysis.types,
                        )
                    })
                || !infer_type_arguments(
                    signature.return_type.ok_or_else(invariant_error)?,
                    *return_type,
                    &mut type_arguments,
                    &self.analysis.records,
                    &self.analysis.types,
                )
                || signature.type_parameters.iter().any(
                    |ty| !matches!(ty, ValueType::Parameter(id) if type_arguments.contains_key(id)),
                )
            {
                return Err(invariant_error());
            }
            signature.parameters = signature
                .parameters
                .iter()
                .map(|ty| {
                    substitute_type_existing(
                        *ty,
                        &type_arguments,
                        &self.analysis.records,
                        &self.analysis.types,
                    )
                    .ok_or_else(invariant_error)
                })
                .collect::<Result<_, _>>()?;
            signature.return_type = Some(
                substitute_type_existing(
                    signature.return_type.ok_or_else(invariant_error)?,
                    &type_arguments,
                    &self.analysis.records,
                    &self.analysis.types,
                )
                .ok_or_else(invariant_error)?,
            );
        }
        if !valid_arguments(&arguments, &signature, self.analysis, false) {
            return Err(invariant_error());
        }
        let caller_scopes = std::mem::take(&mut self.scopes);
        let caller_key = std::mem::replace(&mut self.key, key.clone());
        let caller_types = std::mem::replace(&mut self.type_arguments, type_arguments);
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
        self.key = caller_key;
        self.type_arguments = caller_types;
        self.call_depth -= 1;
        let value = completing_value(result?)?;
        if signature.return_type != Some(value.ty()) {
            return Err(invariant_error());
        }
        Ok(value)
    }

    fn call_value(&mut self, callable: Value, arguments: Vec<Value>) -> Result<Value, EvalError> {
        let Value::Function { type_id, function } = callable else {
            return Err(invariant_error());
        };
        match function.as_ref() {
            FunctionValue::Named(key) => self.call_function(key, arguments, Some(type_id)),
            FunctionValue::Closure {
                parameters,
                body,
                captures,
                key,
                type_arguments,
            } => {
                let Some(TypeInfo::Function {
                    parameters: types,
                    return_type,
                }) = self.analysis.types.get(type_id)
                else {
                    return Err(invariant_error());
                };
                if arguments.len() != types.len()
                    || !arguments
                        .iter()
                        .zip(types)
                        .all(|(value, ty)| value.ty() == *ty)
                {
                    return Err(invariant_error());
                }
                self.enter_call()?;
                let caller_scopes = std::mem::take(&mut self.scopes);
                let caller_key = std::mem::replace(&mut self.key, key.clone());
                let caller_types =
                    std::mem::replace(&mut self.type_arguments, type_arguments.clone());
                self.scopes.push(captures.clone());
                self.scopes
                    .push(parameters.iter().cloned().zip(arguments).collect());
                let result = self.eval_expr(body);
                self.scopes = caller_scopes;
                self.key = caller_key;
                self.type_arguments = caller_types;
                self.call_depth -= 1;
                let value = completing_value(result?)?;
                if value.ty() != *return_type {
                    return Err(invariant_error());
                }
                Ok(value)
            }
        }
    }

    fn enter_call(&mut self) -> Result<(), EvalError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(resource_fault());
        }
        self.step()?;
        self.call_depth += 1;
        Ok(())
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
                completion => return Ok(completion),
            }
        }
        match &block.tail {
            Some(tail) => self.eval_expr(tail),
            None => Ok(Completion::Value(Value::Unit)),
        }
    }

    fn eval_statement(&mut self, statement: &Statement) -> Result<Completion, EvalError> {
        match &statement.kind {
            StatementKind::Let {
                name,
                value: expression,
                ..
            } => {
                let value = value!(self.eval_expr(expression));
                self.scopes
                    .last_mut()
                    .ok_or_else(invariant_error)?
                    .insert(name.name.clone(), value);
            }
            StatementKind::LetPattern {
                pattern,
                value: expression,
                ..
            } => {
                let value = value!(self.eval_expr(expression));
                let mut bindings = BTreeMap::new();
                if !match_pattern(pattern, &value, &mut bindings) {
                    return Err(invariant_error());
                }
                self.scopes
                    .last_mut()
                    .ok_or_else(invariant_error)?
                    .extend(bindings);
            }
            StatementKind::Assign {
                target,
                value: expression,
            } => {
                let ExprKind::Name(name) = &target.kind else {
                    return Err(invariant_error());
                };
                let scope = self
                    .scopes
                    .iter()
                    .rposition(|scope| scope.contains_key(&name.name))
                    .ok_or_else(invariant_error)?;
                let value = value!(self.eval_expr(expression));
                self.scopes[scope].insert(name.name.clone(), value);
            }
            StatementKind::Return(expression) => {
                let value = match expression {
                    Some(expression) => value!(self.eval_expr(expression)),
                    None => Value::Unit,
                };
                return Ok(Completion::Return(value));
            }
            StatementKind::Expression(expression) => return self.eval_expr(expression),
            StatementKind::Break => return Ok(Completion::Break),
            StatementKind::Continue => return Ok(Completion::Continue),
            StatementKind::While { condition, body } => loop {
                self.step()?;
                let Value::Bool(condition) = value!(self.eval_expr(condition)) else {
                    return Err(invariant_error());
                };
                if !condition {
                    break;
                }
                match self.eval_block(body)? {
                    Completion::Value(_) | Completion::Continue => {}
                    Completion::Break => break,
                    completion => return Ok(completion),
                }
            },
            StatementKind::For {
                name,
                iterable,
                body,
            } => {
                let iterable = value!(self.eval_expr(iterable));
                match iterable {
                    Value::List { elements, .. } => {
                        for value in elements.iter().cloned() {
                            match self.iteration(&name.name, value, body)? {
                                Completion::Break => break,
                                Completion::Return(value) => return Ok(Completion::Return(value)),
                                _ => {}
                            }
                        }
                    }
                    Value::String(text) => {
                        for scalar in text.chars() {
                            match self.iteration(
                                &name.name,
                                Value::String(scalar.to_string()),
                                body,
                            )? {
                                Completion::Break => break,
                                Completion::Return(value) => return Ok(Completion::Return(value)),
                                _ => {}
                            }
                        }
                    }
                    Value::Range { start, end } => {
                        for integer in start..end {
                            match self.iteration(&name.name, Value::Int(integer), body)? {
                                Completion::Break => break,
                                Completion::Return(value) => return Ok(Completion::Return(value)),
                                _ => {}
                            }
                        }
                    }
                    _ => return Err(invariant_error()),
                }
            }
        }
        Ok(Completion::Value(Value::Unit))
    }

    fn iteration(
        &mut self,
        name: &str,
        value: Value,
        body: &Block,
    ) -> Result<Completion, EvalError> {
        self.step()?;
        self.scopes.push(BTreeMap::from([(name.to_owned(), value)]));
        let result = self.eval_block(body);
        self.scopes.pop();
        result
    }

    fn expression_type(&self, expression: &Expr) -> Result<ValueType, EvalError> {
        let ty = self
            .analysis
            .expression_types
            .get(&(self.key.clone(), expression.span.clone()))
            .copied()
            .ok_or_else(invariant_error)?;
        substitute_type_existing(
            ty,
            &self.type_arguments,
            &self.analysis.records,
            &self.analysis.types,
        )
        .ok_or_else(invariant_error)
    }

    fn local(&self, name: &str) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
    }

    fn eval_expr(&mut self, mut expression: &Expr) -> Result<Completion, EvalError> {
        self.step()?;
        // Completed fixtures need no suspension frame, but await still consumes a step.
        while let ExprKind::Await(operand) = &expression.kind {
            self.step()?;
            expression = operand;
        }
        // Keep recursively entered control expressions off the larger aggregate frame.
        match &expression.kind {
            ExprKind::Block(block) => self.eval_block(block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => match value!(self.eval_expr(condition)) {
                Value::Bool(true) => self.eval_block(then_branch),
                Value::Bool(false) => self.eval_expr(else_branch),
                _ => Err(invariant_error()),
            },
            ExprKind::Call { callee, arguments } => self.eval_call(expression, callee, arguments),
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left = value!(self.eval_expr(left));
                match (operator, &left) {
                    (Symbol::AndAnd, Value::Bool(false)) => {
                        return Ok(Completion::Value(Value::Bool(false)))
                    }
                    (Symbol::OrOr, Value::Bool(true)) => {
                        return Ok(Completion::Value(Value::Bool(true)))
                    }
                    _ => {}
                }
                let right = value!(self.eval_expr(right));
                eval_binary(*operator, left, right).map(Completion::Value)
            }
            _ => self.eval_value_expr(expression),
        }
    }

    fn eval_value_expr(&mut self, expression: &Expr) -> Result<Completion, EvalError> {
        let value = match &expression.kind {
            ExprKind::Integer { value, .. } => {
                Value::Int(value.parse().map_err(|_| invariant_error())?)
            }
            ExprKind::Float { value, .. } => {
                Value::Float(value.parse().map_err(|_| invariant_error())?)
            }
            ExprKind::String(value) => Value::String(value.clone()),
            ExprKind::Bool(value) => Value::Bool(*value),
            ExprKind::Unit => Value::Unit,
            ExprKind::Name(name) => {
                if let Some(value) = self.local(&name.name) {
                    value
                } else {
                    let key = self
                        .analysis
                        .module_symbols
                        .get(&expression.span.source_id)
                        .and_then(|symbols| symbols.get(&name.name))
                        .cloned()
                        .ok_or_else(invariant_error)?;
                    let ValueType::Function(type_id) = self.expression_type(expression)? else {
                        return Err(invariant_error());
                    };
                    Value::Function {
                        type_id,
                        function: Rc::new(FunctionValue::Named(key)),
                    }
                }
            }
            ExprKind::Await(_) => unreachable!("completed await operands are unwrapped above"),
            ExprKind::Unary { operator, operand } => {
                if *operator == Symbol::Minus {
                    if let ExprKind::Integer {
                        value,
                        literal_span,
                    } = &operand.kind
                    {
                        if operand.span == *literal_span && value == "2147483648" {
                            self.step()?;
                            return Ok(Completion::Value(Value::Int(i32::MIN)));
                        }
                    }
                }
                let value = value!(self.eval_expr(operand));
                eval_unary(*operator, value)?
            }
            ExprKind::Binary { .. }
            | ExprKind::Call { .. }
            | ExprKind::Block(_)
            | ExprKind::If { .. } => {
                unreachable!("control expressions are evaluated in the smaller dispatch frame")
            }
            ExprKind::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(value!(self.eval_expr(element)));
                }
                self.list(self.expression_type(expression)?, values)?
            }
            ExprKind::Tuple(elements) if elements.is_empty() => Value::Unit,
            ExprKind::Tuple(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(value!(self.eval_expr(element)));
                }
                let ValueType::Tuple(type_id) = self.expression_type(expression)? else {
                    return Err(invariant_error());
                };
                let Some(TypeInfo::Tuple(types)) = self.analysis.types.get(type_id) else {
                    return Err(invariant_error());
                };
                if types.len() != values.len() {
                    return Err(invariant_error());
                }
                bounded(Value::Tuple {
                    type_id,
                    elements: Rc::new(values),
                })?
            }
            ExprKind::Arrow { parameters, body } => {
                let ValueType::Function(type_id) = self.expression_type(expression)? else {
                    return Err(invariant_error());
                };
                let captures = self
                    .scopes
                    .iter()
                    .flat_map(|scope| {
                        scope
                            .iter()
                            .filter(|(_, value)| !matches!(value, Value::Capability(_)))
                            .map(|(name, value)| (name.clone(), value.clone()))
                    })
                    .collect();
                Value::Function {
                    type_id,
                    function: Rc::new(FunctionValue::Closure {
                        parameters: parameters
                            .iter()
                            .map(|parameter| parameter.name.name.clone())
                            .collect(),
                        body: body.clone(),
                        captures,
                        key: self.key.clone(),
                        type_arguments: self.type_arguments.clone(),
                    }),
                }
            }
            ExprKind::Match { subject, arms } => {
                let subject = value!(self.eval_expr(subject));
                for arm in arms {
                    let mut bindings = BTreeMap::new();
                    if !match_pattern(&arm.pattern, &subject, &mut bindings) {
                        continue;
                    }
                    self.scopes.push(bindings);
                    let result = (|| {
                        if let Some(guard) = &arm.guard {
                            match self.eval_expr(guard)? {
                                Completion::Value(Value::Bool(false)) => return Ok(None),
                                Completion::Value(Value::Bool(true)) => {}
                                Completion::Value(_) => return Err(invariant_error()),
                                completion => return Ok(Some(completion)),
                            }
                        }
                        self.eval_expr(&arm.body).map(Some)
                    })();
                    self.scopes.pop();
                    if let Some(result) = result? {
                        return Ok(result);
                    }
                }
                return Err(invariant_error());
            }
            ExprKind::Record { base, fields, .. } => {
                let mut values = if let Some(base) = base {
                    let Value::Record { fields, .. } = value!(self.eval_expr(base)) else {
                        return Err(invariant_error());
                    };
                    fields.as_ref().clone()
                } else {
                    BTreeMap::new()
                };
                for (name, expression) in fields {
                    values.insert(name.name.clone(), value!(self.eval_expr(expression)));
                }
                let ValueType::Record(type_id) = self.expression_type(expression)? else {
                    return Err(invariant_error());
                };
                bounded(Value::Record {
                    type_id,
                    fields: Rc::new(values),
                })?
            }
            ExprKind::Member { object, name } => {
                if let Some((_, tag)) = self.enum_constructor(expression) {
                    let ValueType::Enum(type_id) = self.expression_type(expression)? else {
                        return Err(invariant_error());
                    };
                    bounded(Value::Enum {
                        type_id,
                        tag: Rc::new(tag),
                        values: Rc::new(Vec::new()),
                    })?
                } else if matches!(&object.kind, ExprKind::Name(namespace) if namespace.name == "Option")
                    && name.name == "None"
                {
                    self.option(self.expression_type(expression)?, None)?
                } else {
                    let Value::Record { fields, .. } = value!(self.eval_expr(object)) else {
                        return Err(invariant_error());
                    };
                    fields
                        .get(&name.name)
                        .cloned()
                        .ok_or_else(invariant_error)?
                }
            }
            ExprKind::Index { object, index } => {
                let object = value!(self.eval_expr(object));
                let Value::Int(index) = value!(self.eval_expr(index)) else {
                    return Err(invariant_error());
                };
                let value = match object {
                    Value::List { elements, .. } => usize::try_from(index)
                        .ok()
                        .and_then(|index| elements.get(index).cloned()),
                    Value::String(text) => usize::try_from(index)
                        .ok()
                        .and_then(|index| text.chars().nth(index))
                        .map(|scalar| Value::String(scalar.to_string())),
                    _ => return Err(invariant_error()),
                };
                self.option(self.expression_type(expression)?, value)?
            }
            ExprKind::Propagate(operand) => {
                let Value::Enum { tag, values, .. } = value!(self.eval_expr(operand)) else {
                    return Err(invariant_error());
                };
                let [payload] = values.as_slice() else {
                    return Err(invariant_error());
                };
                match tag.as_str() {
                    "Ok" => payload.clone(),
                    "Err" => {
                        let return_type = self
                            .analysis
                            .signatures
                            .get(&self.key)
                            .and_then(|signature| signature.return_type)
                            .ok_or_else(invariant_error)?;
                        let Some(ValueType::Enum(type_id)) = substitute_type_existing(
                            return_type,
                            &self.type_arguments,
                            &self.analysis.records,
                            &self.analysis.types,
                        ) else {
                            return Err(invariant_error());
                        };
                        return Ok(Completion::Return(bounded(Value::Enum {
                            type_id,
                            tag,
                            values,
                        })?));
                    }
                    _ => return Err(invariant_error()),
                }
            }
        };
        Ok(Completion::Value(value))
    }

    fn eval_call(
        &mut self,
        expression: &Expr,
        callee: &Expr,
        arguments: &[Expr],
    ) -> Result<Completion, EvalError> {
        let enum_constructor = self.enum_constructor(callee);
        let constructor = matches!(&callee.kind, ExprKind::Member { object, name } if matches!(&object.kind, ExprKind::Name(namespace) if namespace.name == "Option") && name.name == "Some");
        let builtin = match &callee.kind {
            ExprKind::Name(name)
                if self.local(&name.name).is_none() && builtin_name(&name.name) =>
            {
                Some(name.name.as_str())
            }
            _ => None,
        };
        let callable = if constructor || builtin.is_some() || enum_constructor.is_some() {
            None
        } else {
            Some(value!(self.eval_expr(callee)))
        };
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            values.push(value!(self.eval_expr(argument)));
        }
        let result = if let Some((_, tag)) = enum_constructor {
            let ValueType::Enum(type_id) = self.expression_type(expression)? else {
                return Err(invariant_error());
            };
            bounded(Value::Enum {
                type_id,
                tag: Rc::new(tag),
                values: Rc::new(values),
            })?
        } else if constructor {
            self.option(
                self.expression_type(expression)?,
                Some(values.into_iter().next().ok_or_else(invariant_error)?),
            )?
        } else if let Some(name) = builtin {
            self.builtin(name, values, self.expression_type(expression)?)?
        } else {
            self.call_value(callable.ok_or_else(invariant_error)?, values)?
        };
        Ok(Completion::Value(result))
    }

    fn list(&self, ty: ValueType, elements: Vec<Value>) -> Result<Value, EvalError> {
        let ValueType::List(type_id) = ty else {
            return Err(invariant_error());
        };
        bounded(Value::List {
            type_id,
            elements: Rc::new(elements),
        })
    }

    fn enum_constructor(&self, expression: &Expr) -> Option<(usize, String)> {
        let ExprKind::Member { object, name } = &expression.kind else {
            return None;
        };
        let ExprKind::Name(qualifier) = &object.kind else {
            return None;
        };
        if self.local(&qualifier.name).is_some() {
            return None;
        }
        let key = if matches!(qualifier.name.as_str(), "Result" | "HostError") {
            (String::new(), qualifier.name.clone())
        } else {
            self.analysis
                .module_symbols
                .get(&self.key.0)?
                .get(&qualifier.name)?
                .clone()
        };
        let id = crate::analyzer::enum_type(&self.analysis.types, &key)?;
        Some((id, name.name.clone()))
    }

    fn option(&self, ty: ValueType, value: Option<Value>) -> Result<Value, EvalError> {
        let ValueType::Option(type_id) = ty else {
            return Err(invariant_error());
        };
        bounded(Value::Option {
            type_id,
            value: value.map(Rc::new),
        })
    }

    fn map(&self, ty: ValueType, entries: Vec<(Value, Value)>) -> Result<Value, EvalError> {
        let ValueType::Map(type_id) = ty else {
            return Err(invariant_error());
        };
        bounded(Value::Map {
            type_id,
            entries: Rc::new(entries),
        })
    }

    fn step(&mut self) -> Result<(), EvalError> {
        if self.remaining_steps == 0 {
            return Err(resource_fault());
        }
        self.remaining_steps -= 1;
        Ok(())
    }

    fn host_operation(
        &self,
        name: &str,
        capability: &HostCapability,
        arguments: &[Value],
        ty: ValueType,
    ) -> Result<Value, EvalError> {
        let expected_kind = if name == "consoleLog" {
            CapabilityKind::Console
        } else if name == "httpGet" {
            CapabilityKind::Http
        } else {
            CapabilityKind::Storage
        };
        if capability.kind != expected_kind {
            return Err(invariant_error());
        }
        let arguments = arguments
            .iter()
            .map(|value| match value {
                Value::String(text) => Ok(text.clone()),
                _ => Err(invariant_error()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (capability.callback)(name, &arguments)
        }))
        .ok()
        .and_then(Result::ok)
        .ok_or_else(|| {
            EvalError::Runtime(RuntimeFault {
                code: "UBI-R0004",
                message: "Unexpected trusted-adapter fault".to_owned(),
            })
        })?;
        if let Some(value) = serde_json::from_str(&response)
            .ok()
            .and_then(|json| decode_host_value(&json, ty, self.analysis, 0))
        {
            return Ok(value);
        }
        let ValueType::Enum(type_id) = ty else {
            return Err(invariant_error());
        };
        let Some(TypeInfo::Enum { variants, .. }) = self.analysis.types.get(type_id) else {
            return Err(invariant_error());
        };
        let Some([ValueType::Enum(error_id)]) = variants.get("Err").map(Vec::as_slice) else {
            return Err(invariant_error());
        };
        bounded(Value::Enum {
            type_id,
            tag: Rc::new("Err".to_owned()),
            values: Rc::new(vec![Value::Enum {
                type_id: *error_id,
                tag: Rc::new("Decode".to_owned()),
                values: Rc::new(vec![Value::String("Malformed host response".to_owned())]),
            }]),
        })
    }

    fn builtin(
        &mut self,
        name: &str,
        arguments: Vec<Value>,
        ty: ValueType,
    ) -> Result<Value, EvalError> {
        match (name, arguments.as_slice()) {
            (
                "consoleLog" | "storageGet" | "storageSet" | "storageRemove" | "storageGetAsync"
                | "storageSetAsync" | "storageRemoveAsync" | "httpGet",
                [Value::Capability(capability), arguments @ ..],
            ) => self.host_operation(name, capability, arguments, ty),
            ("range", [Value::Int(start), Value::Int(end)]) => Ok(Value::Range {
                start: *start,
                end: *end,
            }),
            ("length", [Value::String(text)]) => checked_length(text.chars().count()),
            ("length", [Value::List { elements, .. }]) => checked_length(elements.len()),
            ("length", [Value::Map { entries, .. }]) => checked_length(entries.len()),
            ("mapEmpty", []) => self.map(ty, Vec::new()),
            ("mapSet", [Value::Map { entries, .. }, key, value]) => {
                let mut result = entries.as_ref().clone();
                if let Some((_, old_value)) =
                    result.iter_mut().find(|(entry_key, _)| entry_key == key)
                {
                    *old_value = value.clone();
                } else {
                    result.push((key.clone(), value.clone()));
                }
                self.map(ty, result)
            }
            ("mapGet", [Value::Map { entries, .. }, key]) => self.option(
                ty,
                entries
                    .iter()
                    .find(|(entry_key, _)| entry_key == key)
                    .map(|(_, value)| value.clone()),
            ),
            ("mapHas", [Value::Map { entries, .. }, key]) => Ok(Value::Bool(
                entries.iter().any(|(entry_key, _)| entry_key == key),
            )),
            ("mapRemove", [Value::Map { entries, .. }, key]) => self.map(
                ty,
                entries
                    .iter()
                    .filter(|(entry_key, _)| entry_key != key)
                    .cloned()
                    .collect(),
            ),
            ("mapKeys", [Value::Map { entries, .. }]) => {
                self.list(ty, entries.iter().map(|(key, _)| key.clone()).collect())
            }
            ("mapValues", [Value::Map { entries, .. }]) => {
                self.list(ty, entries.iter().map(|(_, value)| value.clone()).collect())
            }
            ("append", [Value::List { elements, .. }, value]) => {
                let mut result = elements.as_ref().clone();
                result.push(value.clone());
                self.list(ty, result)
            }
            ("set", [list @ Value::List { elements, .. }, Value::Int(index), value]) => {
                let result = usize::try_from(*index)
                    .ok()
                    .filter(|index| *index < elements.len())
                    .map(|index| {
                        let mut result = elements.as_ref().clone();
                        result[index] = value.clone();
                        self.list(list.ty(), result)
                    })
                    .transpose()?;
                self.option(ty, result)
            }
            ("map", [Value::List { elements, .. }, callback]) => {
                let mut result = Vec::with_capacity(elements.len());
                for value in elements.iter() {
                    self.step()?;
                    result.push(self.call_value(callback.clone(), vec![value.clone()])?);
                }
                self.list(ty, result)
            }
            ("filter", [Value::List { elements, .. }, callback]) => {
                let mut result = Vec::new();
                for value in elements.iter() {
                    self.step()?;
                    let Value::Bool(include) =
                        self.call_value(callback.clone(), vec![value.clone()])?
                    else {
                        return Err(invariant_error());
                    };
                    if include {
                        result.push(value.clone());
                    }
                }
                self.list(ty, result)
            }
            ("find", [Value::List { elements, .. }, callback]) => {
                for value in elements.iter() {
                    self.step()?;
                    let Value::Bool(found) =
                        self.call_value(callback.clone(), vec![value.clone()])?
                    else {
                        return Err(invariant_error());
                    };
                    if found {
                        return self.option(ty, Some(value.clone()));
                    }
                }
                self.option(ty, None)
            }
            ("fold", [Value::List { elements, .. }, initial, callback]) => {
                let mut accumulator = initial.clone();
                for value in elements.iter() {
                    self.step()?;
                    accumulator =
                        self.call_value(callback.clone(), vec![accumulator, value.clone()])?;
                }
                Ok(accumulator)
            }
            ("contains", [Value::String(text), Value::String(search)]) => {
                Ok(Value::Bool(text.contains(search)))
            }
            ("startsWith", [Value::String(text), Value::String(search)]) => {
                Ok(Value::Bool(text.starts_with(search)))
            }
            ("endsWith", [Value::String(text), Value::String(search)]) => {
                Ok(Value::Bool(text.ends_with(search)))
            }
            ("trim", [Value::String(text)]) => Ok(Value::String(
                text.trim_matches([' ', '\t', '\r', '\n']).to_owned(),
            )),
            ("split", [Value::String(text), Value::String(separator)]) => {
                let result = if separator.is_empty() {
                    text.chars()
                        .map(|scalar| Value::String(scalar.to_string()))
                        .collect()
                } else {
                    text.split(separator)
                        .map(|field| Value::String(field.to_owned()))
                        .collect()
                };
                self.list(ty, result)
            }
            ("join", [Value::List { elements, .. }, Value::String(separator)]) => {
                let fields: Result<Vec<&str>, _> = elements
                    .iter()
                    .map(|value| match value {
                        Value::String(text) => Ok(text.as_str()),
                        _ => Err(invariant_error()),
                    })
                    .collect();
                Ok(Value::String(fields?.join(separator)))
            }
            (
                "replace",
                [Value::String(text), Value::String(search), Value::String(replacement)],
            ) => Ok(Value::String(text.replace(search, replacement))),
            ("slice", [Value::String(text), Value::Int(start), Value::Int(end)]) => {
                let start = (*start).max(0) as usize;
                let end = (*end).max(0) as usize;
                Ok(Value::String(
                    text.chars()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect(),
                ))
            }
            ("abs", [Value::Int(value)]) => value
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(overflow_fault),
            ("abs", [Value::Float(value)]) => Ok(Value::Float(value.abs())),
            ("min", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).min(*b))),
            ("max", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).max(*b))),
            ("min", [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(float_min(*a, *b))),
            ("max", [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(float_max(*a, *b))),
            ("clamp", [Value::Int(value), Value::Int(low), Value::Int(high)]) => {
                if low > high {
                    Err(invalid_clamp_fault())
                } else {
                    Ok(Value::Int((*value).max(*low).min(*high)))
                }
            }
            ("clamp", [Value::Float(value), Value::Float(low), Value::Float(high)]) => {
                if low > high {
                    Err(invalid_clamp_fault())
                } else {
                    Ok(Value::Float(float_min(float_max(*value, *low), *high)))
                }
            }
            ("floor", [Value::Float(value)]) => Ok(Value::Float(value.floor())),
            ("ceil", [Value::Float(value)]) => Ok(Value::Float(value.ceil())),
            ("round", [Value::Float(value)]) => Ok(Value::Float(value.round())),
            ("sqrt", [Value::Float(value)]) => Ok(Value::Float(value.sqrt())),
            ("sin", [Value::Float(value)]) => Ok(Value::Float(value.sin())),
            ("cos", [Value::Float(value)]) => Ok(Value::Float(value.cos())),
            ("tan", [Value::Float(value)]) => Ok(Value::Float(value.tan())),
            ("log", [Value::Float(value)]) => Ok(Value::Float(value.ln())),
            ("exp", [Value::Float(value)]) => Ok(Value::Float(value.exp())),
            ("pow", [Value::Float(base), Value::Float(exponent)]) => {
                let result = if *exponent == 0.0 {
                    1.0
                } else if exponent.is_nan()
                    || base.is_nan()
                    || (base.abs() == 1.0 && exponent.is_infinite())
                {
                    f64::NAN
                } else {
                    base.powf(*exponent)
                };
                Ok(Value::Float(result))
            }
            ("toFloat", [Value::Int(value)]) => Ok(Value::Float(f64::from(*value))),
            ("toInt", [Value::Float(value)]) => {
                let value = value.trunc();
                let result = (value.is_finite()
                    && value >= f64::from(i32::MIN)
                    && value <= f64::from(i32::MAX))
                .then_some(Value::Int(value as i32));
                self.option(ty, result)
            }
            ("parseInt", [Value::String(text)]) => {
                let bytes = text.as_bytes();
                let digits = if bytes
                    .first()
                    .is_some_and(|byte| matches!(byte, b'+' | b'-'))
                {
                    &bytes[1..]
                } else {
                    bytes
                };
                let result = if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) {
                    text.parse::<i32>().ok().map(Value::Int)
                } else {
                    None
                };
                self.option(ty, result)
            }
            ("parseFloat", [Value::String(text)]) => {
                let result = if decimal_float(text) {
                    text.parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .map(Value::Float)
                } else {
                    None
                };
                self.option(ty, result)
            }
            ("toString", [Value::Int(value)]) => Ok(Value::String(value.to_string())),
            ("toString", [Value::Bool(value)]) => Ok(Value::String(value.to_string())),
            ("toString", [Value::String(value)]) => Ok(Value::String(value.clone())),
            _ => Err(invariant_error()),
        }
    }
}

fn checked_length(length: usize) -> Result<Value, EvalError> {
    i32::try_from(length).map(Value::Int).map_err(|_| {
        EvalError::Runtime(RuntimeFault {
            code: "UBI-R0003",
            message: "Length exceeds int range".into(),
        })
    })
}

fn float_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        a.min(b)
    }
}

fn float_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        a.max(b)
    }
}

fn invalid_clamp_fault() -> EvalError {
    EvalError::Runtime(RuntimeFault {
        code: "UBI-R0003",
        message: "Clamp lower bound exceeds upper bound".to_owned(),
    })
}

fn decimal_float(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = usize::from(
        bytes
            .first()
            .is_some_and(|byte| matches!(byte, b'+' | b'-')),
    );
    let start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if start == index {
        return false;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if start == index {
            return false;
        }
    }
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        index += 1;
        if bytes
            .get(index)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            index += 1;
        }
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if start == index {
            return false;
        }
    }
    index == bytes.len()
}

fn bounded(value: Value) -> Result<Value, EvalError> {
    if aggregate_depth(&value) > 32 {
        Err(resource_fault())
    } else {
        Ok(value)
    }
}

fn completing_value(completion: Completion) -> Result<Value, EvalError> {
    match completion {
        Completion::Value(value) | Completion::Return(value) => Ok(value),
        _ => Err(invariant_error()),
    }
}

fn match_pattern(pattern: &Pattern, value: &Value, bindings: &mut BTreeMap<String, Value>) -> bool {
    match (&pattern.kind, value) {
        (PatternKind::Wildcard, _) => true,
        (PatternKind::Binding(name), _) => {
            bindings.insert(name.name.clone(), value.clone());
            true
        }
        (PatternKind::Tuple(patterns), Value::Unit) if patterns.is_empty() => true,
        (PatternKind::Tuple(patterns), Value::Tuple { elements, .. }) => {
            patterns.len() == elements.len()
                && patterns
                    .iter()
                    .zip(elements.iter())
                    .all(|(pattern, value)| match_pattern(pattern, value, bindings))
        }
        (PatternKind::Integer(integer), Value::Int(value)) => {
            integer.parse::<i32>().ok() == Some(*value)
        }
        (PatternKind::String(text), Value::String(value)) => text == value,
        (PatternKind::Bool(boolean), Value::Bool(value)) => boolean == value,
        (
            PatternKind::Some(pattern),
            Value::Option {
                value: Some(value), ..
            },
        ) => match_pattern(pattern, value, bindings),
        (PatternKind::None, Value::Option { value: None, .. }) => true,
        (PatternKind::Variant { name, payloads, .. }, Value::Enum { tag, values, .. }) => {
            name.name == **tag
                && payloads.len() == values.len()
                && payloads
                    .iter()
                    .zip(values.iter())
                    .all(|(pattern, value)| match_pattern(pattern, value, bindings))
        }
        _ => false,
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
