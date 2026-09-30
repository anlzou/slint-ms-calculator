use super::value::{value_from_str, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    /// x 的 y 次幂
    Pow,
    /// lhs 次方根 of rhs（"3 ʸ√x 8" = 2）
    Root,
    /// 取余（截断语义，符号跟随被除数）
    Mod,
    /// 科学记数法 E 记法：lhs × 10^rhs（"2 exp 3" = 2E3 = 2000）
    Exp,
}

impl Op {
    pub fn display(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "×",
            Op::Div => "÷",
            Op::Pow => "^",
            Op::Root => "ʸ√x",
            Op::Mod => "mod",
            Op::Exp => "E",
        }
    }

    /// 幂/根/exp：优先级高于乘除、右结合
    pub fn is_power(self) -> bool {
        matches!(self, Op::Pow | Op::Root | Op::Exp)
    }

    /// 这些运算符之后的 +/- 视作下一操作数的正负号，而非替换运算符
    pub fn takes_signed_operand(self) -> bool {
        matches!(self, Op::Pow | Op::Root | Op::Mod | Op::Exp)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(Value),
    /// 一元运算结果的即时值 + 展示文本（如 sqr(2)），求值时视作数字
    NumText(Value, String),
    Bin(Op),
    Percent,
    LParen,
    RParen,
}

pub fn tokenize(s: &str) -> Result<Vec<Tok>, &'static str> {
    let mut toks = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                let mut has_dot = false;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    if chars[i] == '.' {
                        if has_dot {
                            return Err("数字中包含多个小数点");
                        }
                        has_dot = true;
                    }
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                toks.push(Tok::Num(value_from_str(&text)));
            }
            '+' => {
                toks.push(Tok::Bin(Op::Add));
                i += 1;
            }
            '-' => {
                toks.push(Tok::Bin(Op::Sub));
                i += 1;
            }
            '*' => {
                toks.push(Tok::Bin(Op::Mul));
                i += 1;
            }
            '/' => {
                toks.push(Tok::Bin(Op::Div));
                i += 1;
            }
            '^' => {
                toks.push(Tok::Bin(Op::Pow));
                i += 1;
            }
            '%' => {
                toks.push(Tok::Percent);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            _ => return Err("无法识别的字符"),
        }
    }
    Ok(toks)
}
