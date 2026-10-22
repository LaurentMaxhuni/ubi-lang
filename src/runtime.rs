//! Native ABI: checked exports and owned JSON values, with no implicit IO authority.
use crate::analyzer::Analysis;
use crate::interpreter::{decode_host_value, invoke, InvocationError, Value};
use serde_json::{json, Value as Json};

/// An immutable checked program. Calls own their evaluation scopes and budgets.
pub struct Runtime {
    analysis: Analysis,
}

/// A rejected call or a bounded Ubi runtime fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError(pub String);

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RuntimeError {}

impl Runtime {
    pub(crate) fn new(analysis: Analysis) -> Self {
        Self { analysis }
    }

    /// Invoke a concrete, exported, synchronous, pure function.
    /// Aggregates use the existing host JSON shapes, supplied as owned JSON
    /// values rather than the JavaScript adapter's serialized JSON strings.
    /// Each call has the interpreter's one-million-step/32-frame limits;
    /// boundary data is limited to 100,000 values and 4 MiB of string bytes.
    pub fn invoke(
        &self,
        module: &str,
        export: &str,
        arguments: &[Json],
    ) -> Result<Json, RuntimeError> {
        let key = (module.to_owned(), export.to_owned());
        let signature = self
            .analysis
            .signatures
            .get(&key)
            .ok_or_else(|| RuntimeError(format!("unknown export: {module}::{export}")))?;
        let function = self
            .analysis
            .modules
            .get(module)
            .and_then(|m| {
                m.declarations.iter().find_map(|d| match d {
                    crate::parser::Declaration::Function(f) if f.name.name == export => Some(f),
                    _ => None,
                })
            })
            .ok_or_else(|| RuntimeError("unknown export".into()))?;
        if !function.exported
            || signature.asynchronous
            || !function.effects.is_empty()
            || !signature.type_parameters.is_empty()
            || arguments.len() != signature.parameters.len()
        {
            return Err(RuntimeError("native calls require a concrete exported pure synchronous function and exact argument count".into()));
        }
        let mut budget = BoundaryBudget::default();
        for argument in arguments {
            budget.check(argument, 0)?;
        }
        let arguments = arguments
            .iter()
            .zip(&signature.parameters)
            .map(|(json, ty)| {
                decode_host_value(json, *ty, &self.analysis, 0)
                    .ok_or_else(|| RuntimeError("invalid native argument type or shape".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = invoke(&self.analysis, &key, arguments).map_err(|error| {
            RuntimeError(match error {
                InvocationError::Runtime(fault) => format!("{}: {}", fault.code, fault.message),
                other => format!("native invocation rejected: {other:?}"),
            })
        })?;
        let result = encode(&result, &mut BoundaryBudget::default(), 0)?;
        Ok(result)
    }
}

#[derive(Default)]
struct BoundaryBudget {
    values: usize,
    bytes: usize,
}
impl BoundaryBudget {
    fn consume(&mut self, bytes: usize, depth: usize) -> Result<(), RuntimeError> {
        self.values += 1;
        self.bytes = self.bytes.saturating_add(bytes);
        if depth > 64 || self.values > 100_000 || self.bytes > 4 * 1024 * 1024 {
            Err(RuntimeError(
                "UBI-R0003: native boundary resource limit".into(),
            ))
        } else {
            Ok(())
        }
    }
    fn check(&mut self, value: &Json, depth: usize) -> Result<(), RuntimeError> {
        self.consume(value.as_str().map_or(0, str::len), depth)?;
        match value {
            Json::Array(items) => {
                for item in items {
                    self.check(item, depth + 1)?;
                }
            }
            Json::Object(fields) => {
                for (key, value) in fields {
                    self.bytes = self.bytes.saturating_add(key.len());
                    self.check(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn encode(value: &Value, budget: &mut BoundaryBudget, depth: usize) -> Result<Json, RuntimeError> {
    budget.consume(
        match value {
            Value::String(s) => s.len(),
            _ => 0,
        },
        depth,
    )?;
    Ok(match value {
        Value::Int(n) => json!(n),
        Value::Float(n) if n.is_nan() => json!("NaN"),
        Value::Float(n) if *n == f64::INFINITY => json!("+Infinity"),
        Value::Float(n) if *n == f64::NEG_INFINITY => json!("-Infinity"),
        Value::Float(n) if *n == 0.0 && n.is_sign_negative() => json!("-0"),
        Value::Float(n) => json!(n),
        Value::Bool(b) => json!(b),
        Value::String(s) => json!(s),
        Value::Unit => Json::Null,
        Value::Record { fields, .. } => Json::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    budget.bytes = budget.bytes.saturating_add(key.len());
                    Ok((key.clone(), encode(value, budget, depth + 1)?))
                })
                .collect::<Result<_, RuntimeError>>()?,
        ),
        Value::List { elements, .. } | Value::Tuple { elements, .. } => Json::Array(
            elements
                .iter()
                .map(|v| encode(v, budget, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Value::Enum { tag, values, .. } => json!({"tag": tag.as_str(), "values": values.iter()
            .map(|v| encode(v, budget, depth + 1)).collect::<Result<Vec<_>, _>>()?}),
        Value::Option { value: None, .. } => json!({"tag":"None"}),
        Value::Option { value: Some(v), .. } => {
            json!({"tag":"Some", "value":encode(v, budget, depth + 1)?})
        }
        Value::Map { entries, .. } => Json::Array(
            entries
                .iter()
                .map(|(k, v)| {
                    Ok(json!([
                        encode(k, budget, depth + 1)?,
                        encode(v, budget, depth + 1)?
                    ]))
                })
                .collect::<Result<_, RuntimeError>>()?,
        ),
        _ => {
            return Err(RuntimeError(
                "native exports cannot return functions, ranges or capabilities".into(),
            ))
        }
    })
}
