pub mod eval;
pub mod format;
pub mod lexer;
pub mod memory;
pub mod parser;
pub mod value;

pub use eval::{apply_unary, apply_unary_in, evaluate, AngleMode, CalcError, UnaryOp};
pub use format::{error_message, expression_to_display, value_to_display};
pub use lexer::{tokenize, Op, Tok};
pub use memory::Memory;
pub use value::{snap_value, value_from_str, value_rand, Rational, Value};

pub fn demo_snapshot() {
    let toks = tokenize("2+3*4").unwrap();
    let result = evaluate(&toks).unwrap();
    println!(
        "{} = {}",
        expression_to_display(&toks),
        value_to_display(&result)
    );
}
