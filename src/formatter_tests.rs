//! Independent SPEC section 22 token, comment, and idempotence expectations.
use crate::formatter::format_source;
use crate::lexer::lex;

fn spellings(source: &str) -> Vec<String> {
    let lexed = lex("main.ubi", source).unwrap();
    let mut spans: Vec<_> = lexed
        .tokens
        .iter()
        .map(|t| &t.span)
        .chain(lexed.comments.iter().map(|c| &c.span))
        .collect();
    spans.sort_by_key(|s| s.start);
    spans
        .into_iter()
        .filter(|s| s.start != s.end)
        .map(|s| source[s.start..s.end].to_owned())
        .collect()
}

#[test]
fn format_preserves_spelling_comments_order_and_is_idempotent() {
    for source in [
        "// header😀\nexport fn main( )-> int{/*before*/let n=0010; // value\n n+2/*tail*/}\n// end\n",
        "export fn text()->string{\"a\\u{1F600}\\n\\\"\"}// trailing\n",
        "record Box<T>{value:T} fn id<T>(x:T)->T{x} export fn main()->int{let b:Box<int> =Box{value:id(2)};b.value}",
        "export async fn main()->int{let n=await value();n} async fn value()->int{1}",
        "export fn main()->int{let mut n=0;while(n<3){n=n+1;}n}",
        "record Callbacks{value:List<fn(int,)->fn()->int>} fn make(n:int)->fn(int)->fn()->int{(x:int)=>()=>n+x}",
    ] {
        let formatted=format_source(source).unwrap();
        assert_eq!(spellings(source),spellings(&formatted));
        assert_eq!(format_source(&formatted).unwrap(),formatted);
        assert!(formatted.ends_with('\n'));
    }
}

#[test]
fn invalid_syntax_has_no_formatted_result() {
    for source in [
        "export fn main( {",
        "export fn main() -> string { \"unfinished",
        "export fn main() -> int { 1 + }",
    ] {
        assert!(
            format_source(source).is_err(),
            "invalid source formatted: {source}"
        );
    }
}
