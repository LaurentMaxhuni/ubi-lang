use crate::lexer::{lex, Comment, CommentKind, Span, Symbol, Token, TokenKind};

fn token(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span {
            source_id: "main.ubi".to_owned(),
            start,
            end,
        },
    }
}

#[test]
fn tokens_use_utf8_byte_spans_and_eof_is_zero_width() {
    let source = "let x = 12;\n";
    let lexed = lex("main.ubi", source).unwrap();

    assert_eq!(
        lexed.tokens,
        vec![
            token(TokenKind::Keyword("let".into()), 0, 3),
            token(TokenKind::Identifier("x".into()), 4, 5),
            token(TokenKind::Symbol(Symbol::Equal), 6, 7),
            token(TokenKind::Integer("12".into()), 8, 10),
            token(TokenKind::Symbol(Symbol::Semicolon), 10, 11),
            token(TokenKind::Eof, 12, 12),
        ]
    );
}

#[test]
fn reserved_word_prefix_does_not_split_an_identifier() {
    let source = "if iffy";
    let lexed = lex("main.ubi", source).unwrap();

    assert_eq!(
        lexed.tokens,
        vec![
            token(TokenKind::Keyword("if".into()), 0, 2),
            token(TokenKind::Identifier("iffy".into()), 3, 7),
            token(TokenKind::Eof, 7, 7),
        ]
    );
}

#[test]
fn strings_decode_escapes_and_comments_keep_utf8_byte_spans() {
    let source = concat!(r#""é\u{1F642}\"\n" /* λ🦄 */ // hi🦄"#, "\n");
    let lexed = lex("main.ubi", source).unwrap();

    assert_eq!(
        lexed.tokens,
        vec![
            token(TokenKind::String("é🙂\"\n".into()), 0, 17),
            token(TokenKind::Eof, 41, 41),
        ]
    );
    assert_eq!(
        lexed.comments,
        vec![
            Comment {
                kind: CommentKind::Block,
                span: Span {
                    source_id: "main.ubi".into(),
                    start: 18,
                    end: 30,
                },
            },
            Comment {
                kind: CommentKind::Line,
                span: Span {
                    source_id: "main.ubi".into(),
                    start: 31,
                    end: 40,
                },
            },
        ]
    );
}

#[test]
fn line_and_block_comments_are_retained_in_source_order() {
    let source = "a//first\nb/*middle*/c//last";
    let lexed = lex("main.ubi", source).unwrap();

    assert_eq!(
        lexed.tokens,
        vec![
            token(TokenKind::Identifier("a".into()), 0, 1),
            token(TokenKind::Identifier("b".into()), 9, 10),
            token(TokenKind::Identifier("c".into()), 20, 21),
            token(TokenKind::Eof, 27, 27),
        ]
    );
    assert_eq!(
        lexed.comments,
        vec![
            Comment {
                kind: CommentKind::Line,
                span: Span {
                    source_id: "main.ubi".into(),
                    start: 1,
                    end: 8,
                },
            },
            Comment {
                kind: CommentKind::Block,
                span: Span {
                    source_id: "main.ubi".into(),
                    start: 10,
                    end: 20,
                },
            },
            Comment {
                kind: CommentKind::Line,
                span: Span {
                    source_id: "main.ubi".into(),
                    start: 21,
                    end: 27,
                },
            },
        ]
    );
}

#[test]
fn symbols_use_maximal_munch() {
    let source = "....->-=>===!==<<=>>=&&||!+*/%?";
    let expected = [
        (Symbol::Ellipsis, "..."),
        (Symbol::Dot, "."),
        (Symbol::ThinArrow, "->"),
        (Symbol::Minus, "-"),
        (Symbol::FatArrow, "=>"),
        (Symbol::EqualEqual, "=="),
        (Symbol::Equal, "="),
        (Symbol::BangEqual, "!="),
        (Symbol::Equal, "="),
        (Symbol::Less, "<"),
        (Symbol::LessEqual, "<="),
        (Symbol::Greater, ">"),
        (Symbol::GreaterEqual, ">="),
        (Symbol::AndAnd, "&&"),
        (Symbol::OrOr, "||"),
        (Symbol::Bang, "!"),
        (Symbol::Plus, "+"),
        (Symbol::Star, "*"),
        (Symbol::Slash, "/"),
        (Symbol::Percent, "%"),
        (Symbol::Question, "?"),
    ];
    let lexed = lex("main.ubi", source).unwrap();

    assert_eq!(lexed.tokens.len(), expected.len() + 1);
    let mut offset = 0;
    for (actual, (symbol, spelling)) in lexed.tokens.iter().zip(expected) {
        assert_eq!(actual.kind, TokenKind::Symbol(symbol));
        assert_eq!(
            (actual.span.start, actual.span.end),
            (offset, offset + spelling.len())
        );
        assert_eq!(&source[actual.span.start..actual.span.end], spelling);
        offset += spelling.len();
    }
    assert_eq!(lexed.tokens.last(), Some(&token(TokenKind::Eof, 31, 31)));
}

fn assert_lex_error(source: &str, start: usize, end: usize) {
    let error = lex("main.ubi", source).unwrap_err();
    assert_eq!(error.code, "UBI0001");
    assert!(!error.message.is_empty());
    assert_eq!(
        error.primary,
        Span {
            source_id: "main.ubi".into(),
            start,
            end,
        }
    );
}

#[test]
fn malformed_strings_and_comments_report_lexical_errors() {
    // The bad escape bytes are the offending token.
    assert_lex_error(r#""a\qz""#, 2, 4);
    // Unterminated lexical items span from their opening delimiter through EOF.
    assert_lex_error("\"open", 0, 5);
    assert_lex_error("x /* open", 2, 9);
}
