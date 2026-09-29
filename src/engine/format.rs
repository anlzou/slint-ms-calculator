use super::eval::CalcError;
use super::lexer::{Op, Tok};
use super::value::Value;

const MAX_PLAIN_INT: f64 = 1e16;

pub fn group_digits(int_part: &str) -> String {
    let neg = int_part.starts_with('-');
    let digits = int_part.trim_start_matches('-');
    let mut out = String::new();
    for (idx, c) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let grouped: String = out.chars().rev().collect();
    if neg {
        format!("-{grouped}")
    } else {
        grouped
    }
}

fn to_scientific(f: f64) -> String {
    let s = format!("{:.14e}", f);
    let (mant, exp) = s.split_once('e').unwrap();
    let mant = mant.trim_end_matches('0').trim_end_matches('.');
    let exp = exp.trim_start_matches('+');
    let (esign, e_digits) = match exp.split_once('-') {
        Some((_, d)) => ("-", d),
        None => ("+", exp),
    };
    format!("{mant}E{esign}{e_digits:0>2}")
}

fn float_to_display(f: f64) -> String {
    if f == 0.0 {
        return "0".into();
    }
    let a = f.abs();
    if a >= MAX_PLAIN_INT || a < 1e-16 {
        return to_scientific(f);
    }
    if f == f.round() {
        return group_digits(&(f as i128).to_string());
    }
    let s = format!("{f}");
    match s.split_once('.') {
        Some((ip, fp)) => format!("{}.{}", group_digits(ip), fp),
        None => s,
    }
}

pub fn value_to_display(v: &Value) -> String {
    match v {
        Value::Exact(r) => {
            if r.den == 1 {
                let n = r.num;
                if n.abs() < MAX_PLAIN_INT as i128 {
                    return group_digits(&n.to_string());
                }
                float_to_display(r.to_f64())
            } else {
                float_to_display(r.to_f64())
            }
        }
        Value::Approx(f) => float_to_display(*f),
    }
}

/// 公式行展示："2 + 3 × 4"（数字带分组，运算符两侧留空格；行首减号视作负号紧贴）
pub fn expression_to_display(toks: &[Tok]) -> String {
    let mut out = String::new();
    for t in toks {
        match t {
            Tok::Num(v) => {
                if !out.is_empty() && !out.ends_with('(') && !out.ends_with(' ') && out != "-" {
                    out.push(' ');
                }
                out.push_str(&value_to_display(v));
            }
            Tok::NumText(_, text) => {
                if !out.is_empty() && !out.ends_with('(') && !out.ends_with(' ') && out != "-" {
                    out.push(' ');
                }
                out.push_str(text);
            }
            Tok::Bin(op) => {
                if out.is_empty() {
                    if matches!(op, Op::Sub) {
                        out.push('-');
                    }
                } else {
                    out.push(' ');
                    out.push_str(op.display());
                    out.push(' ');
                }
            }
            Tok::Percent => out.push('%'),
            Tok::LParen => {
                if !out.is_empty() && !out.ends_with('(') {
                    out.push(' ');
                }
                out.push('(');
            }
            Tok::RParen => out.push(')'),
        }
    }
    out
}

pub fn error_message(e: CalcError) -> &'static str {
    match e {
        CalcError::DivideByZero => "除数不能为零。",
        CalcError::InputOutOfRange => "输入超出范围。",
        CalcError::InvalidInput => "输入无效。",
        CalcError::InvalidExpression => "输入无效。",
    }
}
