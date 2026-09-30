use slint_ms_calculator::engine::{
    apply_unary, apply_unary_in, error_message, evaluate, expression_to_display, format,
    tokenize, value_from_str, value_to_display, AngleMode, CalcError, Op, Tok, UnaryOp, Value,
};

fn ev(expr: &str) -> Result<Value, CalcError> {
    evaluate(&tokenize(expr).unwrap())
}

fn disp(expr: &str) -> String {
    value_to_display(&ev(expr).unwrap())
}

#[test]
fn operator_precedence() {
    assert_eq!(disp("2+3*4"), "14");
    assert_eq!(disp("2*3+4*5"), "26");
    assert_eq!(disp("1+2*3-4"), "3");
    assert_eq!(disp("8/4+1"), "3");
}

#[test]
fn rational_precision() {
    assert_eq!(disp("0.1+0.2"), "0.3");
    assert_eq!(disp("1/3+1/6"), "0.5");
    assert_eq!(disp("0.3-0.1"), "0.2");
}

#[test]
fn context_sensitive_percent() {
    assert_eq!(disp("50+10%"), "55"); // 50 + 50*10%
    assert_eq!(disp("5*20%"), "1"); // 5 * 0.2
    assert_eq!(disp("100%"), "1"); // 独立百分号
    assert_eq!(disp("50%+10"), "10.5");
}

#[test]
fn divide_by_zero() {
    assert_eq!(ev("5/0"), Err(CalcError::DivideByZero));
    assert_eq!(error_message(CalcError::DivideByZero), "除数不能为零。");
}

#[test]
fn dangling_operators_normalized() {
    assert_eq!(disp("2+"), "2"); // 尾部悬空运算符按 = 时丢弃
    assert_eq!(disp("2++3"), "5");
    assert_eq!(disp("2+-3"), "-1");
    assert_eq!(disp("2*/3"), "0.6666666666666666"); // 保留后者 2/3
}

#[test]
fn unary_minus_and_parens() {
    assert_eq!(disp("-5+2"), "-3");
    assert_eq!(disp("2*(3+4)"), "14");
    assert_eq!(disp("2*(3+4"), "14"); // 缺右括号自动闭合，同原版
}

#[test]
fn unary_operations() {
    assert_eq!(apply_unary(UnaryOp::Square, Value::from_int(2)), Ok(Value::from_int(4)));
    assert_eq!(
        apply_unary(UnaryOp::SquareRoot, Value::from_int(9)),
        Ok(Value::from_int(3))
    );
    assert_eq!(
        value_to_display(&apply_unary(UnaryOp::Reciprocal, Value::from_int(4)).unwrap()),
        "0.25"
    );
    assert_eq!(
        apply_unary(UnaryOp::Reciprocal, Value::from_int(0)),
        Err(CalcError::DivideByZero)
    );
    assert_eq!(
        apply_unary(UnaryOp::SquareRoot, Value::from_int(-4)),
        Err(CalcError::InvalidInput)
    );
    let s2 = apply_unary(UnaryOp::SquareRoot, Value::from_int(2)).unwrap();
    assert_eq!(value_to_display(&s2), "1.4142135623730951");
}

#[test]
fn display_formatting() {
    assert_eq!(disp("1234567"), "1,234,567");
    assert_eq!(disp("1234567*7"), "8,641,969");
    assert_eq!(disp("999999999*999999999"), "9.99999998E+17"); // 超过 1e16 转科学计数法
    assert_eq!(disp("10000000000000000+0"), "1E+16"); // 超过 1e16 转科学计数法
    assert_eq!(format::group_digits("-1234"), "-1,234");
    assert_eq!(disp("25.5+0"), "25.5");
}

#[test]
fn expression_line_display() {
    assert_eq!(expression_to_display(&tokenize("2+3*4").unwrap()), "2 + 3 × 4");
    assert_eq!(expression_to_display(&tokenize("1234567/2").unwrap()), "1,234,567 ÷ 2");
    assert_eq!(expression_to_display(&tokenize("-5+2").unwrap()), "-5 + 2");
    assert_eq!(expression_to_display(&tokenize("50+10%").unwrap()), "50 + 10%");
}

#[test]
fn large_numbers_promote_to_f64() {
    // i128 范围内的连乘保持精确；触发降级后仍给出有限结果
    let v = ev("999999999*999999999*999999999").unwrap();
    assert!(v.to_f64().is_finite());
}

#[test]
fn num_text_token_displays_and_evals() {
    // sqr(2) 型 token：展示用文本，求值用即时值
    let toks = vec![
        Tok::NumText(Value::from_int(4), "sqr(2)".into()),
        Tok::Bin(Op::Add),
        Tok::Num(Value::from_int(1)),
    ];
    assert_eq!(value_to_display(&evaluate(&toks).unwrap()), "5");
    assert_eq!(expression_to_display(&toks), "sqr(2) + 1");
}

#[test]
fn token_helpers_roundtrip() {
    let toks = tokenize("2 * (3.5 - 1) / 4 %")
        .unwrap()
        .into_iter()
        .filter(|t| !matches!(t, Tok::Percent))
        .collect::<Vec<_>>();
    assert_eq!(toks.len(), 9);
    assert_eq!(toks[4], Tok::Bin(Op::Sub));
}

#[test]
fn power_operator() {
    assert_eq!(disp("2^10"), "1,024");
    assert_eq!(disp("2^3^2"), "512"); // 右结合：2^(3^2)
    assert_eq!(disp("-2^2"), "-4"); // 一元负号优先级低于 ^
    assert_eq!(disp("2^-3"), "0.125");
    assert_eq!(disp("2^0"), "1");
    assert_eq!(disp("0^0"), "1");
    assert_eq!(disp("4^0.5"), "2");
    assert_eq!(disp("2^"), "2"); // 尾部悬空 ^ 按 = 时丢弃
    assert_eq!(
        expression_to_display(&tokenize("2^10").unwrap()),
        "2 ^ 10"
    );
}

fn bin_eval(a: i128, op: Op, b: &str) -> Result<Value, CalcError> {
    let toks = vec![
        Tok::Num(Value::from_int(a)),
        Tok::Bin(op),
        Tok::Num(value_from_str(b)),
    ];
    evaluate(&toks)
}

#[test]
fn root_operator() {
    assert_eq!(value_to_display(&bin_eval(3, Op::Root, "8").unwrap()), "2");
    assert_eq!(value_to_display(&bin_eval(2, Op::Root, "9").unwrap()), "3");
    assert_eq!(value_to_display(&bin_eval(3, Op::Root, "-8").unwrap()), "-2");
    assert_eq!(bin_eval(2, Op::Root, "-4"), Err(CalcError::InvalidInput)); // 负数偶次根
    assert_eq!(bin_eval(0, Op::Root, "8"), Err(CalcError::DivideByZero));
}

#[test]
fn mod_operator() {
    assert_eq!(value_to_display(&bin_eval(5, Op::Mod, "2").unwrap()), "1");
    assert_eq!(
        value_to_display(&bin_eval(-5, Op::Mod, "2").unwrap()),
        "-1"
    ); // 符号跟随被除数
    assert_eq!(bin_eval(5, Op::Mod, "0"), Err(CalcError::DivideByZero));
}

fn trig(angle: AngleMode, op: UnaryOp, x: &str) -> Result<Value, CalcError> {
    apply_unary_in(angle, op, value_from_str(x))
}

#[test]
fn trig_deg_rad() {
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Sin, "30").unwrap()), "0.5");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Cos, "60").unwrap()), "0.5");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Tan, "45").unwrap()), "1");
    assert_eq!(
        value_to_display(&trig(AngleMode::Deg, UnaryOp::Sin, "180").unwrap()),
        "0"
    ); // 快照消去 1.2e-16 浮点尾巴
    assert_eq!(value_to_display(&trig(AngleMode::Rad, UnaryOp::Cos, "0").unwrap()), "1");
    assert_eq!(
        value_to_display(&trig(AngleMode::Deg, UnaryOp::Asin, "0.5").unwrap()),
        "30"
    );
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Asin, "2"), Err(CalcError::InvalidInput));
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Acos, "-2"), Err(CalcError::InvalidInput));
}

#[test]
fn logs_and_exponentials() {
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Log, "100").unwrap()), "2");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Log, "1000").unwrap()), "3");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Ln, "1").unwrap()), "0");
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Ln, "0"), Err(CalcError::InvalidInput));
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Log, "-5"), Err(CalcError::InvalidInput));
    assert_eq!(
        value_to_display(&apply_unary(UnaryOp::TenPow, Value::from_int(3)).unwrap()),
        "1,000"
    );
    assert_eq!(
        value_to_display(&apply_unary(UnaryOp::TwoPow, Value::from_int(10)).unwrap()),
        "1,024"
    );
    assert_eq!(
        value_to_display(&apply_unary(UnaryOp::TenPow, Value::from_int(-2)).unwrap()),
        "0.01"
    );
}

#[test]
fn factorial_abs_cbrt() {
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Factorial, Value::from_int(5)).unwrap()), "120");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Factorial, Value::from_int(0)).unwrap()), "1");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Factorial, Value::from_int(10)).unwrap()), "3,628,800");
    assert_eq!(
        apply_unary(UnaryOp::Factorial, Value::from_int(171)),
        Err(CalcError::InputOutOfRange)
    );
    assert_eq!(
        apply_unary(UnaryOp::Factorial, Value::from_int(-3)),
        Err(CalcError::InvalidInput)
    );
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Abs, Value::from_int(-7)).unwrap()), "7");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Cbrt, Value::from_int(27)).unwrap()), "3");
    assert_eq!(
        apply_unary(UnaryOp::Cbrt, Value::from_int(-27))
            .map(|v| value_to_display(&v)),
        Ok("-3".into())
    );
    assert_eq!(
        value_to_display(&apply_unary(UnaryOp::Cube, Value::from_int(3)).unwrap()),
        "27"
    );
}

#[test]
fn sec_csc_cot_and_inverses() {
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Sec, "60").unwrap()), "2");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Csc, "30").unwrap()), "2");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Cot, "45").unwrap()), "1");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Asec, "2").unwrap()), "60");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Acsc, "2").unwrap()), "30");
    assert_eq!(value_to_display(&trig(AngleMode::Deg, UnaryOp::Acot, "1").unwrap()), "45");
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Asec, "0.5"), Err(CalcError::InvalidInput));
    assert_eq!(trig(AngleMode::Deg, UnaryOp::Acsc, "0.5"), Err(CalcError::InvalidInput));
}

#[test]
fn hyperbolic_family() {
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Sinh, Value::from_int(0)).unwrap()), "0");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Cosh, Value::from_int(0)).unwrap()), "1");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Tanh, Value::from_int(0)).unwrap()), "0");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Sech, Value::from_int(0)).unwrap()), "1");
    assert_eq!(apply_unary(UnaryOp::Csch, Value::from_int(0)), Err(CalcError::InputOutOfRange)); // 1/sinh(0)=∞
    assert_eq!(apply_unary(UnaryOp::Coth, Value::from_int(0)), Err(CalcError::InputOutOfRange));
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Acosh, Value::from_int(1)).unwrap()), "0");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Atanh, Value::from_int(0)).unwrap()), "0");
    assert_eq!(apply_unary(UnaryOp::Acosh, Value::from_int(0)), Err(CalcError::InvalidInput));
    assert_eq!(apply_unary(UnaryOp::Atanh, Value::from_int(1)), Err(CalcError::InvalidInput));
    assert_eq!(apply_unary(UnaryOp::Asech, Value::from_int(2)), Err(CalcError::InvalidInput));
    assert_eq!(apply_unary(UnaryOp::Acsch, Value::from_int(0)), Err(CalcError::InvalidInput));
    assert_eq!(apply_unary(UnaryOp::Acoth, value_from_str("0.5")), Err(CalcError::InvalidInput)); // |x|<=1
}

#[test]
fn floor_ceil() {
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Floor, value_from_str("2.7")).unwrap()), "2");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Floor, value_from_str("-2.1")).unwrap()), "-3");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Ceil, value_from_str("2.1")).unwrap()), "3");
    assert_eq!(value_to_display(&apply_unary(UnaryOp::Ceil, value_from_str("-2.7")).unwrap()), "-2");
}
