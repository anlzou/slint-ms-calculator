use super::eval::CalcError;
use super::lexer::{Op, Tok};
use super::value::Value;

#[derive(Clone, Debug)]
pub enum Expr {
    Num(Value),
    Neg(Box<Expr>),
    Bin { op: Op, lhs: Box<Expr>, rhs: Box<Expr> },
    Percent(Box<Expr>),
    Paren(Box<Expr>),
}

/// 规范化按键序列，贴近原版行为：
/// - 尾部悬空的运算符丢弃（"2+" 按 = 得 2）；操作数后的 % 有意义需保留（"50+10%"）
/// - 连续两个二元运算符时保留后者（"2++3"→"2+3"，"2+-3"→"2-3"）
pub fn normalize(toks: &[Tok]) -> Vec<Tok> {
    let mut toks = toks.to_vec();
    loop {
        match toks.last() {
            Some(Tok::Bin(_) | Tok::LParen) => {
                toks.pop();
            }
            Some(Tok::Percent) => {
                let preceded_by_operand = toks.len() >= 2
                    && matches!(
                        toks[toks.len() - 2],
                        Tok::Num(_) | Tok::NumText(_, _) | Tok::RParen
                    );
                if preceded_by_operand {
                    break;
                }
                toks.pop();
            }
            _ => break,
        }
    }
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    for t in toks {
        if let Tok::Bin(_) = t {
            if matches!(out.last(), Some(Tok::Bin(_))) {
                out.pop();
            }
        }
        out.push(t);
    }
    out
}

struct Parser<'a> {
    toks: &'a [Tok],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Tok> {
        self.toks.get(self.i).cloned()
    }

    fn eat_bin(&mut self, op: Op) -> bool {
        if self.peek() == Some(Tok::Bin(op)) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Expr, CalcError> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Bin(Op::Add)) => Op::Add,
                Some(Tok::Bin(Op::Sub)) => Op::Sub,
                _ => break,
            };
            self.i += 1;
            let rhs = self.term()?;
            lhs = Expr::Bin { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Expr, CalcError> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Bin(Op::Mul)) => Op::Mul,
                Some(Tok::Bin(Op::Div)) => Op::Div,
                _ => break,
            };
            self.i += 1;
            let rhs = self.unary()?;
            lhs = Expr::Bin { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, CalcError> {
        if self.eat_bin(Op::Sub) {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat_bin(Op::Add) {
            return self.unary();
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, CalcError> {
        let mut e = self.primary()?;
        while self.peek() == Some(Tok::Percent) {
            self.i += 1;
            e = Expr::Percent(Box::new(e));
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, CalcError> {
        match self.peek() {
            Some(Tok::Num(v)) | Some(Tok::NumText(v, _)) => {
                self.i += 1;
                Ok(Expr::Num(v))
            }
            Some(Tok::LParen) => {
                self.i += 1;
                let inner = self.expr()?;
                if self.peek() == Some(Tok::RParen) {
                    self.i += 1;
                }
                Ok(Expr::Paren(Box::new(inner)))
            }
            _ => Err(CalcError::InvalidExpression),
        }
    }
}

pub fn parse(toks: &[Tok]) -> Result<Expr, CalcError> {
    if toks.is_empty() {
        return Err(CalcError::InvalidExpression);
    }
    let mut p = Parser { toks, i: 0 };
    let e = p.expr()?;
    if p.i != toks.len() {
        return Err(CalcError::InvalidExpression);
    }
    Ok(e)
}

pub fn parse_normalized(toks: &[Tok]) -> Result<Expr, CalcError> {
    parse(&normalize(toks))
}
