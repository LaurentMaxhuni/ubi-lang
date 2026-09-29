use crate::lexer::{lex, Comment, LexError, Symbol, Token, TokenKind};
use crate::source::{MAX_NESTING, MAX_SOURCE_BYTES, MAX_TOKENS};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParseError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) primary: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Module {
    pub(crate) imports: Vec<Import>,
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) comments: Vec<Comment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Import {
    pub(crate) names: Vec<Identifier>,
    pub(crate) path: String,
    pub(crate) path_span: Span,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Declaration {
    Function(Function),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Function {
    pub(crate) exported: bool,
    pub(crate) name: Identifier,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) return_type: Option<Type>,
    pub(crate) body: Block,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parameter {
    pub(crate) name: Identifier,
    pub(crate) ty: Type,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Type {
    pub(crate) name: String,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identifier {
    pub(crate) name: String,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) statements: Vec<Statement>,
    pub(crate) tail: Option<Box<Expr>>,
    pub(crate) span: Span,
    nesting: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Statement {
    pub(crate) kind: StatementKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatementKind {
    Let {
        name: Identifier,
        annotation: Option<Type>,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Return(Option<Expr>),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) span: Span,
    nesting: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExprKind {
    Integer {
        value: String,
        literal_span: Span,
    },
    Float {
        value: String,
        literal_span: Span,
    },
    String(String),
    Bool(bool),
    Unit,
    Name(Identifier),
    Unary {
        operator: Symbol,
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        operator: Symbol,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        name: Identifier,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Propagate(Box<Expr>),
    Block(Block),
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Box<Expr>,
    },
}

pub(crate) fn parse(source_id: &str, source: &str) -> Result<Module, ParseError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(ParseError {
            code: "UBI0090".to_owned(),
            message: "Compiler resource limit exceeded".to_owned(),
            primary: Span {
                source_id: source_id.to_owned(),
                start: 0,
                end: 0,
            },
        });
    }

    let lexed = lex(source_id, source).map_err(from_lex_error)?;
    if lexed.tokens.len().saturating_sub(1) > MAX_TOKENS {
        let token = &lexed.tokens[MAX_TOKENS];
        return Err(resource_error(token.span.clone()));
    }

    Parser {
        tokens: lexed.tokens,
        comments: lexed.comments,
        position: 0,
        depth: 0,
    }
    .parse_module()
}

fn from_lex_error(error: LexError) -> ParseError {
    ParseError {
        code: error.code,
        message: error.message,
        primary: error.primary,
    }
}

struct Parser {
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    position: usize,
    depth: usize,
}

impl Parser {
    fn parse_module(mut self) -> Result<Module, ParseError> {
        let mut imports = Vec::new();
        while self.at_keyword("import") {
            imports.push(self.parse_import()?);
        }

        let mut declarations = Vec::new();
        while !self.at_eof() {
            declarations.push(self.parse_declaration()?);
        }

        Ok(Module {
            imports,
            declarations,
            comments: self.comments,
        })
    }

    fn parse_import(&mut self) -> Result<Import, ParseError> {
        let start = self.expect_keyword("import")?.span;
        self.expect_symbol(Symbol::LBrace)?;
        let mut names = Vec::new();
        if !self.at_symbol(Symbol::RBrace) {
            loop {
                names.push(self.parse_identifier()?);
                if self.eat_symbol(Symbol::Comma).is_none() {
                    break;
                }
                if self.at_symbol(Symbol::RBrace) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RBrace)?;
        self.expect_keyword("from")?;
        let (path, path_span) = match &self.current().kind {
            TokenKind::String(path) => {
                let path = path.clone();
                let span = self.current().span.clone();
                self.advance();
                (path, span)
            }
            _ => return Err(self.error("Expected an import path string")),
        };
        let end = self.expect_symbol(Symbol::Semicolon)?.span;
        if names.is_empty() {
            return Err(ParseError {
                code: "UBI0002".to_owned(),
                message: "An import requires at least one name".to_owned(),
                primary: self.previous().span.clone(),
            });
        }
        Ok(Import {
            names,
            path,
            path_span,
            span: cover(&start, &end),
        })
    }

    fn parse_declaration(&mut self) -> Result<Declaration, ParseError> {
        let export_span = self.eat_keyword("export").map(|token| token.span);
        let exported = export_span.is_some();
        if self.at_keyword("fn") {
            return self
                .parse_function(exported, export_span)
                .map(Declaration::Function);
        }
        if self.at_keyword("record") || self.at_keyword("enum") {
            return Err(self.unsupported(self.current().span.clone(), "Type declarations"));
        }
        if is_unsupported_keyword(self.current()) {
            return Err(self.unsupported(self.current().span.clone(), "This language feature"));
        }
        Err(self.error("Expected a function declaration"))
    }

    fn parse_function(
        &mut self,
        exported: bool,
        declaration_start: Option<Span>,
    ) -> Result<Function, ParseError> {
        let fn_span = self.expect_keyword("fn")?.span;
        let name = self.parse_identifier()?;
        if self.at_symbol(Symbol::Less) {
            return Err(self.unsupported(self.current().span.clone(), "Generic functions"));
        }
        self.expect_symbol(Symbol::LParen)?;
        let mut parameters = Vec::new();
        if !self.at_symbol(Symbol::RParen) {
            loop {
                let parameter_name = self.parse_identifier()?;
                self.expect_symbol(Symbol::Colon)?;
                let ty = self.parse_type()?;
                let parameter = Parameter {
                    span: cover(&parameter_name.span, &ty.span),
                    name: parameter_name,
                    ty,
                };
                parameters.push(parameter);
                if self.eat_symbol(Symbol::Comma).is_none() {
                    break;
                }
                if self.at_symbol(Symbol::RParen) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RParen)?;
        let return_type = if self.eat_symbol(Symbol::ThinArrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = cover(declaration_start.as_ref().unwrap_or(&fn_span), &body.span);
        Ok(Function {
            exported,
            name,
            parameters,
            return_type,
            body,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        if self.at_eof() {
            return Err(self.error("Expected a type name"));
        }
        let token = self.advance().clone();
        let name = match token.kind {
            TokenKind::Identifier(name) => name,
            TokenKind::Keyword(name)
                if matches!(name.as_str(), "int" | "float" | "bool" | "string" | "unit") =>
            {
                name
            }
            TokenKind::Keyword(_) if is_unsupported_keyword(&token) => {
                return Err(self.unsupported(token.span, "This language feature"));
            }
            _ => return Err(self.error_previous("Expected a type name")),
        };
        if self.at_symbol(Symbol::Less) {
            return Err(self.unsupported(self.current().span.clone(), "Generic types"));
        }
        Ok(Type {
            name,
            span: token.span,
        })
    }

    fn parse_identifier(&mut self) -> Result<Identifier, ParseError> {
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let identifier = Identifier {
                    name: name.clone(),
                    span: self.current().span.clone(),
                };
                self.advance();
                Ok(identifier)
            }
            _ => Err(self.error("Expected a name")),
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?.span;
        let mut statements = Vec::new();
        let mut tail = None;
        let mut child_nesting = 0;

        while !self.at_symbol(Symbol::RBrace) && !self.at_eof() {
            if self.at_keyword("let") {
                let statement = self.parse_let_statement()?;
                child_nesting = child_nesting.max(statement_nesting(&statement));
                statements.push(statement);
                continue;
            }
            if self.at_keyword("return") {
                let statement = self.parse_return_statement()?;
                child_nesting = child_nesting.max(statement_nesting(&statement));
                statements.push(statement);
                continue;
            }

            let expression = self.parse_expression(0)?;
            if self.eat_symbol(Symbol::Equal).is_some() {
                let target_span = expression.span.clone();
                if !matches!(expression.kind, ExprKind::Name(_)) {
                    return Err(self.unsupported(target_span, "Assignment to fields or indexes"));
                }
                let value = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::Semicolon)?.span;
                let statement = Statement {
                    span: cover(&expression.span, &end),
                    kind: StatementKind::Assign {
                        target: expression,
                        value,
                    },
                };
                child_nesting = child_nesting.max(statement_nesting(&statement));
                statements.push(statement);
            } else if let Some(end) = self.eat_symbol(Symbol::Semicolon) {
                let statement = Statement {
                    span: cover(&expression.span, &end.span),
                    kind: StatementKind::Expression(expression),
                };
                child_nesting = child_nesting.max(statement_nesting(&statement));
                statements.push(statement);
            } else {
                tail = Some(Box::new(expression));
                if !self.at_symbol(Symbol::RBrace) {
                    return Err(self.error("Expected `;` or `}` after expression"));
                }
            }
        }

        let close = self.expect_symbol(Symbol::RBrace)?.span;
        let span = cover(&open, &close);
        child_nesting = child_nesting.max(tail.as_ref().map_or(0, |expression| expression.nesting));
        let nesting = self.nested_depth(std::iter::once(child_nesting), &span)?;
        Ok(Block {
            statements,
            tail,
            span,
            nesting,
        })
    }

    fn parse_let_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect_keyword("let")?.span;
        if self.at_keyword("mut") {
            return Err(self.unsupported(self.current().span.clone(), "Mutable bindings"));
        }
        let name = self.parse_identifier()?;
        let annotation = if self.eat_symbol(Symbol::Colon).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_symbol(Symbol::Equal)?;
        let value = self.parse_expression(0)?;
        let end = self.expect_symbol(Symbol::Semicolon)?.span;
        Ok(Statement {
            span: cover(&start, &end),
            kind: StatementKind::Let {
                name,
                annotation,
                value,
            },
        })
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.expect_keyword("return")?.span;
        let value = if self.at_symbol(Symbol::Semicolon) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        let end = self.expect_symbol(Symbol::Semicolon)?.span;
        Ok(Statement {
            span: cover(&start, &end),
            kind: StatementKind::Return(value),
        })
    }

    fn parse_expression(&mut self, min_binding_power: u8) -> Result<Expr, ParseError> {
        let mut comparison_seen = false;
        self.parse_expression_with_comparison(min_binding_power, &mut comparison_seen)
    }

    fn parse_expression_with_comparison(
        &mut self,
        min_binding_power: u8,
        comparison_seen: &mut bool,
    ) -> Result<Expr, ParseError> {
        if self.depth >= MAX_NESTING {
            return Err(resource_error(self.current().span.clone()));
        }
        self.depth += 1;
        let result = self.parse_expression_inner(min_binding_power, comparison_seen);
        self.depth -= 1;
        result
    }

    fn parse_expression_inner(
        &mut self,
        min_binding_power: u8,
        comparison_seen: &mut bool,
    ) -> Result<Expr, ParseError> {
        let mut left = self.parse_prefix_or_primary(comparison_seen)?;
        loop {
            if self.at_symbol(Symbol::LBrace) && matches!(left.kind, ExprKind::Name(_)) {
                return Err(self.unsupported(self.current().span.clone(), "Record values"));
            }
            if self.at_symbol(Symbol::LParen) {
                let arguments = self.parse_arguments()?;
                let end = self.previous().span.clone();
                let span = cover(&left.span, &end);
                let nesting = self.nested_depth(
                    std::iter::once(left.nesting)
                        .chain(arguments.iter().map(|argument| argument.nesting)),
                    &span,
                )?;
                left = Expr {
                    span,
                    nesting,
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                };
                continue;
            }
            if self.eat_symbol(Symbol::Dot).is_some() {
                let name = self.parse_identifier()?;
                let span = cover(&left.span, &name.span);
                let nesting = self.nested_depth(std::iter::once(left.nesting), &span)?;
                left = Expr {
                    span,
                    nesting,
                    kind: ExprKind::Member {
                        object: Box::new(left),
                        name,
                    },
                };
                continue;
            }
            if self.eat_symbol(Symbol::LBracket).is_some() {
                let index = self.parse_expression(0)?;
                let end = self.expect_symbol(Symbol::RBracket)?.span;
                let span = cover(&left.span, &end);
                let nesting =
                    self.nested_depth([left.nesting, index.nesting].into_iter(), &span)?;
                left = Expr {
                    span,
                    nesting,
                    kind: ExprKind::Index {
                        object: Box::new(left),
                        index: Box::new(index),
                    },
                };
                continue;
            }
            if self.eat_symbol(Symbol::Question).is_some() {
                let end = self.previous().span.clone();
                let span = cover(&left.span, &end);
                let nesting = self.nested_depth(std::iter::once(left.nesting), &span)?;
                left = Expr {
                    span,
                    nesting,
                    kind: ExprKind::Propagate(Box::new(left)),
                };
                continue;
            }

            let Some((left_power, right_power)) = binary_binding_power(self.current()) else {
                break;
            };
            if left_power < min_binding_power {
                break;
            }
            let operator = match self.current().kind {
                TokenKind::Symbol(operator) => operator,
                _ => unreachable!("binding power only exists for symbols"),
            };
            if is_comparison(operator) {
                if *comparison_seen {
                    return Err(self.error("Comparison operators cannot be chained"));
                }
                *comparison_seen = true;
            }
            let operator_span = self.advance().span.clone();
            let right = if matches!(operator, Symbol::AndAnd | Symbol::OrOr) {
                self.parse_expression(right_power)?
            } else {
                self.parse_expression_with_comparison(right_power, comparison_seen)?
            };
            let span = cover(&left.span, &right.span);
            let nesting =
                self.nested_depth([left.nesting, right.nesting].into_iter(), &operator_span)?;
            left = Expr {
                span,
                nesting,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_prefix_or_primary(&mut self, comparison_seen: &mut bool) -> Result<Expr, ParseError> {
        if let TokenKind::Symbol(operator @ (Symbol::Minus | Symbol::Bang)) = self.current().kind {
            let start = self.advance().span.clone();
            let operand = self.parse_expression_with_comparison(13, comparison_seen)?;
            let span = cover(&start, &operand.span);
            let nesting = self.nested_depth(std::iter::once(operand.nesting), &span)?;
            return Ok(Expr {
                span,
                nesting,
                kind: ExprKind::Unary {
                    operator,
                    operand: Box::new(operand),
                },
            });
        }

        let token = self.advance().clone();
        match token.kind {
            TokenKind::Eof => Err(self.error("Expected an expression")),
            TokenKind::Integer(value) => Ok(Expr {
                kind: ExprKind::Integer {
                    value,
                    literal_span: token.span.clone(),
                },
                span: token.span,
                nesting: 1,
            }),
            TokenKind::Float(value) => Ok(Expr {
                kind: ExprKind::Float {
                    value,
                    literal_span: token.span.clone(),
                },
                span: token.span,
                nesting: 1,
            }),
            TokenKind::String(value) => Ok(Expr {
                kind: ExprKind::String(value),
                span: token.span,
                nesting: 1,
            }),
            TokenKind::Identifier(name) => Ok(Expr {
                span: token.span.clone(),
                nesting: 1,
                kind: ExprKind::Name(Identifier {
                    name,
                    span: token.span,
                }),
            }),
            TokenKind::Keyword(ref keyword) if keyword == "true" || keyword == "false" => {
                Ok(Expr {
                    kind: ExprKind::Bool(keyword == "true"),
                    span: token.span,
                    nesting: 1,
                })
            }
            TokenKind::Keyword(ref keyword) if keyword == "if" => {
                self.parse_if_expression(token.span)
            }
            TokenKind::Keyword(ref keyword) if keyword == "match" => {
                Err(self.unsupported(token.span, "Pattern matching"))
            }
            TokenKind::Keyword(_) if is_unsupported_keyword(&token) => {
                Err(self.unsupported(token.span, "This language feature"))
            }
            TokenKind::Symbol(Symbol::LParen) if self.arrow_follows_parentheses() => {
                Err(self.unsupported(token.span, "Arrow functions"))
            }
            TokenKind::Symbol(Symbol::LParen) => {
                if let Some(close) = self.eat_symbol(Symbol::RParen) {
                    return Ok(Expr {
                        kind: ExprKind::Unit,
                        span: cover(&token.span, &close.span),
                        nesting: 1,
                    });
                }
                let mut expression = self.parse_expression(0)?;
                let close = self.expect_symbol(Symbol::RParen)?.span;
                expression.span = cover(&token.span, &close);
                Ok(expression)
            }
            TokenKind::Symbol(Symbol::LBrace) => {
                self.position -= 1;
                let block = self.parse_block()?;
                let span = block.span.clone();
                let nesting = self.nested_depth(std::iter::once(block.nesting), &span)?;
                Ok(Expr {
                    span,
                    nesting,
                    kind: ExprKind::Block(block),
                })
            }
            TokenKind::Symbol(Symbol::LBracket) => {
                Err(self.unsupported(token.span, "List expressions"))
            }
            TokenKind::Symbol(Symbol::Underscore) => {
                Err(self.error_previous("Expected an expression"))
            }
            _ => Err(self.error_previous("Expected an expression")),
        }
    }

    fn parse_if_expression(&mut self, start: Span) -> Result<Expr, ParseError> {
        self.expect_symbol(Symbol::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect_symbol(Symbol::RParen)?;
        let then_branch = self.parse_block()?;
        self.expect_keyword("else")?;
        let else_branch = if self.at_keyword("if") {
            self.parse_expression(0)?
        } else {
            let block = self.parse_block()?;
            let span = block.span.clone();
            let nesting = self.nested_depth(std::iter::once(block.nesting), &span)?;
            Expr {
                span,
                nesting,
                kind: ExprKind::Block(block),
            }
        };
        let span = cover(&start, &else_branch.span);
        let nesting = self.nested_depth(
            [condition.nesting, then_branch.nesting, else_branch.nesting].into_iter(),
            &span,
        )?;
        Ok(Expr {
            span,
            nesting,
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch: Box::new(else_branch),
            },
        })
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect_symbol(Symbol::LParen)?;
        let mut arguments = Vec::new();
        if !self.at_symbol(Symbol::RParen) {
            loop {
                arguments.push(self.parse_expression(0)?);
                if self.eat_symbol(Symbol::Comma).is_none() {
                    break;
                }
                if self.at_symbol(Symbol::RParen) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RParen)?;
        Ok(arguments)
    }

    fn arrow_follows_parentheses(&self) -> bool {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(self.position) {
            match token.kind {
                TokenKind::Symbol(Symbol::LParen) => depth += 1,
                TokenKind::Symbol(Symbol::RParen) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return self
                            .tokens
                            .get(index + 1)
                            .is_some_and(|next| next.kind == TokenKind::Symbol(Symbol::FatArrow));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<Token, ParseError> {
        if self.at_keyword(keyword) {
            Ok(self.advance().clone())
        } else {
            Err(self.error(&format!("Expected `{keyword}`")))
        }
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<Token, ParseError> {
        if self.at_symbol(symbol) {
            Ok(self.advance().clone())
        } else {
            Err(self.error(&format!("Expected `{}`", symbol_text(symbol))))
        }
    }

    fn eat_keyword(&mut self, keyword: &str) -> Option<Token> {
        self.at_keyword(keyword).then(|| self.advance().clone())
    }

    fn eat_symbol(&mut self, symbol: Symbol) -> Option<Token> {
        self.at_symbol(symbol).then(|| self.advance().clone())
    }

    fn at_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(value) if value == keyword)
    }

    fn at_symbol(&self, symbol: Symbol) -> bool {
        self.current().kind == TokenKind::Symbol(symbol)
    }

    fn at_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.position.saturating_sub(1)]
    }

    fn advance(&mut self) -> &Token {
        let current = self.position;
        if !self.at_eof() {
            self.position += 1;
        }
        &self.tokens[current]
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            code: "UBI0002".to_owned(),
            message: message.to_owned(),
            primary: self.current().span.clone(),
        }
    }

    fn error_previous(&self, message: &str) -> ParseError {
        ParseError {
            code: "UBI0002".to_owned(),
            message: message.to_owned(),
            primary: self.previous().span.clone(),
        }
    }

    fn unsupported(&self, span: Span, feature: &str) -> ParseError {
        ParseError {
            code: "UBI0003".to_owned(),
            message: format!("Unsupported in this language profile: {feature}"),
            primary: span,
        }
    }

    fn nested_depth(
        &self,
        child_depths: impl Iterator<Item = usize>,
        span: &Span,
    ) -> Result<usize, ParseError> {
        let depth = child_depths.max().unwrap_or(0) + 1;
        if depth > MAX_NESTING {
            Err(resource_error(span.clone()))
        } else {
            Ok(depth)
        }
    }
}

fn binary_binding_power(token: &Token) -> Option<(u8, u8)> {
    let TokenKind::Symbol(symbol) = token.kind else {
        return None;
    };
    Some(match symbol {
        Symbol::OrOr => (1, 2),
        Symbol::AndAnd => (3, 4),
        Symbol::EqualEqual | Symbol::BangEqual => (5, 6),
        Symbol::Less | Symbol::LessEqual | Symbol::Greater | Symbol::GreaterEqual => (7, 8),
        Symbol::Plus | Symbol::Minus => (9, 10),
        Symbol::Star | Symbol::Slash | Symbol::Percent => (11, 12),
        _ => return None,
    })
}

fn is_comparison(symbol: Symbol) -> bool {
    matches!(
        symbol,
        Symbol::Less
            | Symbol::LessEqual
            | Symbol::Greater
            | Symbol::GreaterEqual
            | Symbol::EqualEqual
            | Symbol::BangEqual
    )
}

fn is_unsupported_keyword(token: &Token) -> bool {
    matches!(
        &token.kind,
        TokenKind::Keyword(keyword)
            if matches!(
                keyword.as_str(),
                "async" | "await" | "throw" | "try" | "catch" | "class" | "while"
                    | "for" | "break" | "continue" | "new" | "null" | "undefined"
                    | "any" | "as"
            )
    )
}

fn symbol_text(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::LParen => "(",
        Symbol::RParen => ")",
        Symbol::LBrace => "{",
        Symbol::RBrace => "}",
        Symbol::LBracket => "[",
        Symbol::RBracket => "]",
        Symbol::Comma => ",",
        Symbol::Colon => ":",
        Symbol::Semicolon => ";",
        Symbol::Dot => ".",
        Symbol::Ellipsis => "...",
        Symbol::Underscore => "_",
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Star => "*",
        Symbol::Slash => "/",
        Symbol::Percent => "%",
        Symbol::Bang => "!",
        Symbol::Equal => "=",
        Symbol::EqualEqual => "==",
        Symbol::BangEqual => "!=",
        Symbol::Less => "<",
        Symbol::LessEqual => "<=",
        Symbol::Greater => ">",
        Symbol::GreaterEqual => ">=",
        Symbol::AndAnd => "&&",
        Symbol::OrOr => "||",
        Symbol::ThinArrow => "->",
        Symbol::FatArrow => "=>",
        Symbol::Question => "?",
    }
}

fn cover(first: &Span, last: &Span) -> Span {
    Span {
        source_id: first.source_id.clone(),
        start: first.start,
        end: last.end,
    }
}

fn statement_nesting(statement: &Statement) -> usize {
    match &statement.kind {
        StatementKind::Let { value, .. } | StatementKind::Expression(value) => value.nesting,
        StatementKind::Assign { target, value } => target.nesting.max(value.nesting),
        StatementKind::Return(value) => value.as_ref().map_or(0, |expression| expression.nesting),
    }
}

fn resource_error(span: Span) -> ParseError {
    ParseError {
        code: "UBI0090".to_owned(),
        message: "Compiler resource limit exceeded".to_owned(),
        primary: span,
    }
}
