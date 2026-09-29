use std::char;

pub(crate) use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier(String),
    Keyword(String),
    Integer(String),
    Float(String),
    String(String),
    Symbol(Symbol),
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Symbol {
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Ellipsis,
    Underscore,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    ThinArrow,
    FatArrow,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) kind: CommentKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lexed {
    pub(crate) tokens: Vec<Token>,
    pub(crate) comments: Vec<Comment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LexError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) primary: Span,
}

const KEYWORDS: &[&str] = &[
    "import",
    "from",
    "export",
    "fn",
    "record",
    "enum",
    "let",
    "mut",
    "return",
    "if",
    "else",
    "match",
    "true",
    "false",
    "int",
    "float",
    "bool",
    "string",
    "unit",
    "async",
    "await",
    "throw",
    "try",
    "catch",
    "class",
    "while",
    "for",
    "break",
    "continue",
    "new",
    "null",
    "undefined",
    "any",
    "as",
];

pub(crate) fn lex(source_id: &str, source: &str) -> Result<Lexed, LexError> {
    Lexer {
        source_id,
        source,
        offset: 0,
        tokens: Vec::new(),
        comments: Vec::new(),
    }
    .scan()
}

struct Lexer<'a> {
    source_id: &'a str,
    source: &'a str,
    offset: usize,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
}

impl Lexer<'_> {
    fn scan(mut self) -> Result<Lexed, LexError> {
        while let Some(current) = self.peek() {
            if is_whitespace(current) {
                self.advance();
            } else if self.starts_with("//") {
                self.line_comment();
            } else if self.starts_with("/*") {
                self.block_comment()?;
            } else if current == '"' {
                self.string()?;
            } else if current.is_ascii_digit() {
                self.number()?;
            } else if is_identifier_start(current) {
                self.identifier();
            } else if let Some((spelling, symbol)) = self.symbol() {
                let start = self.offset;
                self.advance_n(spelling.len());
                self.push(TokenKind::Symbol(symbol), start);
            } else {
                let start = self.offset;
                self.advance();
                return Err(self.error(start, self.offset, "Invalid source character"));
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span(self.offset, self.offset),
        });
        Ok(Lexed {
            tokens: self.tokens,
            comments: self.comments,
        })
    }

    fn identifier(&mut self) {
        let start = self.offset;
        self.advance();
        while self.peek().is_some_and(is_identifier_continue) {
            self.advance();
        }
        let word = &self.source[start..self.offset];
        let kind = if word == "_" {
            TokenKind::Symbol(Symbol::Underscore)
        } else if KEYWORDS.contains(&word) {
            TokenKind::Keyword(word.to_owned())
        } else {
            TokenKind::Identifier(word.to_owned())
        };
        self.push(kind, start);
    }

    fn number(&mut self) -> Result<(), LexError> {
        let start = self.offset;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance();
        }

        let mut is_float = false;
        if self.peek() == Some('.')
            && self
                .peek_after_current()
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            is_float = true;
            self.advance();
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.advance();
            }
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            is_float = true;
            let exponent_start = self.offset;
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            let digits_start = self.offset;
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.advance();
            }
            if self.offset == digits_start {
                return Err(self.error(
                    exponent_start,
                    self.offset,
                    "Float exponent requires at least one digit",
                ));
            }
        }

        let spelling = self.source[start..self.offset].to_owned();
        self.push(
            if is_float {
                TokenKind::Float(spelling)
            } else {
                TokenKind::Integer(spelling)
            },
            start,
        );
        Ok(())
    }

    fn string(&mut self) -> Result<(), LexError> {
        let start = self.offset;
        self.advance();
        let mut value = String::new();

        while let Some(current) = self.peek() {
            match current {
                '"' => {
                    self.advance();
                    self.push(TokenKind::String(value), start);
                    return Ok(());
                }
                '\r' | '\n' => {
                    let error_start = self.offset;
                    self.advance();
                    return Err(self.error(error_start, self.offset, "Raw newline in string"));
                }
                '\\' => self.escape(&mut value, start)?,
                other => {
                    self.advance();
                    value.push(other);
                }
            }
        }

        Err(self.error(start, self.offset, "Unterminated string"))
    }

    fn escape(&mut self, value: &mut String, string_start: usize) -> Result<(), LexError> {
        let escape_start = self.offset;
        self.advance();
        match self.peek() {
            Some('"') => {
                self.advance();
                value.push('"');
            }
            Some('\\') => {
                self.advance();
                value.push('\\');
            }
            Some('n') => {
                self.advance();
                value.push('\n');
            }
            Some('r') => {
                self.advance();
                value.push('\r');
            }
            Some('t') => {
                self.advance();
                value.push('\t');
            }
            Some('u') => {
                self.advance();
                if self.peek() != Some('{') {
                    return Err(self.error(escape_start, self.offset, "Expected '{' after \\u"));
                }
                self.advance();
                let digits_start = self.offset;
                while self.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                    self.advance();
                }
                let digits_end = self.offset;
                let digit_count = digits_end - digits_start;
                if self.peek() != Some('}') {
                    if self
                        .peek()
                        .is_some_and(|ch| !matches!(ch, '\r' | '\n' | '"'))
                    {
                        self.advance();
                    }
                    return Err(self.error(escape_start, self.offset, "Malformed Unicode escape"));
                }
                self.advance();
                if !(1..=6).contains(&digit_count) {
                    return Err(self.error(
                        escape_start,
                        self.offset,
                        "Invalid Unicode escape length",
                    ));
                }
                let scalar = u32::from_str_radix(&self.source[digits_start..digits_end], 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| {
                        self.error(escape_start, self.offset, "Escape is not a Unicode scalar")
                    })?;
                value.push(scalar);
            }
            Some(_) => {
                self.advance();
                return Err(self.error(escape_start, self.offset, "Unknown string escape"));
            }
            None => return Err(self.error(string_start, self.offset, "Unterminated string")),
        }
        Ok(())
    }

    fn line_comment(&mut self) {
        let start = self.offset;
        self.advance_n(2);
        while self.peek().is_some_and(|ch| !matches!(ch, '\r' | '\n')) {
            self.advance();
        }
        self.comments.push(Comment {
            kind: CommentKind::Line,
            span: self.span(start, self.offset),
        });
    }

    fn block_comment(&mut self) -> Result<(), LexError> {
        let start = self.offset;
        self.advance_n(2);
        while self.peek().is_some() {
            if self.starts_with("*/") {
                self.advance_n(2);
                self.comments.push(Comment {
                    kind: CommentKind::Block,
                    span: self.span(start, self.offset),
                });
                return Ok(());
            }
            self.advance();
        }
        Err(self.error(start, self.offset, "Unterminated block comment"))
    }

    fn symbol(&self) -> Option<(&'static str, Symbol)> {
        const SYMBOLS: &[(&str, Symbol)] = &[
            ("...", Symbol::Ellipsis),
            ("->", Symbol::ThinArrow),
            ("=>", Symbol::FatArrow),
            ("==", Symbol::EqualEqual),
            ("!=", Symbol::BangEqual),
            ("<=", Symbol::LessEqual),
            (">=", Symbol::GreaterEqual),
            ("&&", Symbol::AndAnd),
            ("||", Symbol::OrOr),
            ("(", Symbol::LParen),
            (")", Symbol::RParen),
            ("{", Symbol::LBrace),
            ("}", Symbol::RBrace),
            ("[", Symbol::LBracket),
            ("]", Symbol::RBracket),
            (",", Symbol::Comma),
            (":", Symbol::Colon),
            (";", Symbol::Semicolon),
            (".", Symbol::Dot),
            ("+", Symbol::Plus),
            ("-", Symbol::Minus),
            ("*", Symbol::Star),
            ("/", Symbol::Slash),
            ("%", Symbol::Percent),
            ("!", Symbol::Bang),
            ("=", Symbol::Equal),
            ("<", Symbol::Less),
            (">", Symbol::Greater),
            ("?", Symbol::Question),
            ("_", Symbol::Underscore),
        ];
        SYMBOLS
            .iter()
            .find(|(spelling, _)| self.starts_with(spelling))
            .map(|(spelling, symbol)| (*spelling, *symbol))
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, self.offset),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            source_id: self.source_id.to_owned(),
            start,
            end,
        }
    }

    fn error(&self, start: usize, end: usize, message: &str) -> LexError {
        LexError {
            code: "UBI0001".to_owned(),
            message: message.to_owned(),
            primary: self.span(start, end),
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.offset..)?.chars().next()
    }

    fn peek_after_current(&self) -> Option<char> {
        let current = self.peek()?;
        self.source
            .get(self.offset + current.len_utf8()..)?
            .chars()
            .next()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.offset += ch.len_utf8();
        }
    }

    fn advance_n(&mut self, bytes: usize) {
        debug_assert!(self.source.is_char_boundary(self.offset + bytes));
        self.offset += bytes;
    }

    fn starts_with(&self, text: &str) -> bool {
        self.source[self.offset..].starts_with(text)
    }
}

fn is_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | ' ')
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
