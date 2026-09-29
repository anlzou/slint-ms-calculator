use crate::engine::{
    apply_unary, error_message, evaluate, expression_to_display, value_from_str, value_to_display,
    CalcError, Memory, Op, Tok, UnaryOp, Value,
};

/// 一条历史记录：表达式（含末尾 "="）、展示结果、可直接召回的值
#[derive(Clone)]
pub struct HistoryEntry {
    pub expression: String,
    pub result: String,
    pub value: Value,
}

/// M5 控制器：四则/一元/百分号/CE/C/← + 记忆（MS/M+/M-/MR/MC）+ 历史 + 物理键盘文本映射。
#[derive(Default)]
pub struct App {
    tokens: Vec<Tok>,
    input: String,
    after_equals: bool,
    formula: String,
    error: Option<CalcError>,
    mem: Memory,
    history: Vec<HistoryEntry>,
    hist_seq: u64,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: &str) {
        let key = match key {
            // 逻辑按键码 + FocusScope 透传的物理键盘字符（Enter 为 "\n"，退格 0x08，Esc 0x1B）
            "Enter" | "\n" | "\r" => "=",
            "Escape" | "\u{1b}" => "clear",
            "Backspace" | "\u{8}" => "backspace",
            "Delete" | "\u{7f}" => "ce",
            "F9" => "negate",
            "F2" => "sqrt",
            "%" => "percent",
            other => other,
        };
        if self.error.is_some() && key != "clear" {
            self.reset();
        }
        match key {
            "clear" => self.reset(),
            "ce" => self.input.clear(),
            "backspace" => {
                if !self.after_equals {
                    self.input.pop();
                }
            }
            "." => self.on_dot(),
            "=" => self.on_equals(),
            "+" => self.on_operator(Op::Add),
            "-" => self.on_operator(Op::Sub),
            "*" => self.on_operator(Op::Mul),
            "/" => self.on_operator(Op::Div),
            "percent" => self.on_percent(),
            "negate" => self.on_negate(),
            "inv" => self.on_unary(UnaryOp::Reciprocal),
            "square" => self.on_unary(UnaryOp::Square),
            "sqrt" => self.on_unary(UnaryOp::SquareRoot),
            "ms" => self.on_memory(MemOp::Store),
            "m+" => self.on_memory(MemOp::Add),
            "m-" => self.on_memory(MemOp::Sub),
            "mr" => self.on_memory(MemOp::Recall),
            "mc" => self.on_memory(MemOp::Clear),
            other => {
                if let Some(c) = other.chars().next() {
                    if c.is_ascii_digit() && other.len() == 1 {
                        self.on_digit(c);
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        let mem = self.mem;
        let (history, seq) = (std::mem::take(&mut self.history), self.hist_seq);
        *self = Self::default();
        self.mem = mem; // C 不清记忆/历史，同原版
        self.history = history;
        self.hist_seq = seq;
    }

    /// 当前可参与记忆运算的值：有表达式时按完整表达式（含输入串）实时求值，同原版存"屏幕上的值"
    fn current_value(&self) -> Option<Value> {
        if self.tokens.is_empty() {
            return (!self.input.is_empty()).then(|| value_from_str(&self.input));
        }
        let mut toks = self.tokens.clone();
        if !self.input.is_empty() {
            toks.push(Tok::Num(value_from_str(&self.input)));
        }
        evaluate(&toks).ok()
    }

    fn on_memory(&mut self, op: MemOp) {
        match op {
            MemOp::Clear => self.mem.clear(),
            MemOp::Recall => {
                if let Some(v) = self.mem.recall() {
                    self.tokens = vec![Tok::Num(v)];
                    self.input.clear();
                    self.formula.clear();
                    self.after_equals = true;
                }
            }
            _ => {
                if let Some(v) = self.current_value() {
                    match op {
                        MemOp::Store => self.mem.store(v),
                        MemOp::Add => self.mem.add(v),
                        MemOp::Sub => self.mem.sub(v),
                        _ => {}
                    }
                    // 记忆操作会"落地"当前输入：屏幕保留结果，后续数字另起新输入
                    self.tokens = vec![Tok::Num(v)];
                    self.input.clear();
                    self.formula.clear();
                    self.after_equals = true;
                }
            }
        }
    }

    pub fn memory_busy(&self) -> bool {
        self.mem.busy()
    }

    fn push_history(&mut self, expression: String, v: &Value) {
        let result = value_to_display(v);
        // 同原版：与最新一条完全相同则不重复记录
        if self.history.first().is_some_and(|h| h.expression == expression && h.result == result) {
            return;
        }
        self.history.insert(0, HistoryEntry { expression, result, value: *v });
        self.history.truncate(50);
        self.hist_seq += 1;
    }

    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// 每次历史记录变化 +1，Rust 端据此决定是否重建列表模型
    pub fn history_seq(&self) -> u64 {
        self.hist_seq
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.hist_seq += 1;
    }

    pub fn recall_history(&mut self, index: i32) {
        let Some(h) = self.history.get(index.max(0) as usize) else {
            return;
        };
        self.error = None;
        self.tokens = vec![Tok::Num(h.value)];
        self.input.clear();
        self.formula = h.expression.clone();
        self.after_equals = true;
    }

    fn commit(&mut self) {
        if !self.input.is_empty() {
            self.tokens.push(Tok::Num(value_from_str(&self.input)));
            self.input.clear();
        }
    }

    fn leave_equals(&mut self) {
        if self.after_equals {
            self.after_equals = false;
            self.formula.clear();
        }
    }

    /// 结果态下按数字/小数点 = 开启全新表达式（结果不延续）
    fn break_equals(&mut self) {
        if self.after_equals {
            self.after_equals = false;
            self.formula.clear();
            self.tokens.clear();
        }
    }

    fn on_digit(&mut self, d: char) {
        self.break_equals();
        if self.input == "0" {
            self.input = d.to_string();
        } else if self.input.chars().count() < 16 {
            self.input.push(d);
        }
    }

    fn on_dot(&mut self) {
        self.break_equals();
        if self.input.is_empty() {
            self.input = "0.".into();
        } else if !self.input.contains('.') {
            self.input.push('.');
        }
    }

    fn on_operator(&mut self, op: Op) {
        self.commit();
        self.leave_equals();
        if matches!(self.tokens.last(), Some(Tok::Bin(_))) {
            self.tokens.pop();
        } else if self.tokens.is_empty() {
            self.tokens.push(Tok::Num(Value::from_int(0)));
        }
        self.tokens.push(Tok::Bin(op));
    }

    fn on_equals(&mut self) {
        self.commit();
        if self.tokens.is_empty() {
            return;
        }
        match evaluate(&self.tokens) {
            Ok(v) => {
                self.formula = format!("{} =", expression_to_display(&self.tokens));
                self.push_history(self.formula.clone(), &v);
                self.tokens = vec![Tok::Num(v)];
                self.after_equals = true;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn on_percent(&mut self) {
        self.leave_equals();
        if !self.input.is_empty() {
            self.commit();
            self.tokens.push(Tok::Percent);
        } else if matches!(self.tokens.last(), Some(Tok::Num(_) | Tok::RParen)) {
            self.tokens.push(Tok::Percent);
        }
    }

    fn on_negate(&mut self) {
        if !self.input.is_empty() {
            self.input = match self.input.strip_prefix('-') {
                Some(rest) => rest.to_string(),
                None => format!("-{}", self.input),
            };
        } else {
            match self.tokens.last_mut() {
                Some(Tok::Num(v)) => *v = v.neg(),
                Some(Tok::NumText(v, text)) => {
                    *v = v.neg();
                    *text = format!("-({text})");
                }
                _ => {}
            }
        }
    }

    /// 取走"当前操作数"（输入串优先，其次表达式末尾数字），返回 (值, 展示文本)
    fn take_operand(&mut self) -> Option<(Value, String)> {
        if !self.input.is_empty() {
            let text = std::mem::take(&mut self.input);
            return Some((value_from_str(&text), text));
        }
        match self.tokens.pop() {
            Some(Tok::Num(v)) => Some((v, value_to_display(&v))),
            Some(Tok::NumText(v, text)) => Some((v, text)),
            Some(other) => {
                self.tokens.push(other);
                None
            }
            None => None,
        }
    }

    /// 一元运算即时求值，结果以函数文本回填（2 的平方显示 sqr(2)），同原版
    fn on_unary(&mut self, op: UnaryOp) {
        self.leave_equals();
        let Some((v, text)) = self.take_operand() else { return };
        match apply_unary(op, v) {
            Ok(r) => {
                let name = match op {
                    UnaryOp::Reciprocal => "1/",
                    UnaryOp::Square => "sqr",
                    UnaryOp::SquareRoot => "sqrt",
                    UnaryOp::Negate => "-",
                };
                self.tokens.push(Tok::NumText(r, format!("{name}({text})")));
                self.after_equals = true;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn big_line(&self) -> String {
        if let Some(e) = self.error {
            return error_message(e).into();
        }
        let mut toks = self.tokens.clone();
        if !self.input.is_empty() {
            toks.push(Tok::Num(value_from_str(&self.input)));
        }
        if toks.is_empty() {
            return "0".into();
        }
        expression_to_display(&toks)
    }

    pub fn small_line(&self) -> String {
        if self.error.is_some() {
            String::new()
        } else {
            self.formula.clone()
        }
    }
}

#[derive(Clone, Copy)]
enum MemOp {
    Store,
    Add,
    Sub,
    Recall,
    Clear,
}

#[cfg(test)]
mod tests {
    use super::App;

    fn type_keys(seq: &[&str]) -> (String, String) {
        let mut app = App::new();
        for k in seq {
            app.handle_key(k);
        }
        (app.big_line(), app.small_line())
    }

    #[test]
    fn precedence_via_ui() {
        assert_eq!(
            type_keys(&["2", "+", "3", "*", "4", "="]),
            ("14".into(), "2 + 3 × 4 =".into())
        );
    }

    #[test]
    fn live_expression_shown() {
        assert_eq!(type_keys(&["1", "2", "+", "3"]).0, "12 + 3");
    }

    #[test]
    fn continue_after_equals() {
        assert_eq!(type_keys(&["2", "+", "3", "=", "*", "4", "="]).0, "20");
    }

    #[test]
    fn replace_operator_and_ce_clear_input() {
        assert_eq!(type_keys(&["5", "+", "*", "2", "="]).0, "10");
        assert_eq!(type_keys(&["1", "2", "3", "ce", "+", "1", "="]).0, "1");
    }

    #[test]
    fn percent_and_negate() {
        assert_eq!(type_keys(&["5", "0", "+", "1", "0", "percent", "="]).0, "55");
        assert_eq!(type_keys(&["7", "negate", "+", "1", "0", "="]).0, "3");
    }

    #[test]
    fn error_then_any_key_recovers() {
        assert_eq!(type_keys(&["5", "/", "0", "="]).0, "除数不能为零。");
        assert_eq!(type_keys(&["5", "/", "0", "=", "3", "+", "1", "="]).0, "4");
    }

    #[test]
    fn backspace_edits_input() {
        assert_eq!(type_keys(&["1", "2", "3", "backspace"]).0, "12");
    }

    #[test]
    fn unary_instant_function_display() {
        assert_eq!(
            type_keys(&["2", "square", "="]),
            ("4".into(), "sqr(2) =".into())
        );
        assert_eq!(type_keys(&["9", "sqrt", "+", "1", "="]).0, "4");
        assert_eq!(type_keys(&["9", "sqrt"]).0, "sqrt(9)");
        assert_eq!(type_keys(&["2", "square", "square", "="]).0, "16");
        assert_eq!(
            type_keys(&["4", "inv", "="]),
            ("0.25".into(), "1/(4) =".into())
        );
    }

    #[test]
    fn unary_then_digit_starts_fresh() {
        assert_eq!(type_keys(&["2", "square", "5"]).0, "5");
        assert_eq!(type_keys(&["2", "square", "*", "3", "="]).0, "12");
    }

    #[test]
    fn unary_errors() {
        assert_eq!(type_keys(&["0", "inv"]).0, "除数不能为零。");
        assert_eq!(type_keys(&["4", "sqrt", "negate", "sqrt"]).0, "输入无效。");
    }

    fn run(seq: &[&str]) -> App {
        let mut app = App::new();
        for k in seq {
            app.handle_key(k);
        }
        app
    }

    #[test]
    fn memory_store_recall_clear() {
        let app = run(&["2", "ms"]);
        assert!(app.memory_busy());
        let app = run(&["2", "ms", "clear", "mr"]);
        assert!(app.memory_busy(), "C 不应清空记忆");
        assert_eq!(app.big_line(), "2");
        let app = run(&["2", "ms", "clear", "mr", "mc"]);
        assert!(!app.memory_busy());
    }

    #[test]
    fn memory_stores_live_expression() {
        let app = run(&["1", "2", "+", "3", "ms", "clear", "mr"]);
        assert_eq!(app.big_line(), "15");
    }

    #[test]
    fn memory_add_subtract() {
        let app = run(&["5", "ms", "3", "m+", "2", "m-", "mr"]);
        assert_eq!(app.big_line(), "6");
        // 记忆为空时按 M- 直接存入相反数
        let app = run(&["4", "m-", "mr"]);
        assert_eq!(app.big_line(), "-4");
    }

    #[test]
    fn physical_keyboard_aliases() {
        assert_eq!(run(&["2", "+", "3", "\n"]).big_line(), "5");
        assert_eq!(run(&["1", "2", "\u{8}"]).big_line(), "1");
        assert_eq!(run(&["9", "\u{1b}"]).big_line(), "0");
        assert_eq!(run(&["5", "/", "0", "="]).big_line(), "除数不能为零。");
        assert_eq!(run(&["5", "/", "0", "=", "7"]).big_line(), "7");
    }

    #[test]
    fn history_records_newest_first() {
        let mut app = run(&["2", "+", "3", "=", "clear", "4", "*", "5", "="]);
        assert_eq!(app.history().len(), 2, "C 不清历史");
        assert_eq!(app.history()[0].expression, "4 × 5 =");
        assert_eq!(app.history()[1].result, "5");
        app.recall_history(1);
        assert_eq!(app.big_line(), "5");
        assert_eq!(app.small_line(), "2 + 3 =");
        // 召回后可继续运算
        assert_eq!({
            app.handle_key("+");
            app.handle_key("1");
            app.handle_key("=");
            app.big_line()
        }, "6");
    }

    #[test]
    fn history_dedup_and_clear() {
        let mut app = run(&["1", "=", "clear", "1", "="]);
        assert_eq!(app.history().len(), 1, "重复结果不记录");
        let seq = app.history_seq();
        app.clear_history();
        assert!(app.history().is_empty());
        assert_ne!(app.history_seq(), seq, "清空要触发模型重建");
    }

    #[test]
    fn history_ignores_errors() {
        let app = run(&["5", "/", "0", "=", "1", "+", "1", "="]);
        assert_eq!(app.history().len(), 1);
        assert_eq!(app.history()[0].result, "2");
    }
}
