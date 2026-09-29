// Compiler library; source loading stays independent of filesystem access.

mod analyzer;
mod diagnostics;
mod interpreter;
mod ir;
mod js_backend;
mod lexer;
mod parser;
mod source;
mod span;

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod lexer_tests;

#[cfg(test)]
#[path = "source_tests.rs"]
mod source_tests;

#[cfg(test)]
#[path = "parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod analysis_tests;

#[cfg(test)]
#[path = "ir_tests.rs"]
mod ir_tests;

#[cfg(test)]
#[path = "interpreter_tests.rs"]
mod interpreter_tests;

#[cfg(test)]
#[path = "js_backend_tests.rs"]
mod js_backend_tests;
