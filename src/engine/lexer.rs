use super::value::{value_from_str, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    pub fn display(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "×",
            Op::Div => "÷",
        }
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
