use crate::engine::{
    apply_unary_in, error_message, evaluate, expression_to_display, value_from_str, value_rand,
    value_to_display, AngleMode, CalcError, Memory, Op, Rational, Tok, UnaryOp, Value,
};

/// 一条历史记录：表达式（含末尾 "="）、展示结果、可直接召回的值
#[derive(Clone)]
pub struct HistoryEntry {
    pub expression: String,
    pub result: String,
    pub value: Value,
}

/// 历史上限（同原版：只留最近 50 条），落盘与恢复都按这个上限走
pub const HISTORY_LIMIT: usize = 50;

/// 值 → 状态文件里的一个字段。精确有理数走 `E num den`（`0.5` 恢复后仍是 1/2，
/// 不降级成浮点），近似值走 `A <Debug 形式>`；Rust 的 f64 Debug 是最短可回读表示，
/// `inf`/`NaN` 也能被 `parse::<f64>()` 原样读回。
fn encode_value(v: Value) -> String {
    match v {
        Value::Exact(r) => format!("E {} {}", r.num, r.den),
        Value::Approx(f) => format!("A {f:?}"),
    }
}

fn decode_value(s: &str) -> Option<Value> {
    let mut p = s.split(' ');
    match p.next()? {
        "E" => Rational::new(p.next()?.parse().ok()?, p.next()?.parse().ok()?).map(Value::exact),
        "A" => p.next()?.parse::<f64>().ok().map(Value::Approx),
        _ => None,
    }
}

/// 状态文件按行按 tab 分列，表达式里理论上不会出现这两个字符（按键码不含），
/// 但写文件前还是折一下，免得手工改过的文件把后面的列挤歪。
fn one_line(s: &str) -> String {
    s.chars().map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

/// 计算器模式（S3 起驱动 UI 键盘互换）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcMode {
    Standard,
    Scientific,
}

impl Default for CalcMode {
    fn default() -> Self {
        CalcMode::Standard
    }
}

/// M5 控制器：四则/一元/百分号/CE/C/← + 记忆（MS/M+/M-/MR/MC）+ 历史 + 物理键盘文本映射。
/// S2 起追加科学型：模式/角度制/2nd、幂根模二元、三角对数一元、常量与括号。
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
    mode: CalcMode,
    angle: AngleMode,
    second: bool,
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
            "2nd" => "second",
            // 科学型物理键映射（同原版：q=√ @=x² i=1/x s=sin o=cos t=tan l=ln g=log n=!=n! r=rand ^=xʸ；C 仍是清空）
            "q" => "sqrt",
            "@" => "square",
            "i" => "inv",
            "s" => "sin",
            "o" => "cos",
            "t" => "tan",
            "l" => "ln",
            "g" => "log",
            "n" | "!" => "fact",
            "r" => "rand",
            "^" => "pow",
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
            // ---- 科学型（S2）----
            "(" => self.on_lparen(),
            ")" => self.on_rparen(),
            "second" => self.second = !self.second,
            "deg" => self.angle = AngleMode::Deg,
            "rad" => self.angle = AngleMode::Rad,
            "standard" => self.set_mode(CalcMode::Standard),
            "scientific" => self.set_mode(CalcMode::Scientific),
            "pow" => self.on_operator(Op::Pow),
            "root" => self.on_operator(Op::Root),
            "mod" => self.on_operator(Op::Mod),
            "exp" => self.on_operator(Op::Exp),
            "sin" => self.on_trig(UnaryOp::Sin, UnaryOp::Asin),
            "cos" => self.on_trig(UnaryOp::Cos, UnaryOp::Acos),
            "tan" => self.on_trig(UnaryOp::Tan, UnaryOp::Atan),
            "sec" => self.on_trig(UnaryOp::Sec, UnaryOp::Asec),
            "csc" => self.on_trig(UnaryOp::Csc, UnaryOp::Acsc),
            "cot" => self.on_trig(UnaryOp::Cot, UnaryOp::Acot),
            "sinh" => self.on_trig(UnaryOp::Sinh, UnaryOp::Asinh),
            "cosh" => self.on_trig(UnaryOp::Cosh, UnaryOp::Acosh),
            "tanh" => self.on_trig(UnaryOp::Tanh, UnaryOp::Atanh),
            "sech" => self.on_trig(UnaryOp::Sech, UnaryOp::Asech),
            "csch" => self.on_trig(UnaryOp::Csch, UnaryOp::Acsch),
            "coth" => self.on_trig(UnaryOp::Coth, UnaryOp::Acoth),
            "floor" => self.on_unary(UnaryOp::Floor),
            "ceil" => self.on_unary(UnaryOp::Ceil),
            "todms" => self.on_convert(true),
            "todeg" => self.on_convert(false),
            "cube" => self.on_unary(UnaryOp::Cube),
            "cbrt" => self.on_unary(UnaryOp::Cbrt),
            "tenpow" => self.on_unary(UnaryOp::TenPow),
            "epow" => self.on_unary(UnaryOp::EPow),
            "twopow" => self.on_unary(UnaryOp::TwoPow),
            "ln" => self.on_unary(UnaryOp::Ln),
            "log" => self.on_unary(UnaryOp::Log),
            "fact" => self.on_unary(UnaryOp::Factorial),
            "abs" => self.on_unary(UnaryOp::Abs),
            "pi" => self.on_const(Value::Approx(std::f64::consts::PI), "π"),
            "e" => self.on_const(Value::Approx(std::f64::consts::E), "e"),
            "rand" => self.on_const(value_rand(), "rand"),
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
        let history = std::mem::take(&mut self.history);
        let seq = self.hist_seq;
        let (mode, angle, second) = (self.mode, self.angle, self.second);
        *self = Self::default();
        self.mem = mem; // C 不清记忆/历史/模式/角度制，同原版
        self.history = history;
        self.hist_seq = seq;
        self.mode = mode;
        self.angle = angle;
        self.second = second;
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
        self.history.truncate(HISTORY_LIMIT);
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
            // 数字紧跟操作数/闭括号：隐式乘法（"12(3)" → 12 × (3)）
            if matches!(
                self.tokens.last(),
                Some(Tok::Num(_) | Tok::NumText(_, _) | Tok::RParen)
            ) {
                self.tokens.push(Tok::Bin(Op::Mul));
            }
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
        if let Some(Tok::Bin(prev)) = self.tokens.last().cloned() {
            // 幂/根/mod 之后的 +/- 是下一操作数的符号，两个都保留（2^-3=0.125）
            if !(prev.takes_signed_operand() && matches!(op, Op::Add | Op::Sub)) {
                self.tokens.pop();
            }
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
        match apply_unary_in(self.angle, op, v) {
            Ok(r) => {
                self.tokens
                    .push(Tok::NumText(r, format!("{}({text})", op.display_name())));
                self.after_equals = true;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// 三角键：2nd 打开时用反函数，用完自动复位（同原版）
    fn on_trig(&mut self, normal: UnaryOp, alt: UnaryOp) {
        let op = if self.second {
            self.second = false;
            alt
        } else {
            normal
        };
        self.on_unary(op);
    }

    /// π / e / rand：作为操作数插入，展示用符号文本
    fn on_const(&mut self, v: Value, text: &str) {
        self.break_equals();
        self.input.clear();
        self.tokens.push(Tok::NumText(v, text.to_string()));
        self.after_equals = true;
    }

    /// →dms：当前角度值改显度分秒文本（数值不变）；→deg：把当前值按 DMS 位制重解为十进制度
    fn on_convert(&mut self, to_dms: bool) {
        self.leave_equals();
        let Some((v, _)) = self.take_operand() else { return };
        let f = v.to_f64();
        let (r, text) = if to_dms {
            (v, dms_text(f))
        } else {
            let d = f.trunc();
            let rest = (f - d).abs() * 100.0;
            let m = rest.trunc();
            let s = (rest - m) * 100.0;
            let val = d + d.signum() * (m / 60.0 + s / 3600.0);
            (super::engine::snap_value(val), plain_text(val))
        };
        self.tokens.push(Tok::NumText(r, text));
        self.after_equals = true;
    }

    fn on_lparen(&mut self) {
        self.break_equals();
        self.commit();
        if matches!(
            self.tokens.last(),
            Some(Tok::Num(_) | Tok::NumText(_, _) | Tok::RParen)
        ) {
            self.tokens.push(Tok::Bin(Op::Mul)); // "(2)(3)" 类隐式乘法
        }
        self.tokens.push(Tok::LParen);
    }

    fn on_rparen(&mut self) {
        self.commit();
        if !matches!(
            self.tokens.last(),
            Some(Tok::Num(_) | Tok::NumText(_, _) | Tok::RParen)
        ) {
            return; // "(+)"、"5)" 等无处可闭，忽略
        }
        let opens = self.tokens.iter().filter(|t| matches!(t, Tok::LParen)).count();
        let closes = self.tokens.iter().filter(|t| matches!(t, Tok::RParen)).count();
        if opens > closes {
            self.tokens.push(Tok::RParen);
        }
    }

    /// 序列化成状态文本（格式见 `crate::persist`）：只存"换个进程还想看到"的东西——
    /// 模式、角度制、记忆槽、历史（最新在前，最多 `HISTORY_LIMIT` 条）。
    /// 当前输入的表达式不存，同原版重开计算器只有一个干净的 0。
    pub fn snapshot(&self) -> String {
        let mut out = String::from("slint-ms-calc\tv1\n");
        out.push_str(&format!(
            "mode\t{}\n",
            if self.mode == CalcMode::Scientific { "scientific" } else { "standard" }
        ));
        out.push_str(&format!(
            "angle\t{}\n",
            match self.angle {
                AngleMode::Deg => "deg",
                AngleMode::Rad => "rad",
            }
        ));
        if let Some(v) = self.mem.recall() {
            out.push_str(&format!("mem\t{}\n", encode_value(v)));
        }
        for h in self.history.iter().take(HISTORY_LIMIT) {
            out.push_str(&format!(
                "hist\t{}\t{}\t{}\n",
                encode_value(h.value),
                one_line(&h.expression),
                one_line(&h.result)
            ));
        }
        out
    }

    /// 从状态文本恢复。认不出的行一律跳过：将来格式升级、或文件被手改坏，
    /// 都只该丢那一行，不该让程序起不来。
    pub fn restore(&mut self, text: &str) {
        // 三个"单值"字段用 Option<Option<..>>：外层 None = 文件里根本没有这一行（保持默认），
        // 内层 None = 有这一行但值认不出（同样保持默认，但别把它当成错误）。
        let mut mode: Option<Option<CalcMode>> = None;
        let mut angle: Option<Option<AngleMode>> = None;
        let mut mem: Option<Value> = None;
        let mut history: Vec<HistoryEntry> = Vec::new();
        for line in text.lines() {
            let mut fields = line.split('\t');
            match fields.next() {
                Some("mode") => {
                    mode = Some(match fields.next() {
                        Some("scientific") => Some(CalcMode::Scientific),
                        Some("standard") => Some(CalcMode::Standard),
                        _ => None,
                    })
                }
                Some("angle") => {
                    angle = Some(match fields.next() {
                        Some("rad") => Some(AngleMode::Rad),
                        Some("deg") => Some(AngleMode::Deg),
                        _ => None,
                    })
                }
                Some("mem") => mem = fields.next().and_then(decode_value),
                Some("hist") => {
                    let (Some(value), Some(expression), Some(result)) = (
                        fields.next().and_then(decode_value),
                        fields.next(),
                        fields.next(),
                    ) else {
                        continue;
                    };
                    if expression.is_empty() || history.len() >= HISTORY_LIMIT {
                        continue;
                    }
                    history.push(HistoryEntry {
                        expression: expression.to_string(),
                        result: result.to_string(),
                        value,
                    });
                }
                _ => {}
            }
        }
        if let Some(Some(m)) = mode {
            self.mode = m;
        }
        if let Some(Some(a)) = angle {
            self.angle = a;
        }
        if let Some(v) = mem {
            self.mem.store(v);
        }
        if !history.is_empty() {
            self.history = history;
        }
        // 版本号必须动：Rust 端靠它决定要不要重建历史模型，启动时的初值是 0，
        // 恢复出条目却仍是 0 的话，历史面板会一直是空的。
        self.hist_seq += 1;
    }

    pub fn set_mode(&mut self, m: CalcMode) {
        if self.mode == m {
            return;
        }
        self.mode = m;
        self.reset(); // 切模式清当前表达式，保留记忆/历史/角度制
    }

    pub fn mode(&self) -> CalcMode {
        self.mode
    }

    pub fn angle_label(&self) -> &'static str {
        match self.angle {
            AngleMode::Deg => "DEG",
            AngleMode::Rad => "RAD",
        }
    }

    pub fn second_active(&self) -> bool {
        self.second
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

/// 十进制度 → "12°30′15.5″" 文本（秒保留至 1e-6，进位回分/时）
fn dms_text(f: f64) -> String {
    let sign = if f < 0.0 { "-" } else { "" };
    let a = f.abs();
    let mut d = a.trunc() as i64;
    let mf = (a - d as f64) * 60.0;
    let mut m = mf.trunc() as i64;
    let mut s = ((mf - m as f64) * 60.0 * 1e6).round() / 1e6;
    if s >= 60.0 {
        s -= 60.0;
        m += 1;
    }
    if m >= 60 {
        m -= 60;
        d += 1;
    }
    let st = if s == s.trunc() {
        format!("{}", s.trunc() as i64)
    } else {
        format!("{}", s)
    };
    format!("{sign}{d}°{m}′{st}″")
}

/// →deg 结果的简洁十进制文本（整数不带小数点，小数去尾零）
fn plain_text(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{}", f.trunc() as i64)
    } else {
        let mut s = format!("{:.10}", f);
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_value, App, CalcMode, HISTORY_LIMIT};
    use crate::engine::{Rational, Value};

    fn type_keys(seq: &[&str]) -> (String, String) {
        let mut app = App::new();
        for k in seq {
            app.handle_key(k);
        }
        (app.big_line(), app.small_line())
    }

    /// 状态落盘 ↔ 恢复：置顶在 Wayland 下只能靠重启进程实现，重启不能丢记忆和历史
    #[test]
    fn snapshot_roundtrip_keeps_mem_history_mode_angle() {
        let mut app = App::new();
        for k in ["1", "/", "2", "=", "3", "+", "4", "="] {
            app.handle_key(k);
        }
        app.handle_key("8");
        app.handle_key("ms");
        app.handle_key("scientific");
        app.handle_key("rad");
        // 上面切模式时把表达式清了，重新攒一条含近似值的历史（sin 30° 走 f64 分支）
        app.handle_key("sin");
        app.handle_key("3");
        app.handle_key("0");
        app.handle_key(")");
        app.handle_key("=");

        let mut back = App::new();
        back.restore(&app.snapshot());
        assert_eq!(back.mode(), CalcMode::Scientific, "模式要跟着恢复");
        assert_eq!(back.angle_label(), "RAD");
        assert_eq!(back.history().len(), app.history().len());
        assert_ne!(back.history_seq(), 0, "版本号要动，否则 UI 认为历史模型不用重建");
        assert_eq!(back.history()[0].expression, app.history()[0].expression);
        assert_eq!(back.history()[1].result, "7");
        assert_eq!(back.history()[2].result, "0.5");
        // 精确有理数不能降级成浮点：1 ÷ 2 恢复后召回，显示的仍是 0.5 且值逐位相同
        assert_eq!(back.history()[2].value, app.history()[2].value);
        assert!(back.memory_busy());
        back.handle_key("mr");
        assert_eq!(back.big_line(), "8");
    }

    #[test]
    fn snapshot_roundtrip_covers_special_floats() {
        for v in [
            Value::Approx(f64::INFINITY),
            Value::Approx(f64::NEG_INFINITY),
            Value::Approx(f64::NAN),
            Value::Approx(1e300),
            Value::from_int(-7),
            Value::exact(Rational::new(1, 3).unwrap()),
        ] {
            let mut app = App::new();
            app.restore(&format!("mem\t{}\n", encode_value(v)));
            assert!(app.memory_busy(), "{} 应恢复出记忆槽", encode_value(v));
            let mut back = App::new();
            back.restore(&app.snapshot());
            // NaN 走 to_bits 比较（Value 的 PartialEq 对 Approx 就是这么做的）
            assert_eq!(
                back.mem.recall().map(|x| x.to_f64().to_bits()),
                Some(v.to_f64().to_bits()),
                "{} 存出去要能原样回来",
                encode_value(v)
            );
        }
    }

    #[test]
    fn restore_skips_unparsable_lines() {
        let text = "slint-ms-calc\tv9\n\
                    future-field\twhatever\n\
                    mode\tcalculator\n\
                    mem\tE 1 0\n\
                    mem\tZZZ\n\
                    hist\tE 3 4\t3 ÷ 4 =\t0.75\n\
                    hist\tbogus x y\n";
        let mut app = App::new();
        app.restore(text);
        assert!(!app.memory_busy(), "坏 mem 行要整行跳过");
        assert_eq!(app.mode(), CalcMode::Standard, "认不出的模式值不改默认");
        assert_eq!(app.history().len(), 1);
        assert_eq!(app.history()[0].result, "0.75");
        assert_ne!(app.history_seq(), 0, "有恢复内容就要触发模型重建");
    }

    #[test]
    fn restore_truncates_to_history_limit() {
        let mut text = String::from("slint-ms-calc\tv1\n");
        for i in 0..(HISTORY_LIMIT + 20) {
            text.push_str(&format!("hist\tE {i} 1\t{i} =\t{i}\n"));
        }
        let mut app = App::new();
        app.restore(&text);
        assert_eq!(app.history().len(), HISTORY_LIMIT);
        assert_eq!(app.history()[0].result, "0", "顺序保持：最新在前");
    }

    #[test]
    fn empty_snapshot_restores_nothing_but_version() {
        let mut app = App::new();
        app.restore("slint-ms-calc\tv1\n");
        assert!(!app.memory_busy());
        assert!(app.history().is_empty());
        assert_eq!(app.big_line(), "0");
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

    #[test]
    fn sci_trig_deg_and_display() {
        let app = run(&["3", "0", "sin"]);
        assert_eq!(app.big_line(), "sin(30)");
        assert_eq!(app.angle_label(), "DEG");
        let app = run(&["3", "0", "sin", "="]);
        assert_eq!(app.big_line(), "0.5");
        assert_eq!(app.small_line(), "sin(30) =");
    }

    #[test]
    fn sci_second_toggles_inverse_and_resets() {
        let app = run(&["0", ".", "5", "2nd", "sin"]);
        assert_eq!(app.big_line(), "sin⁻¹(0.5)");
        assert!(!app.second_active(), "2nd 用完自动复位");
        assert_eq!(run(&["0", ".", "5", "2nd", "sin", "="]).big_line(), "30");
        // 未带 2nd 仍是普通 sin
        assert_eq!(run(&["3", "0", "sin"]).big_line(), "sin(30)");
    }

    #[test]
    fn sci_angle_mode_switch() {
        let app = run(&["3", "0", "sin", "="]);
        assert_eq!(app.big_line(), "0.5");
        let app = run(&["3", "0", "rad", "sin", "="]);
        assert_eq!(app.angle_label(), "RAD");
        // sin(30 rad) ≈ -0.988，非错误即可
        assert!(!app.big_line().contains("无效") && !app.big_line().is_empty());
        // C 保留角度制
        let app = run(&["rad", "clear"]);
        assert_eq!(app.angle_label(), "RAD");
    }

    #[test]
    fn sci_paren_basic_and_implicit_mul() {
        let app = run(&["(", "3", "+", "4", ")", "*", "5", "="]);
        assert_eq!(app.big_line(), "35");
        assert_eq!(app.small_line(), "(3 + 4) × 5 =");
        // 12( → 12 × (
        let app = run(&["1", "2", "(", "3", ")"]);
        assert_eq!(app.big_line(), "12 × (3)");
        assert_eq!(run(&["1", "2", "(", "3", ")", "="]).big_line(), "36");
        // 多余右括号忽略；缺右括号按 = 自动闭合
        assert_eq!(run(&["5", ")"]).big_line(), "5");
        assert_eq!(run(&["(", "2", "+", "3", "="]).big_line(), "5");
    }

    #[test]
    fn sci_binary_power_root_mod() {
        assert_eq!(run(&["2", "pow", "1", "0", "="]).big_line(), "1,024");
        assert_eq!(
            run(&["2", "pow", "1", "0", "="]).small_line(),
            "2 ^ 10 ="
        );
        assert_eq!(run(&["2", "pow", "3", "pow", "2", "="]).big_line(), "512");
        // 2^-3：幂后紧跟 +/- 视为指数符号
        assert_eq!(run(&["2", "pow", "-", "3", "="]).big_line(), "0.125");
        assert_eq!(run(&["3", "root", "8", "="]).big_line(), "2");
        assert_eq!(run(&["5", "mod", "2", "="]).big_line(), "1");
        assert_eq!(run(&["5", "negate", "mod", "2", "="]).big_line(), "-1");
    }

    #[test]
    fn sci_exp_notation() {
        // exp = 科学记数法 a × 10ᵇ：2 exp 3 = 2E3 = 2000
        assert_eq!(run(&["2", "exp", "3", "="]).big_line(), "2,000");
        assert_eq!(run(&["2", "exp", "3"]).big_line(), "2 E 3");
        // 指数可为负：1.5 exp -2 = 0.015
        assert_eq!(run(&["1", ".", "5", "exp", "-", "2", "="]).big_line(), "0.015");
        // 优先级高于乘除：2 + 3 exp 2 = 2 + 300 = 302
        assert_eq!(run(&["2", "+", "3", "exp", "2", "="]).big_line(), "302");
        assert_eq!(run(&["2", "exp", "2", "*", "3", "="]).big_line(), "600");
    }

    #[test]
    fn sci_unary_functions_display() {
        assert_eq!(run(&["2", "cube", "="]).big_line(), "8");
        assert_eq!(run(&["2", "7", "cbrt"]).big_line(), "cbrt(27)");
        assert_eq!(run(&["2", "7", "cbrt", "="]).big_line(), "3");
        assert_eq!(run(&["1", "0", "0", "log", "="]).big_line(), "2");
        assert_eq!(run(&["5", "fact", "="]).big_line(), "120");
        assert_eq!(run(&["7", "negate", "abs", "="]).big_line(), "7");
        assert_eq!(run(&["2", "tenpow", "="]).big_line(), "100");
        assert_eq!(run(&["1", "0", "twopow", "="]).big_line(), "1,024");
    }

    #[test]
    fn sci_constants() {
        // DEG 下 sin(π) 是 sin(3.14159°)；换 RAD 后 sin(π)=0
        assert_eq!(run(&["pi", "rad", "sin", "="]).big_line(), "0");
        let app = run(&["pi", "sin"]);
        assert_eq!(app.big_line(), "sin(π)");
        assert_eq!(run(&["pi", "+", "2"]).big_line(), "π + 2");
        assert_eq!(run(&["e", "ln", "="]).big_line(), "1");
        // rand 插入 [0,1) 值
        let mut app = App::new();
        app.handle_key("rand");
        assert_eq!(app.big_line(), "rand");
    }

    #[test]
    fn sci_domain_errors() {
        assert_eq!(run(&["2", "2nd", "sin"]).big_line(), "输入无效。");
        assert_eq!(run(&["0", "ln"]).big_line(), "输入无效。");
        assert_eq!(run(&["3", "root", "-", "8", "="]).big_line(), "-2"); // 负数开奇次根
        assert_eq!(run(&["2", "root", "-", "4", "="]).big_line(), "输入无效。");
        assert_eq!(run(&["5", "mod", "0", "="]).big_line(), "除数不能为零。");
        assert_eq!(run(&["1", "7", "1", "fact"]).big_line(), "输入超出范围。");
    }

    #[test]
    fn sci_physical_shortcuts() {
        // 物理键：s=sin、o=cos、^=xʸ、n=n!、q=√、@=x²
        assert_eq!(run(&["3", "0", "s", "="]).big_line(), "0.5");
        assert_eq!(run(&["6", "0", "o", "="]).big_line(), "0.5");
        assert_eq!(run(&["2", "^", "1", "0", "="]).big_line(), "1,024");
        assert_eq!(run(&["5", "n", "="]).big_line(), "120");
        assert_eq!(run(&["4", "q", "="]).big_line(), "2");
        assert_eq!(run(&["9", "@", "="]).big_line(), "81");
    }

    #[test]
    fn sci_mode_switch() {
        let mut app = run(&["2", "+", "3"]);
        app.handle_key("scientific");
        assert_eq!(app.mode(), CalcMode::Scientific);
        assert_eq!(app.big_line(), "0", "切模式清表达式");
        app.handle_key("clear");
        app.handle_key("standard");
        assert_eq!(app.mode(), CalcMode::Standard);
        // 模式切换保留记忆
        let app = run(&["2", "ms", "scientific"]);
        assert!(app.memory_busy());
    }

    #[test]
    fn sci_sec_csc_cot_and_inverse() {
        assert_eq!(run(&["6", "0", "sec"]).big_line(), "sec(60)");
        assert_eq!(run(&["2nd", "2", "sec"]).big_line(), "sec⁻¹(2)");
        assert_eq!(run(&["6", "0", "sec", "="]).big_line(), "2");
        assert_eq!(run(&["3", "0", "csc", "="]).big_line(), "2");
        assert_eq!(run(&["4", "5", "cot", "="]).big_line(), "1");
    }

    #[test]
    fn sci_hyperbolic_and_floor_ceil() {
        assert_eq!(run(&["0", "sinh"]).big_line(), "sinh(0)");
        assert_eq!(run(&["2nd", "0", "sinh", "="]).big_line(), "0");
        assert_eq!(run(&["2", ".", "7", "floor", "="]).big_line(), "2");
        assert_eq!(run(&["2", ".", "1", "ceil"]).big_line(), "ceil(2.1)");
        assert_eq!(run(&["2", ".", "1", "ceil", "="]).big_line(), "3");
    }

    #[test]
    fn sci_dms_conversion() {
        // 12.5 十进制度 → 12°30′0″（数值不变，仅换文本）
        assert_eq!(run(&["1", "2", ".", "5", "todms"]).big_line(), "12°30′0″");
        assert_eq!(run(&["1", "2", ".", "5", "todms", "+", "0", "="]).big_line(), "12.5");
        // →deg：把 12.5 当作 12°50′ 重解 = 12.8333…
        assert_eq!(run(&["1", "2", ".", "5", "todeg"]).big_line(), "12.8333333333");
    }
}
