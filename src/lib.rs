// Compiler library; source loading stays independent of filesystem access.

mod lexer;
mod source;
mod span;

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod lexer_tests;

#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;
