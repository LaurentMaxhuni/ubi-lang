use std::collections::BTreeMap;

use crate::analyzer::{FunctionKey, ValueType};
use crate::diagnostics::Diagnostic;
use crate::ir::{BlockIr, ExprIr, ExprKind, FunctionIr, ModuleIr, Program, StatementKind};
use crate::lexer::Symbol;
use crate::span::Span;

pub(crate) fn emit(program: &Program) -> Result<BTreeMap<String, String>, Diagnostic> {
    let mut files = BTreeMap::new();
    for (module_id, module) in &program.modules {
        let names = ModuleNames::new(module_id, module, program)?;
        files.insert(
            js_file_id(module_id),
            emit_module(module_id, module, &names, program)?,
        );
    }
    Ok(files)
}

fn emit_module(
    module_id: &str,
    module: &ModuleIr,
    names: &ModuleNames,
    program: &Program,
) -> Result<String, Diagnostic> {
    let mut output = String::new();
    for (local_name, target) in &module.imports {
        let binding = names
            .imports
            .get(local_name)
            .ok_or_else(|| invariant(module_id, 0))?;
        let specifier = relative_specifier(module_id, &target.0);
        output.push_str(&format!(
            "import {{ {} as {} }} from {};\n",
            target.1,
            binding,
            js_string(&specifier)
        ));
    }
    if !module.imports.is_empty() {
        output.push('\n');
    }

    output.push_str(RUNTIME);
    for key in &module.functions {
        let function = program
            .functions
            .get(key)
            .ok_or_else(|| invariant(module_id, 0))?;
        output.push('\n');
        output.push_str(&FunctionEmitter::new(names).emit(function)?);
    }

    for (key, binding) in &names.exports {
        let function = program
            .functions
            .get(key)
            .ok_or_else(|| invariant(module_id, 0))?;
        output.push('\n');
        output.push_str(&emit_export_wrapper(
            function,
            names
                .functions
                .get(key)
                .ok_or_else(|| invariant(module_id, 0))?,
            binding,
        ));
    }
    if !names.exports.is_empty() {
        output.push('\n');
        output.push_str("export { ");
        for (index, (key, binding)) in names.exports.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str(binding);
            output.push_str(" as ");
            output.push_str(&key.1);
        }
        output.push_str(" };\n");
    }
    Ok(output)
}

struct ModuleNames {
    functions: BTreeMap<FunctionKey, String>,
    imports: BTreeMap<String, String>,
    imported_targets: BTreeMap<FunctionKey, String>,
    exports: Vec<(FunctionKey, String)>,
}

impl ModuleNames {
    fn new(module_id: &str, module: &ModuleIr, program: &Program) -> Result<Self, Diagnostic> {
        let mut functions = BTreeMap::new();
        let mut exports = Vec::new();
        for (index, key) in module.functions.iter().enumerate() {
            let function = program
                .functions
                .get(key)
                .ok_or_else(|| invariant(module_id, 0))?;
            functions.insert(key.clone(), format!("$ubi_fn{index}"));
            if function.exported {
                let index = exports.len();
                exports.push((key.clone(), format!("$ubi_export{index}")));
            }
        }

        let mut imports = BTreeMap::new();
        let mut imported_targets = BTreeMap::new();
        for (index, (local_name, target)) in module.imports.iter().enumerate() {
            let binding = format!("$ubi_import{index}");
            imports.insert(local_name.clone(), binding.clone());
            imported_targets.insert(target.clone(), binding);
        }

        Ok(Self {
            functions,
            imports,
            imported_targets,
            exports,
        })
    }

    fn call_target(&self, target: &FunctionKey) -> Option<&str> {
        self.imported_targets
            .get(target)
            .or_else(|| self.functions.get(target))
            .map(String::as_str)
    }
}

struct FunctionEmitter<'a> {
    names: &'a ModuleNames,
    local_index: usize,
    scopes: Vec<BTreeMap<String, String>>,
}

impl<'a> FunctionEmitter<'a> {
    fn new(names: &'a ModuleNames) -> Self {
        Self {
            names,
            local_index: 0,
            scopes: Vec::new(),
        }
    }

    fn emit(&mut self, function: &FunctionIr) -> Result<String, Diagnostic> {
        let mut parameter_scope = BTreeMap::new();
        let mut parameters = Vec::new();
        for parameter in &function.parameters {
            let local = self.new_local();
            parameter_scope.insert(parameter.name.clone(), local.clone());
            parameters.push(local);
        }
        parameters.push("$ubi_budget".to_owned());
        self.scopes.push(parameter_scope);
        let body = self.block_contents(&function.body, 2);
        self.scopes.pop();
        let body = body?;

        let function_name = self
            .names
            .functions
            .get(&function.key)
            .ok_or_else(|| invariant_span(&function.span))?;
        let mut output = format!(
            "function {function_name}({}) {{\n  $ubi_enter($ubi_budget);\n  try {{\n",
            parameters.join(", ")
        );
        output.push_str(&body);
        output.push_str("\n  } catch ($ubi_error) {\n");
        output.push_str(
            "    if ($ubi_error && $ubi_error[$ubi_return] === true) return $ubi_error.value;\n",
        );
        output.push_str("    throw $ubi_error;\n");
        output.push_str("  } finally {\n    $ubi_leave($ubi_budget);\n  }\n}\n");
        Ok(output)
    }

    fn block_contents(&mut self, block: &BlockIr, indent: usize) -> Result<String, Diagnostic> {
        self.scopes.push(BTreeMap::new());
        let result = self.block_contents_in_scope(block, indent);
        self.scopes.pop();
        result
    }

    fn block_contents_in_scope(
        &mut self,
        block: &BlockIr,
        indent: usize,
    ) -> Result<String, Diagnostic> {
        let prefix = spaces(indent);
        let mut lines = Vec::new();
        for statement in &block.statements {
            lines.push(format!("{prefix}$ubi_tick($ubi_budget);"));
            match &statement.kind {
                StatementKind::Let { name, value } => {
                    let value = self.expression(value, indent)?;
                    let local = self.new_local();
                    lines.push(format!("{prefix}const {local} = {value};"));
                    self.scopes
                        .last_mut()
                        .ok_or_else(|| invariant_span(&statement.span))?
                        .insert(name.clone(), local);
                }
                StatementKind::Return(value) => {
                    let value = value
                        .as_ref()
                        .map(|value| self.expression(value, indent))
                        .transpose()?
                        .unwrap_or_else(|| "undefined".to_owned());
                    lines.push(format!(
                        "{prefix}throw {{ [$ubi_return]: true, value: {value} }};"
                    ));
                }
                StatementKind::Expression(expression) => {
                    let expression = self.expression(expression, indent)?;
                    lines.push(format!("{prefix}void ({expression});"));
                }
            }
        }
        let tail = block
            .tail
            .as_ref()
            .map(|expression| self.expression(expression, indent))
            .transpose()?
            .unwrap_or_else(|| "undefined".to_owned());
        lines.push(format!("{prefix}return {tail};"));
        Ok(lines.join("\n"))
    }

    fn expression(&mut self, expression: &ExprIr, indent: usize) -> Result<String, Diagnostic> {
        let value = match &expression.kind {
            ExprKind::Integer(value) => value.to_string(),
            ExprKind::Float(value) => value.to_string(),
            ExprKind::String(value) => js_string(value),
            ExprKind::Bool(value) => value.to_string(),
            ExprKind::Unit => "undefined".to_owned(),
            ExprKind::Local(name) => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .cloned()
                .ok_or_else(|| invariant_span(&expression.span))?,
            ExprKind::Call { target, arguments } => {
                let target = self
                    .names
                    .call_target(target)
                    .ok_or_else(|| invariant_span(&expression.span))?;
                let mut values = arguments
                    .iter()
                    .map(|argument| self.expression(argument, indent))
                    .collect::<Result<Vec<_>, _>>()?;
                values.push("$ubi_budget".to_owned());
                format!("{target}({})", values.join(", "))
            }
            ExprKind::Unary { operator, operand } => {
                let operand = self.expression(operand, indent)?;
                match operator {
                    Symbol::Minus if expression.ty == ValueType::Int => {
                        format!("$ubi_neg({operand})")
                    }
                    Symbol::Minus => format!("(-{operand})"),
                    Symbol::Bang => format!("(!{operand})"),
                    _ => return Err(invariant_span(&expression.span)),
                }
            }
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.expression(left, indent)?;
                let right = self.expression(right, indent)?;
                if expression.ty == ValueType::Int {
                    match operator {
                        Symbol::Plus => format!("$ubi_add({left}, {right})"),
                        Symbol::Minus => format!("$ubi_sub({left}, {right})"),
                        Symbol::Star => format!("$ubi_mul({left}, {right})"),
                        Symbol::Slash => format!("$ubi_div({left}, {right})"),
                        Symbol::Percent => format!("$ubi_rem({left}, {right})"),
                        _ => return Err(invariant_span(&expression.span)),
                    }
                } else {
                    let operator =
                        js_operator(*operator).ok_or_else(|| invariant_span(&expression.span))?;
                    format!("({left} {operator} {right})")
                }
            }
            ExprKind::Block(block) => self.block_expression(block, indent)?,
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.expression(condition, indent)?;
                let then_branch = self.block_expression(then_branch, indent)?;
                let else_branch = self.expression(else_branch, indent)?;
                format!("({condition} ? {then_branch} : {else_branch})")
            }
        };

        let ticks = if matches!(expression.kind, ExprKind::Integer(i32::MIN)) {
            "$ubi_tick($ubi_budget), $ubi_tick($ubi_budget), "
        } else {
            "$ubi_tick($ubi_budget), "
        };
        Ok(format!("({ticks}{value})"))
    }

    fn block_expression(&mut self, block: &BlockIr, indent: usize) -> Result<String, Diagnostic> {
        let contents = self.block_contents(block, indent + 1)?;
        Ok(format!("(() => {{\n{contents}\n{}}})()", spaces(indent)))
    }

    fn new_local(&mut self) -> String {
        let local = format!("$ubi_v{}", self.local_index);
        self.local_index += 1;
        local
    }
}

fn emit_export_wrapper(function: &FunctionIr, implementation: &str, binding: &str) -> String {
    let count = function.parameters.len();
    let argument_checks = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| js_argument_check(parameter.ty, &format!("$ubi_args[{index}]")))
        .collect::<Vec<_>>()
        .join(" && ");
    let argument_checks = if argument_checks.is_empty() {
        "true"
    } else {
        &argument_checks
    };
    let mut output = format!("function {binding}(...$ubi_args) {{\n");
    output.push_str(&format!(
        "  let $ubi_budget;\n  if ($ubi_args.length === {} && $ubi_is_budget($ubi_args[{}])) {{\n    $ubi_budget = $ubi_args.pop();\n  }} else if ($ubi_args.length === {count}) {{\n    $ubi_budget = $ubi_new_budget();\n  }} else {{\n    throw new TypeError(\"Invalid Ubi function arguments\");\n  }}\n",
        count + 1,
        count
    ));
    output.push_str(&format!(
        "  if ($ubi_args.length !== {count} || !({})) throw new TypeError(\"Invalid Ubi function arguments\");\n",
        argument_checks
    ));
    output.push_str(&format!(
        "  return {implementation}(...$ubi_args, $ubi_budget);\n}}\n"
    ));
    output
}

fn js_argument_check(ty: crate::analyzer::ValueType, value: &str) -> String {
    use crate::analyzer::ValueType;
    match ty {
        ValueType::Int => format!(
            "typeof {value} === \"number\" && Number.isInteger({value}) && {value} >= -2147483648 && {value} <= 2147483647"
        ),
        ValueType::Float => format!("typeof {value} === \"number\""),
        ValueType::Bool => format!("typeof {value} === \"boolean\""),
        ValueType::String => format!("$ubi_is_scalar_string({value})"),
        ValueType::Unit => format!("{value} === undefined"),
        ValueType::Never | ValueType::Error => "false".to_owned(),
    }
}

fn js_operator(operator: Symbol) -> Option<&'static str> {
    Some(match operator {
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Star => "*",
        Symbol::Slash => "/",
        Symbol::Percent => "%",
        Symbol::EqualEqual => "===",
        Symbol::BangEqual => "!==",
        Symbol::Less => "<",
        Symbol::LessEqual => "<=",
        Symbol::Greater => ">",
        Symbol::GreaterEqual => ">=",
        Symbol::AndAnd => "&&",
        Symbol::OrOr => "||",
        _ => return None,
    })
}

fn js_file_id(source_id: &str) -> String {
    format!("{}.js", source_id.strip_suffix(".ubi").unwrap_or(source_id))
}

fn relative_specifier(importer_id: &str, target_id: &str) -> String {
    let mut importer_dirs: Vec<_> = importer_id.split('/').collect();
    importer_dirs.pop();
    let mut target_parts: Vec<_> = target_id.split('/').collect();
    let file = target_parts.pop().unwrap_or(target_id);
    let target_file = format!("{}.js", file.strip_suffix(".ubi").unwrap_or(file));
    let common = importer_dirs
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();

    let mut parts = vec!["..".to_owned(); importer_dirs.len() - common];
    parts.extend(target_parts[common..].iter().map(|part| (*part).to_owned()));
    parts.push(target_file);
    let relative = parts
        .iter()
        .map(|part| percent_encode_path_segment(part))
        .collect::<Vec<_>>()
        .join("/");
    if relative.starts_with("../") {
        relative
    } else {
        format!("./{relative}")
    }
}

fn percent_encode_path_segment(segment: &str) -> String {
    let mut output = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn js_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{000c}' => output.push_str("\\f"),
            '\u{2028}' => output.push_str("\\u2028"),
            '\u{2029}' => output.push_str("\\u2029"),
            character if character < ' ' => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn spaces(indent: usize) -> String {
    "  ".repeat(indent)
}

fn invariant(source_id: &str, offset: usize) -> Diagnostic {
    Diagnostic::error(
        "UBI0090",
        "Compiler invariant failed while generating JavaScript",
        Span {
            source_id: source_id.to_owned(),
            start: offset,
            end: offset,
        },
    )
}

fn invariant_span(span: &Span) -> Diagnostic {
    Diagnostic::error(
        "UBI0090",
        "Compiler invariant failed while generating JavaScript",
        span.clone(),
    )
}

const RUNTIME: &str = r#"const $ubi_return = Symbol("ubi:return");
const $ubi_budget_key = Symbol.for("ubi.runtime.budget");
function $ubi_fault(code, message) { throw Object.freeze({ code, message }); }
function $ubi_new_budget() { return { [$ubi_budget_key]: true, remaining: 1000000, depth: 0 }; }
function $ubi_is_budget(value) { return !!value && value[$ubi_budget_key] === true; }
function $ubi_tick(budget) {
  if (budget.remaining <= 0) $ubi_fault("UBI-R0005", "Runtime execution limit exceeded");
  budget.remaining -= 1;
}
function $ubi_enter(budget) {
  if (budget.depth >= 32) $ubi_fault("UBI-R0005", "Runtime execution limit exceeded");
  $ubi_tick(budget);
  budget.depth += 1;
}
function $ubi_leave(budget) { budget.depth -= 1; }
function $ubi_i32(value) {
  if (value < -2147483648n || value > 2147483647n) $ubi_fault("UBI-R0001", "Integer overflow");
  return Number(value);
}
function $ubi_add(left, right) { return $ubi_i32(BigInt(left) + BigInt(right)); }
function $ubi_sub(left, right) { return $ubi_i32(BigInt(left) - BigInt(right)); }
function $ubi_mul(left, right) { return $ubi_i32(BigInt(left) * BigInt(right)); }
function $ubi_neg(value) {
  if (value === -2147483648) $ubi_fault("UBI-R0001", "Integer overflow");
  return -value;
}
function $ubi_div(left, right) {
  if (right === 0) $ubi_fault("UBI-R0002", "Integer division or remainder by zero");
  if (left === -2147483648 && right === -1) $ubi_fault("UBI-R0001", "Integer overflow");
  return Math.trunc(left / right);
}
function $ubi_rem(left, right) {
  if (right === 0) $ubi_fault("UBI-R0002", "Integer division or remainder by zero");
  if (left === -2147483648 && right === -1) return 0;
  const value = left % right;
  return value === 0 ? 0 : value;
}
function $ubi_is_scalar_string(value) {
  if (typeof value !== "string") return false;
  for (let i = 0; i < value.length; i += 1) {
    const unit = value.charCodeAt(i);
    if (unit >= 0xD800 && unit <= 0xDBFF) {
      const next = value.charCodeAt(i + 1);
      if (!(next >= 0xDC00 && next <= 0xDFFF)) return false;
      i += 1;
    } else if (unit >= 0xDC00 && unit <= 0xDFFF) {
      return false;
    }
  }
  return true;
}
"#;
