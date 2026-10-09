use crate::diagnostics::Diagnostic;
use crate::lexer::{self, CommentKind, Symbol, TokenKind};
use crate::parser;
use crate::span::Span;

/// Format valid Ubi syntax without rewriting token spellings or comment contents.
pub fn format_source(source: &str) -> Result<String, Diagnostic> {
    parser::parse("format.ubi", source)
        .map_err(|error| Diagnostic::error(&error.code, error.message, error.primary))?;
    let lexed = lexer::lex("format.ubi", source)
        .map_err(|error| Diagnostic::error(&error.code, error.message, error.primary))?;
    let mut items: Vec<_> = lexed
        .tokens
        .into_iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| Item {
            span: token.span,
            kind: Kind::Token(token.kind),
        })
        .chain(lexed.comments.into_iter().map(|comment| Item {
            span: comment.span,
            kind: Kind::Comment(comment.kind),
        }))
        .collect();
    items.sort_by_key(|item| item.span.start);
    let mut output = String::new();
    let mut indent = 0;
    let mut delimiters = Vec::new();
    let mut previous: Option<&TokenKind> = None;
    for (index, item) in items.iter().enumerate() {
        let spelling = &source[item.span.start..item.span.end];
        let next = items.get(index + 1);
        let trailing_comment = next.is_some_and(|next| {
            matches!(next.kind, Kind::Comment(_))
                && !source[item.span.end..next.span.start].contains(['\r', '\n'])
        });
        match &item.kind {
            Kind::Comment(kind) => {
                let standalone = index == 0
                    || source[items[index - 1].span.end..item.span.start].contains(['\r', '\n']);
                if standalone {
                    newline(&mut output);
                }
                append(&mut output, spelling, indent, true);
                if *kind == CommentKind::Line || standalone {
                    newline(&mut output);
                }
                previous = None;
            }
            Kind::Token(kind) => {
                let symbol = match kind {
                    TokenKind::Symbol(symbol) => Some(*symbol),
                    _ => None,
                };
                match symbol {
                    Some(Symbol::LBrace) => {
                        let multiline =
                            !matches!(previous, Some(TokenKind::Keyword(word)) if word == "import");
                        append(&mut output, spelling, indent, true);
                        delimiters.push((Symbol::LBrace, multiline));
                        if multiline {
                            indent += 1;
                            if !trailing_comment
                                && !matches!(
                                    next.map(|item| &item.kind),
                                    Some(Kind::Token(TokenKind::Symbol(Symbol::RBrace)))
                                )
                            {
                                newline(&mut output);
                            }
                        }
                    }
                    Some(Symbol::RBrace) => {
                        let (_, multiline) = delimiters.pop().unwrap();
                        if multiline {
                            indent -= 1;
                            if !matches!(previous, Some(TokenKind::Symbol(Symbol::LBrace))) {
                                newline(&mut output);
                            }
                        }
                        append(&mut output, spelling, indent, !multiline);
                        if !trailing_comment
                            && next.is_some_and(|item| match &item.kind {
                                Kind::Token(TokenKind::Keyword(word)) => {
                                    !matches!(word.as_str(), "else" | "from")
                                }
                                Kind::Token(
                                    TokenKind::Identifier(_)
                                    | TokenKind::String(_)
                                    | TokenKind::Integer(_)
                                    | TokenKind::Float(_),
                                ) => true,
                                _ => false,
                            })
                        {
                            newline(&mut output);
                        }
                    }
                    Some(Symbol::LParen | Symbol::LBracket) => {
                        let spaced = symbol == Some(Symbol::LParen)
                            && matches!(previous, Some(TokenKind::Keyword(_)));
                        append(&mut output, spelling, indent, spaced);
                        delimiters.push((symbol.unwrap(), false));
                    }
                    Some(Symbol::RParen | Symbol::RBracket) => {
                        delimiters.pop();
                        append(&mut output, spelling, indent, false);
                    }
                    Some(Symbol::Semicolon) => {
                        append(&mut output, spelling, indent, false);
                        if !trailing_comment {
                            newline(&mut output);
                        }
                    }
                    Some(Symbol::Comma) => {
                        append(&mut output, spelling, indent, false);
                        if delimiters.last() == Some(&(Symbol::LBrace, true)) && !trailing_comment {
                            newline(&mut output);
                        }
                    }
                    Some(Symbol::Colon | Symbol::Dot | Symbol::Question) => {
                        append(&mut output, spelling, indent, false)
                    }
                    _ => {
                        let spaced = !matches!(
                            previous,
                            Some(TokenKind::Symbol(
                                Symbol::LParen | Symbol::LBracket | Symbol::Dot
                            ))
                        );
                        append(&mut output, spelling, indent, spaced);
                    }
                }
                previous = Some(kind);
            }
        }
    }
    newline(&mut output);
    if output.len() > crate::MAX_SOURCE_BYTES {
        return Err(Diagnostic::error(
            "UBI0090",
            "Formatted source exceeds the source byte limit",
            Span {
                source_id: "format.ubi".to_owned(),
                start: 0,
                end: 0,
            },
        ));
    }
    Ok(output)
}

struct Item {
    span: Span,
    kind: Kind,
}
enum Kind {
    Token(TokenKind),
    Comment(CommentKind),
}

fn append(output: &mut String, spelling: &str, indent: usize, space: bool) {
    if output.is_empty() || output.ends_with('\n') {
        for _ in 0..indent {
            output.push_str("    ");
        }
    } else if space && !output.ends_with([' ', '\n']) {
        output.push(' ');
    }
    output.push_str(spelling);
}

fn newline(output: &mut String) {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}
