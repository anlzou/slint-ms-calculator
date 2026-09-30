use std::f64::consts::PI;

use super::lexer::Op;
use super::parser::{parse_normalized, Expr};
use super::value::{snap_value, Rational, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AngleMode {
    #[default]
    Deg,
    Rad,
}

impl AngleMode {
    fn to_radians(self, f: f64) -> f64 {
        match self {
            AngleMode::Deg => f * PI / 180.0,
            AngleMode::Rad => f,
        }
    }

    fn from_radians(self, f: f64) -> f64 {
        match self {
            AngleMode::Deg => f * 180.0 / PI,
            AngleMode::Rad => f,
        }
    }
}

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
                Expr::Percent(ref inner) if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div) => {
                    let iv = eval(inner)?;
                    match op {
                        Op::Add | Op::Sub => l.mul(iv).percent(),
                        _ => iv.percent(),
                    }
                }
                _ => eval(rhs)?,
            };
            if matches!(op, Op::Div | Op::Mod) && r.is_zero() {
                return Err(CalcError::DivideByZero);
            }
            if matches!(op, Op::Root) && l.is_zero() {
                return Err(CalcError::DivideByZero);
            }
            Ok(match op {
                Op::Add => l.add(r),
                Op::Sub => l.sub(r),
                Op::Mul => l.mul(r),
                Op::Div => l.div(r),
                Op::Pow => pow_values(l, r)?,
                // "3 ʸ√x 8"：l=次数，r=被开方数
                Op::Root => root_value(l, r)?,
                Op::Mod => mod_values(l, r)?,
                Op::Exp => exp_value(l, r)?,
            })
        }
    }
}

/// NaN → 输入无效，±inf → 超出范围，其余取整快照。
pub fn classify_f64(f: f64) -> Result<Value, CalcError> {
    if f.is_nan() {
        Err(CalcError::InvalidInput)
    } else if f.is_infinite() {
        Err(CalcError::InputOutOfRange)
    } else {
        Ok(snap_value(f))
    }
}

fn pow_values(base: Value, exp: Value) -> Result<Value, CalcError> {
    if let Some(e) = exp.integer_value() {
        if let Some(v) = base.pow_i(e) {
            return Ok(v);
        }
    }
    classify_f64(base.to_f64().powf(exp.to_f64()))
}

/// E 记法：m × 10^e（"2 exp 3" = 2E3 = 2000）
fn exp_value(m: Value, e: Value) -> Result<Value, CalcError> {
    let ten_pow = pow_values(Value::from_int(10), e)?;
    Ok(m.mul(ten_pow))
}

fn root_value(deg: Value, radicand: Value) -> Result<Value, CalcError> {
    let rf = radicand.to_f64();
    if rf < 0.0 {
        // 负数只允许奇数整数次根
        return match deg.integer_value() {
            Some(n) if n % 2 != 0 => classify_f64(-(-rf).powf(1.0 / n as f64)),
            _ => Err(CalcError::InvalidInput),
        };
    }
    classify_f64(rf.powf(1.0 / deg.to_f64()))
}

fn mod_values(l: Value, r: Value) -> Result<Value, CalcError> {
    if let (Some(a), Some(b)) = (l.integer_value(), r.integer_value()) {
        return match a.checked_rem(b) {
            Some(v) => Ok(Value::from_int(v)),
            None => Err(CalcError::InputOutOfRange),
        };
    }
    Ok(Value::Approx(l.to_f64() % r.to_f64()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Reciprocal,
    Square,
    SquareRoot,
    Cube,
    Cbrt,
    TenPow,
    EPow,
    TwoPow,
    Ln,
    Log,
    Factorial,
    Abs,
    Sin,
    Cos,
    Tan,
    Sec,
    Csc,
    Cot,
    Asin,
    Acos,
    Atan,
    Asec,
    Acsc,
    Acot,
    Sinh,
    Cosh,
    Tanh,
    Sech,
    Csch,
    Coth,
    Asinh,
    Acosh,
    Atanh,
    Asech,
    Acsch,
    Acoth,
    Floor,
    Ceil,
}

impl UnaryOp {
    /// 回填展示文本用的函数名（如 sqrt(9)）
    pub fn display_name(self) -> &'static str {
        match self {
            UnaryOp::Negate => "-",
            UnaryOp::Reciprocal => "1/",
            UnaryOp::Square => "sqr",
            UnaryOp::SquareRoot => "sqrt",
            UnaryOp::Cube => "cube",
            UnaryOp::Cbrt => "cbrt",
            UnaryOp::TenPow => "10^",
            UnaryOp::EPow => "e^",
            UnaryOp::TwoPow => "2^",
            UnaryOp::Ln => "ln",
            UnaryOp::Log => "log",
            UnaryOp::Factorial => "n!",
            UnaryOp::Abs => "abs",
            UnaryOp::Sin => "sin",
            UnaryOp::Cos => "cos",
            UnaryOp::Tan => "tan",
            UnaryOp::Sec => "sec",
            UnaryOp::Csc => "csc",
            UnaryOp::Cot => "cot",
            UnaryOp::Asin => "sin⁻¹",
            UnaryOp::Acos => "cos⁻¹",
            UnaryOp::Atan => "tan⁻¹",
            UnaryOp::Asec => "sec⁻¹",
            UnaryOp::Acsc => "csc⁻¹",
            UnaryOp::Acot => "cot⁻¹",
            UnaryOp::Sinh => "sinh",
            UnaryOp::Cosh => "cosh",
            UnaryOp::Tanh => "tanh",
            UnaryOp::Sech => "sech",
            UnaryOp::Csch => "csch",
            UnaryOp::Coth => "coth",
            UnaryOp::Asinh => "sinh⁻¹",
            UnaryOp::Acosh => "cosh⁻¹",
            UnaryOp::Atanh => "tanh⁻¹",
            UnaryOp::Asech => "sech⁻¹",
            UnaryOp::Acsch => "csch⁻¹",
            UnaryOp::Acoth => "coth⁻¹",
            UnaryOp::Floor => "floor",
            UnaryOp::Ceil => "ceil",
        }
    }
}

/// 默认角度制（同 Windows 计算器初始状态）。
pub fn apply_unary(op: UnaryOp, v: Value) -> Result<Value, CalcError> {
    apply_unary_in(AngleMode::Deg, op, v)
}

pub fn apply_unary_in(angle: AngleMode, op: UnaryOp, v: Value) -> Result<Value, CalcError> {
    let f = v.to_f64();
    match op {
        UnaryOp::Negate => Ok(v.neg()),
        UnaryOp::Square => Ok(v.mul(v)),
        UnaryOp::Cube => Ok(v.mul(v).mul(v)),
        UnaryOp::Abs => Ok(v.abs()),
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
        UnaryOp::Cbrt => Ok(snap_value(f.cbrt())),
        UnaryOp::TenPow => Ok(pow_const(10, f, v)),
        UnaryOp::TwoPow => Ok(pow_const(2, f, v)),
        UnaryOp::EPow => classify_f64(f.exp()),
        UnaryOp::Ln if f <= 0.0 => Err(CalcError::InvalidInput),
        UnaryOp::Log if f <= 0.0 => Err(CalcError::InvalidInput),
        UnaryOp::Ln => classify_f64(f.ln()),
        UnaryOp::Log => classify_f64(f.log10()),
        UnaryOp::Factorial => {
            let n = match v.integer_value() {
                Some(n) if n >= 0 => n,
                Some(_) => return Err(CalcError::InvalidInput),
                None => return Err(CalcError::InvalidInput),
            };
            if n > 170 {
                return Err(CalcError::InputOutOfRange);
            }
            let mut acc = Value::from_int(1);
            for k in 2..=n {
                acc = acc.mul(Value::from_int(k));
            }
            Ok(acc)
        }
        UnaryOp::Sin => classify_f64(angle.to_radians(f).sin()),
        UnaryOp::Cos => classify_f64(angle.to_radians(f).cos()),
        UnaryOp::Tan => classify_f64(angle.to_radians(f).tan()),
        UnaryOp::Sec => classify_f64(1.0 / angle.to_radians(f).cos()),
        UnaryOp::Csc => classify_f64(1.0 / angle.to_radians(f).sin()),
        UnaryOp::Cot => classify_f64(1.0 / angle.to_radians(f).tan()),
        UnaryOp::Asin if f < -1.0 || f > 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Acos if f < -1.0 || f > 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Asin => classify_f64(angle.from_radians(f.asin())),
        UnaryOp::Acos => classify_f64(angle.from_radians(f.acos())),
        UnaryOp::Atan => classify_f64(angle.from_radians(f.atan())),
        UnaryOp::Asec if f.abs() < 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Acsc if f.abs() < 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Asec => classify_f64(angle.from_radians((1.0 / f).acos())),
        UnaryOp::Acsc => classify_f64(angle.from_radians((1.0 / f).asin())),
        UnaryOp::Acot => classify_f64(angle.from_radians(PI / 2.0 - f.atan())),
        // 双曲族：与角度制无关
        UnaryOp::Sinh => classify_f64(f.sinh()),
        UnaryOp::Cosh => classify_f64(f.cosh()),
        UnaryOp::Tanh => classify_f64(f.tanh()),
        UnaryOp::Sech => classify_f64(1.0 / f.cosh()),
        UnaryOp::Csch => classify_f64(1.0 / f.sinh()),
        UnaryOp::Coth => classify_f64(1.0 / f.tanh()),
        UnaryOp::Asinh => classify_f64(f.asinh()),
        UnaryOp::Acosh if f < 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Atanh if f <= -1.0 || f >= 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Acosh => classify_f64(f.acosh()),
        UnaryOp::Atanh => classify_f64(f.atanh()),
        UnaryOp::Asech if f <= 0.0 || f > 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Acsch if f == 0.0 => Err(CalcError::InvalidInput),
        UnaryOp::Acoth if f.abs() <= 1.0 => Err(CalcError::InvalidInput),
        UnaryOp::Asech | UnaryOp::Acsch => classify_f64((1.0 / f).asinh()),
        UnaryOp::Acoth => classify_f64((1.0 / f).atanh()),
        UnaryOp::Floor => classify_f64(f.floor()),
        UnaryOp::Ceil => classify_f64(f.ceil()),
    }
}

/// 10^x / 2^x：整数指数优先精确，否则降级 powf。
fn pow_const(base: i128, f_exp: f64, v: Value) -> Value {
    if let Some(e) = v.integer_value() {
        if let Some(r) = Rational::from_int(base).pow_i(e) {
            return Value::Exact(r);
        }
    }
    snap_value((base as f64).powf(f_exp))
}
