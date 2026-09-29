use super::lexer::Op;
use super::parser::{parse_normalized, Expr};
use super::value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcError {
    DivideByZero,
    InputOutOfRange,
    InvalidInput,
    InvalidExpression,
}

pub fn evaluate(toks: &[super::lexer::Tok]) -> Result<Value, CalcError> {
    let ast = parse_normalized(toks)?;
    eval(&ast)
}

fn eval(e: &Expr) -> Result<Value, CalcError> {
    match e {
        Expr::Num(v) => Ok(*v),
        Expr::Neg(x) => Ok(eval(x)?.neg()),
        Expr::Paren(x) => eval(x),
        Expr::Percent(x) => Ok(eval(x)?.percent()),
        Expr::Bin { op, lhs, rhs } => {
            let l = eval(lhs)?;
            // 上下文百分号：a ± b% = a ± a*b/100；a ×÷ b% = a ×÷ b/100
            let r = match **rhs {
                Expr::Percent(ref inner) => {
                    let iv = eval(inner)?;
                    match op {
                        Op::Add | Op::Sub => l.mul(iv).percent(),
                        Op::Mul | Op::Div => iv.percent(),
                    }
                }
                _ => eval(rhs)?,
            };
            if matches!(op, Op::Div) && r.is_zero() {
                return Err(CalcError::DivideByZero);
            }
            Ok(match op {
                Op::Add => l.add(r),
                Op::Sub => l.sub(r),
                Op::Mul => l.mul(r),
                Op::Div => l.div(r),
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Reciprocal,
    Square,
    SquareRoot,
}

pub fn apply_unary(op: UnaryOp, v: Value) -> Result<Value, CalcError> {
    match op {
        UnaryOp::Negate => Ok(v.neg()),
        UnaryOp::Square => Ok(v.mul(v)),
        UnaryOp::Reciprocal => {
            if v.is_zero() {
                Err(CalcError::DivideByZero)
            } else {
                Ok(Value::from_int(1).div(v))
            }
        }
        UnaryOp::SquareRoot => match v {
            Value::Exact(r) => match r.sqrt_perfect() {
                Some(s) => Ok(Value::Exact(s)),
                None => {
                    if r.num < 0 {
                        Err(CalcError::InvalidInput)
                    } else {
                        Ok(Value::Approx(r.to_f64().sqrt()))
                    }
                }
            },
            Value::Approx(f) => {
                if f < 0.0 {
                    Err(CalcError::InvalidInput)
                } else {
                    Ok(Value::Approx(f.sqrt()))
                }
            }
        },
    }
}
