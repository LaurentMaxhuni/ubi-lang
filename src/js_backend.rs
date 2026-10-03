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
    {
        output.push_str(FOUNDATION_RUNTIME);
        let records = program
            .records
            .iter()
            .map(|record| {
                let fields = record
                    .fields
                    .iter()
                    .map(|(name, ty)| {
                        let ty = type_descriptor(*ty);
                        format!("[{}, {ty}]", js_string(name))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{fields}]")
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("\nconst $ubi_record_types = [{records}];\n"));
        let types = program
            .types
            .iter()
            .map(|ty| match ty {
                crate::analyzer::TypeInfo::List(inner)
                | crate::analyzer::TypeInfo::Option(inner) => type_descriptor(*inner),
                crate::analyzer::TypeInfo::Function { .. } => "null".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("const $ubi_types = [{types}];\n"));
    }
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
                StatementKind::Let {
                    name,
                    mutable,
                    value,
                } => {
                    let value = self.expression(value, indent)?;
                    let local = self.new_local();
                    let binding = if *mutable { "let" } else { "const" };
                    lines.push(format!("{prefix}{binding} {local} = {value};"));
                    self.scopes
                        .last_mut()
                        .ok_or_else(|| invariant_span(&statement.span))?
                        .insert(name.clone(), local);
                }
                StatementKind::Assign { name, value } => {
                    let local = self
                        .scopes
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(name))
                        .cloned()
                        .ok_or_else(|| invariant_span(&statement.span))?;
                    let value = self.expression(value, indent)?;
                    lines.push(format!("{prefix}{local} = {value};"));
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
                StatementKind::While { condition, body } => {
                    let condition = self.expression(condition, indent)?;
                    let body = self.block_expression(body, indent + 1)?;
                    lines.push(format!("{prefix}while (($ubi_tick($ubi_budget), {condition})) {{\n{prefix}  try {{ void ({body}); }} catch ($ubi_control) {{\n{prefix}    if ($ubi_control === $ubi_break) break;\n{prefix}    if ($ubi_control !== $ubi_continue) throw $ubi_control;\n{prefix}  }}\n{prefix}}}"));
                }
                StatementKind::For {
                    name,
                    iterable,
                    body,
                } => {
                    let iterable = self.expression(iterable, indent)?;
                    let local = self.new_local();
                    self.scopes
                        .push(BTreeMap::from([(name.clone(), local.clone())]));
                    let body = self.block_expression(body, indent + 1);
                    self.scopes.pop();
                    let body = body?;
                    lines.push(format!("{prefix}for (const {local} of $ubi_iter({iterable})) {{\n{prefix}  $ubi_tick($ubi_budget);\n{prefix}  try {{ void ({body}); }} catch ($ubi_control) {{\n{prefix}    if ($ubi_control === $ubi_break) break;\n{prefix}    if ($ubi_control !== $ubi_continue) throw $ubi_control;\n{prefix}  }}\n{prefix}}}"));
                }
                StatementKind::Break => lines.push(format!("{prefix}throw $ubi_break;")),
                StatementKind::Continue => lines.push(format!("{prefix}throw $ubi_continue;")),
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
            ExprKind::List(elements) => {
                let elements = elements
                    .iter()
                    .map(|element| self.expression(element, indent))
                    .collect::<Result<Vec<_>, _>>()?;
                format!("$ubi_make_list([{}])", elements.join(", "))
            }
            ExprKind::Some(value) => format!("$ubi_some({})", self.expression(value, indent)?),
            ExprKind::None => "$ubi_none".to_owned(),
            ExprKind::Index { object, index } => format!(
                "$ubi_index({}, {})",
                self.expression(object, indent)?,
                self.expression(index, indent)?
            ),
            ExprKind::Builtin { name, arguments } => {
                let values = arguments
                    .iter()
                    .map(|argument| self.expression(argument, indent))
                    .collect::<Result<Vec<_>, _>>()?;
                format!(
                    "$ubi_builtin({}, [{}], $ubi_budget, {})",
                    js_string(name),
                    values.join(", "),
                    type_descriptor(expression.ty)
                )
            }
            ExprKind::FunctionRef(target) => self
                .names
                .call_target(target)
                .ok_or_else(|| invariant_span(&expression.span))?
                .to_owned(),
            ExprKind::Invoke { callee, arguments } => {
                let callee = self.expression(callee, indent)?;
                let mut values = arguments
                    .iter()
                    .map(|argument| self.expression(argument, indent))
                    .collect::<Result<Vec<_>, _>>()?;
                values.push("$ubi_budget".to_owned());
                format!("({callee})({})", values.join(", "))
            }
            ExprKind::Arrow {
                parameters,
                body,
                captures,
            } => {
                let mut capture_scope = BTreeMap::new();
                let mut captured_values = Vec::new();
                let mut captured_names = Vec::new();
                for name in captures {
                    captured_values.push(
                        self.scopes
                            .iter()
                            .rev()
                            .find_map(|scope| scope.get(name))
                            .cloned()
                            .ok_or_else(|| invariant_span(&expression.span))?,
                    );
                    let local = self.new_local();
                    capture_scope.insert(name.clone(), local.clone());
                    captured_names.push(local);
                }
                self.scopes.push(capture_scope);
                let mut parameter_scope = BTreeMap::new();
                let mut names = Vec::new();
                for parameter in parameters {
                    let local = self.new_local();
                    parameter_scope.insert(parameter.name.clone(), local.clone());
                    names.push(local);
                }
                names.push("$ubi_budget".to_owned());
                self.scopes.push(parameter_scope);
                let body = self.expression(body, indent + 1);
                self.scopes.pop();
                self.scopes.pop();
                let body = body?;
                format!("(({}) => function({}) {{ $ubi_enter($ubi_budget); try {{ return {body}; }} catch ($ubi_error) {{ if ($ubi_error && $ubi_error[$ubi_return] === true) return $ubi_error.value; throw $ubi_error; }} finally {{ $ubi_leave($ubi_budget); }} }})({})", captured_names.join(", "), names.join(", "), captured_values.join(", "))
            }
            ExprKind::Match { subject, arms } => {
                let subject = self.expression(subject, indent)?;
                let subject_name = self.new_local();
                let mut body = format!("const {subject_name} = {subject};\n");
                for arm in arms {
                    let mut bindings = BTreeMap::new();
                    let mut declarations = Vec::new();
                    let condition = self.pattern(
                        &arm.pattern,
                        &subject_name,
                        &mut bindings,
                        &mut declarations,
                    )?;
                    self.scopes.push(bindings);
                    let guard = arm
                        .guard
                        .as_ref()
                        .map(|guard| self.expression(guard, indent + 1))
                        .transpose();
                    let value = self.expression(&arm.body, indent + 1);
                    self.scopes.pop();
                    let guard = guard?.unwrap_or_else(|| "true".to_owned());
                    let value = value?;
                    body.push_str(&format!(
                        "if ({condition}) {{ {} if ({guard}) return {value}; }}\n",
                        declarations.join(" ")
                    ));
                }
                body.push_str("throw new Error(\"Non-exhaustive Ubi match\");");
                format!("(() => {{ {body} }})()")
            }
            ExprKind::Local(name) => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .cloned()
                .ok_or_else(|| invariant_span(&expression.span))?,
            ExprKind::Record { base, fields } => {
                let base = base
                    .as_ref()
                    .map(|base| self.expression(base, indent))
                    .transpose()?;
                let fields = fields
                    .iter()
                    .map(|(name, value)| {
                        Ok(format!(
                            "[{}, {}]",
                            js_string(name),
                            self.expression(value, indent)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .join(", ");
                match base {
                    Some(base) => format!("$ubi_update_record({base}, [{fields}])"),
                    None => format!("$ubi_make_record([{fields}])"),
                }
            }
            ExprKind::Member { object, name } => format!(
                "({})[{}]",
                self.expression(object, indent)?,
                js_string(name)
            ),
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
                format!("($ubi_tick($ubi_budget), {target}({}))", values.join(", "))
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
                let record_equality =
                    matches!(
                        left.ty,
                        ValueType::Record(_) | ValueType::List(_) | ValueType::Option(_)
                    ) && matches!(operator, Symbol::EqualEqual | Symbol::BangEqual);
                let left = self.expression(left, indent)?;
                let right = self.expression(right, indent)?;
                if record_equality {
                    let prefix = if *operator == Symbol::BangEqual {
                        "!"
                    } else {
                        ""
                    };
                    format!("({prefix}$ubi_equal({left}, {right}))")
                } else if expression.ty == ValueType::String && *operator == Symbol::Plus {
                    format!("$ubi_string({left} + {right})")
                } else if expression.ty == ValueType::Int {
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

    fn pattern(
        &mut self,
        pattern: &crate::parser::Pattern,
        value: &str,
        bindings: &mut BTreeMap<String, String>,
        declarations: &mut Vec<String>,
    ) -> Result<String, Diagnostic> {
        use crate::parser::PatternKind;
        Ok(match &pattern.kind {
            PatternKind::Wildcard => "true".to_owned(),
            PatternKind::Binding(name) => {
                let local = self.new_local();
                declarations.push(format!("const {local} = {value};"));
                bindings.insert(name.name.clone(), local);
                "true".to_owned()
            }
            PatternKind::Integer(text) => {
                let integer = text
                    .parse::<i32>()
                    .map_err(|_| invariant_span(&pattern.span))?;
                format!("{value} === {integer}")
            }
            PatternKind::String(text) => format!("{value} === {}", js_string(text)),
            PatternKind::Bool(boolean) => format!("{value} === {boolean}"),
            PatternKind::None => format!("{value}.tag === \"None\""),
            PatternKind::Some(inner) => {
                let condition =
                    self.pattern(inner, &format!("{value}.value"), bindings, declarations)?;
                format!("({value}.tag === \"Some\" && ({condition}))")
            }
        })
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
    output.push_str("  let $ubi_internal = false;\n");
    output.push_str(&format!(
        "  let $ubi_budget;\n  if ($ubi_args.length === {} && $ubi_is_budget($ubi_args[{}])) {{\n    $ubi_internal = true;\n    $ubi_budget = $ubi_args.pop();\n  }} else if ($ubi_args.length === {count}) {{\n    $ubi_budget = $ubi_new_budget();\n  }} else {{\n    throw new TypeError(\"Invalid Ubi function arguments\");\n  }}\n",
        count + 1,
        count
    ));
    for (index, parameter) in function.parameters.iter().enumerate() {
        if matches!(
            parameter.ty,
            ValueType::Record(_) | ValueType::List(_) | ValueType::Option(_)
        ) {
            output.push_str(&format!("  if (!$ubi_internal) $ubi_args[{index}] = $ubi_read_aggregate($ubi_args[{index}], {});\n", type_descriptor(parameter.ty)));
        } else if parameter.ty == ValueType::Int {
            output.push_str(&format!(
                "  if ($ubi_args[{index}] === 0) $ubi_args[{index}] = 0;\n"
            ));
        }
    }
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
        ValueType::Record(_) | ValueType::List(_) | ValueType::Option(_) => "true".to_owned(),
        ValueType::Never | ValueType::Error | ValueType::Function(_) | ValueType::Range => "false".to_owned(),
    }
}

fn type_descriptor(ty: ValueType) -> String {
    match ty {
        ValueType::Record(id) => format!("[\"record\", {id}]"),
        ValueType::List(id) => format!("[\"list\", {id}]"),
        ValueType::Option(id) => format!("[\"option\", {id}]"),
        ValueType::Function(_) => js_string("function"),
        ValueType::Range => js_string("range"),
        _ => js_string(ty.name()),
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
    format!(
        "{}.mjs",
        source_id.strip_suffix(".ubi").unwrap_or(source_id)
    )
}

fn relative_specifier(importer_id: &str, target_id: &str) -> String {
    let mut importer_dirs: Vec<_> = importer_id.split('/').collect();
    importer_dirs.pop();
    let mut target_parts: Vec<_> = target_id.split('/').collect();
    let file = target_parts.pop().unwrap_or(target_id);
    let target_file = format!("{}.mjs", file.strip_suffix(".ubi").unwrap_or(file));
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

const FOUNDATION_RUNTIME: &str = r#"
const $ubi_break = Symbol("ubi:break");
const $ubi_continue = Symbol("ubi:continue");
const $ubi_aggregate_depth = Symbol.for("ubi:aggregate-depth:v1");
function $ubi_freeze(value, children) {
  const depth = 1 + children.reduce((max, child) => Math.max(max, child && typeof child === "object" ? child[$ubi_aggregate_depth] || 0 : 0), 0);
  if (depth > 32) $ubi_fault("UBI-R0005", "Aggregate nesting limit exceeded");
  Object.defineProperty(value, $ubi_aggregate_depth, { value: depth });
  return Object.freeze(value);
}
function $ubi_make_record(entries) {
  const value = Object.fromEntries(entries);
  return $ubi_freeze(value, Object.values(value));
}
function $ubi_update_record(base, entries) { return $ubi_make_record(Object.entries(base).concat(entries)); }
function $ubi_make_list(values) {
  if (values.length > 2147483647) $ubi_fault("UBI-R0003", "List length limit exceeded");
  return $ubi_freeze(values, values);
}
function $ubi_some(value) { return $ubi_freeze({ tag: "Some", value }, [value]); }
const $ubi_none = $ubi_freeze({ tag: "None" }, []);
function $ubi_equal(left, right) {
  if (left === null || typeof left !== "object") return left === right;
  if (right === null || typeof right !== "object") return false;
  const names = Object.keys(left);
  return names.length === Object.keys(right).length && names.every(name => Object.prototype.hasOwnProperty.call(right, name) && $ubi_equal(left[name], right[name]));
}
function $ubi_string(value) {
  if (Array.from(value).length > 2147483647) $ubi_fault("UBI-R0003", "String length limit exceeded");
  return value;
}
function $ubi_index(value, index) {
  const values = typeof value === "string" ? Array.from(value) : value;
  return index < 0 || index >= values.length ? $ubi_none : $ubi_some(values[index]);
}
function* $ubi_iter(value) {
  if (value && value.$ubi_range === true) {
    for (let i = value.start; i < value.end; i += 1) yield i;
  } else {
    yield* value;
  }
}
function $ubi_read_aggregate(text, type) {
  if (typeof text !== "string") throw new TypeError("Aggregate arguments must be serialized JSON");
  let data;
  try { data = JSON.parse(text); } catch { throw new TypeError("Invalid aggregate JSON"); }
  return $ubi_decode(data, type, 0);
}
function $ubi_decode(data, type, depth) {
  if (Array.isArray(type)) {
    if (depth >= 32 || data === null || typeof data !== "object") throw new TypeError("Invalid aggregate data or nesting");
    const [kind, id] = type;
    if (kind === "list") {
      if (!Array.isArray(data)) throw new TypeError("Invalid list data");
      return $ubi_make_list(data.map(value => $ubi_decode(value, $ubi_types[id], depth + 1)));
    }
    if (Array.isArray(data)) throw new TypeError("Invalid aggregate data");
    if (kind === "option") {
      const names = Object.keys(data);
      if (data.tag === "None" && names.length === 1) return $ubi_none;
      if (data.tag !== "Some" || names.length !== 2 || !Object.prototype.hasOwnProperty.call(data, "value")) throw new TypeError("Invalid Option data");
      return $ubi_some($ubi_decode(data.value, $ubi_types[id], depth + 1));
    }
    const fields = $ubi_record_types[id];
    if (Object.keys(data).length !== fields.length) throw new TypeError("Invalid record fields");
    return $ubi_make_record(fields.map(([name, fieldType]) => {
      if (!Object.prototype.hasOwnProperty.call(data, name)) throw new TypeError("Missing record field");
      return [name, $ubi_decode(data[name], fieldType, depth + 1)];
    }));
  }
  if (type === "unit" && data === null) data = undefined;
  if (type === "float" && typeof data === "string") {
    const specials = { "NaN": NaN, "+Infinity": Infinity, "-Infinity": -Infinity, "+0": 0, "-0": -0 };
    if (Object.prototype.hasOwnProperty.call(specials, data)) data = specials[data];
  }
  const valid = type === "int" ? typeof data === "number" && Number.isInteger(data) && data >= -2147483648 && data <= 2147483647
    : type === "float" ? typeof data === "number"
    : type === "bool" ? typeof data === "boolean"
    : type === "string" ? $ubi_is_scalar_string(data)
    : type === "unit" && data === undefined;
  if (!valid) throw new TypeError("Invalid aggregate element type");
  return type === "int" && data === 0 ? 0 : data;
}
function $ubi_builtin(name, args, budget, type) {
  const [a, b, c] = args;
  switch (name) {
    case "length": return typeof a === "string" ? Array.from(a).length : a.length;
    case "append": return $ubi_make_list([...a, b]);
    case "set": {
      if (b < 0 || b >= a.length) return $ubi_none;
      const result = a.slice(); result[b] = c;
      return $ubi_some($ubi_make_list(result));
    }
    case "map": {
      const result = [];
      for (const value of a) { $ubi_tick(budget); result.push(b(value, budget)); }
      return $ubi_make_list(result);
    }
    case "filter": {
      const result = [];
      for (const value of a) { $ubi_tick(budget); if (b(value, budget)) result.push(value); }
      return $ubi_make_list(result);
    }
    case "find": {
      for (const value of a) { $ubi_tick(budget); if (b(value, budget)) return $ubi_some(value); }
      return $ubi_none;
    }
    case "fold": {
      let result = b;
      for (const value of a) { $ubi_tick(budget); result = c(result, value, budget); }
      return result;
    }
    case "range": return Object.freeze({ $ubi_range: true, start: a, end: b });
    case "contains": return a.includes(b);
    case "startsWith": return a.startsWith(b);
    case "endsWith": return a.endsWith(b);
    case "trim": return a.replace(/^[ \t\r\n]+|[ \t\r\n]+$/g, "");
    case "split": return $ubi_make_list(b === "" ? Array.from(a) : a.split(b));
    case "join": return $ubi_string(a.join(b));
    case "replace": return $ubi_string(b === "" ? c + Array.from(a).join(c) + (a === "" ? "" : c) : a.split(b).join(c));
    case "slice": return Array.from(a).slice(Math.max(0, b), Math.max(0, c)).join("");
    case "abs":
      if (type === "int" && a === -2147483648) $ubi_fault("UBI-R0001", "Integer overflow");
      return Math.abs(a);
    case "min": return Math.min(a, b);
    case "max": return Math.max(a, b);
    case "clamp":
      if (b > c) $ubi_fault("UBI-R0003", "Invalid clamp bounds");
      return Math.min(Math.max(a, b), c);
    case "floor": return Math.floor(a);
    case "ceil": return Math.ceil(a);
    case "round": {
      if (!Number.isFinite(a)) return a;
      const magnitude = Math.abs(a), whole = Math.floor(magnitude);
      const result = magnitude - whole >= 0.5 ? whole + 1 : whole;
      return a < 0 || Object.is(a, -0) ? -result : result;
    }
    case "sqrt": return Math.sqrt(a);
    case "sin": return Math.sin(a);
    case "cos": return Math.cos(a);
    case "tan": return Math.tan(a);
    case "log": return Math.log(a);
    case "exp": return Math.exp(a);
    case "pow": return Math.pow(a, b);
    case "toFloat": return a;
    case "toInt": {
      const value = Math.trunc(a);
      return Number.isFinite(value) && value >= -2147483648 && value <= 2147483647 ? $ubi_some(value === 0 ? 0 : value) : $ubi_none;
    }
    case "parseInt": {
      if (!/^[+-]?[0-9]+(?![\s\S])/.test(a)) return $ubi_none;
      const value = Number(a);
      return Number.isInteger(value) && value >= -2147483648 && value <= 2147483647 ? $ubi_some(value === 0 ? 0 : value) : $ubi_none;
    }
    case "parseFloat": {
      if (!/^[+-]?[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?(?![\s\S])/.test(a)) return $ubi_none;
      const value = Number(a);
      return Number.isFinite(value) ? $ubi_some(value) : $ubi_none;
    }
    case "toString": return String(a);
    default: throw new Error("Unknown Ubi built-in");
  }
}
"#;

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
  const value = Math.trunc(left / right);
  return value === 0 ? 0 : value;
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
