fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// 精确有理数，den 恒为正且与 num 互素。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    pub num: i128,
    pub den: i128,
}

impl Rational {
    pub fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let (mut n, mut d) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd(n, d);
        if g > 1 {
            n /= g;
            d /= g;
        }
        Some(Self { num: n, den: d })
    }

    pub fn from_int(i: i128) -> Self {
        Self { num: i, den: 1 }
    }

    pub fn neg(self) -> Self {
        Self { num: -self.num, den: self.den }
    }

    pub fn add(self, o: Self) -> Option<Self> {
        let n = self.num.checked_mul(o.den)?.checked_add(o.num.checked_mul(self.den)?)?;
        let d = self.den.checked_mul(o.den)?;
        Self::new(n, d)
    }

    pub fn sub(self, o: Self) -> Option<Self> {
        self.add(o.neg())
    }

    pub fn mul(self, o: Self) -> Option<Self> {
        Self::new(
            self.num.checked_mul(o.num)?,
            self.den.checked_mul(o.den)?,
        )
    }

    /// o 为 0 或溢出时返回 None（调用方先判零）。
    pub fn div(self, o: Self) -> Option<Self> {
        if o.num == 0 {
            return None;
        }
        Self::new(
            self.num.checked_mul(o.den)?,
            self.den.checked_mul(o.num)?,
        )
    }

    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// 完全平方数时返回精确平方根。
    pub fn sqrt_perfect(self) -> Option<Self> {
        if self.num < 0 {
            return None;
        }
        let (sn, sd) = (isqrt(self.num), isqrt(self.den));
        if sn * sn == self.num && sd * sd == self.den {
            Self::new(sn, sd)
        } else {
            None
        }
    }

    /// 整数次幂（负指数取倒数）；超出能力时返回 None 由调用方降级 f64。
    pub fn pow_i(self, e: i128) -> Option<Self> {
        if e < 0 && self.num == 0 {
            return None;
        }
        let e_abs = e.unsigned_abs();
        if e_abs > 10_000 {
            return None;
        }
        let e_abs = e_abs as u32;
        let (bn, bd) = if e >= 0 {
            (self.num, self.den)
        } else {
            (self.den, self.num)
        };
        let n_pow = bn.abs().checked_pow(e_abs)?;
        let n = if bn < 0 && e_abs % 2 == 1 {
            n_pow.checked_neg()?
        } else {
            n_pow
        };
        let d = bd.checked_pow(e_abs)?;
        Self::new(n, d)
    }
}

/// 计算值：能精确表示时用 Rational（0.1+0.2=0.3），溢出时降级 f64。
#[derive(Clone, Copy, Debug)]
pub enum Value {
    Exact(Rational),
    Approx(f64),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Exact(a), Value::Exact(b)) => a == b,
            _ => self.to_f64().to_bits() == other.to_f64().to_bits(),
        }
    }
}

impl Value {
    pub fn exact(r: Rational) -> Value {
        Value::Exact(r)
    }

    pub fn from_int(i: i128) -> Value {
        Value::Exact(Rational::from_int(i))
    }

    pub fn to_f64(self) -> f64 {
        match self {
            Value::Exact(r) => r.to_f64(),
            Value::Approx(f) => f,
        }
    }

    pub fn is_zero(self) -> bool {
        match self {
            Value::Exact(r) => r.num == 0,
            Value::Approx(f) => f == 0.0,
        }
    }

    pub fn is_integer(self) -> bool {
        match self {
            Value::Exact(r) => r.den == 1,
            Value::Approx(f) => f.fract() == 0.0 && f.abs() < i128::MAX as f64,
        }
    }

    pub fn integer_value(self) -> Option<i128> {
        match self {
            Value::Exact(r) if r.den == 1 => Some(r.num),
            Value::Approx(f) if self.is_integer() => Some(f as i128),
            _ => None,
        }
    }

    pub fn neg(self) -> Value {
        match self {
            Value::Exact(r) => Value::Exact(r.neg()),
            Value::Approx(f) => Value::Approx(-f),
        }
    }

    fn as_rational(self) -> Option<Rational> {
        match self {
            Value::Exact(r) => Some(r),
            Value::Approx(_) => None,
        }
    }

    pub fn add(self, o: Value) -> Value {
        match (self.as_rational(), o.as_rational()) {
            (Some(a), Some(b)) => match a.add(b) {
                Some(r) => Value::Exact(r),
                None => Value::Approx(a.to_f64() + b.to_f64()),
            },
            _ => Value::Approx(self.to_f64() + o.to_f64()),
        }
    }

    pub fn sub(self, o: Value) -> Value {
        self.add(o.neg())
    }

    pub fn mul(self, o: Value) -> Value {
        match (self.as_rational(), o.as_rational()) {
            (Some(a), Some(b)) => match a.mul(b) {
                Some(r) => Value::Exact(r),
                None => Value::Approx(a.to_f64() * b.to_f64()),
            },
            _ => Value::Approx(self.to_f64() * o.to_f64()),
        }
    }

    /// 假定 o 非零（调用方先判零）。
    pub fn div(self, o: Value) -> Value {
        match (self.as_rational(), o.as_rational()) {
            (Some(a), Some(b)) => match a.div(b) {
                Some(r) => Value::Exact(r),
                None => Value::Approx(a.to_f64() / b.to_f64()),
            },
            _ => Value::Approx(self.to_f64() / o.to_f64()),
        }
    }

    pub fn percent(self) -> Value {
        self.div(Value::from_int(100))
    }

    /// 整数次幂；Exact 走有理数精确路径，Approx 或溢出时降级 f64。
    pub fn pow_i(self, e: i128) -> Option<Value> {
        match self {
            Value::Exact(r) => r.pow_i(e).map(Value::Exact),
            Value::Approx(_) => None,
        }
    }

    pub fn abs(self) -> Value {
        match self {
            Value::Exact(r) if r.num < 0 => Value::Exact(r.neg()),
            Value::Approx(f) if f < 0.0 => Value::Approx(-f),
            other => other,
        }
    }
}

/// 十进制字面量转精确值；超出 i128 能力时降级 f64。
pub fn value_from_str(s: &str) -> Value {
    let (ip, fp) = match s.split_once('.') {
        Some(pair) => pair,
        None => (s, ""),
    };
    let digits: String = format!("{ip}{fp}").trim_matches('_').to_string();
    let sign_neg = digits.starts_with('-');
    let unsigned = digits.trim_start_matches(['-', '+']);
    if unsigned.is_empty() || unsigned.len() > 24 {
        return Value::Approx(s.parse::<f64>().unwrap_or(0.0));
    }
    match unsigned.parse::<i128>() {
        Ok(v) => {
            let mut num = v;
            if sign_neg {
                num = -num;
            }
            match Rational::new(num, 10i128.pow(fp.len() as u32)) {
                Some(r) => Value::Exact(r),
                None => Value::Approx(s.parse::<f64>().unwrap_or(0.0)),
            }
        }
        Err(_) => Value::Approx(s.parse::<f64>().unwrap_or(0.0)),
    }
}

/// 超越函数结果贴近整数/短小数时取整（sin(180°)=1.2e-16→0，sin(30°)=0.4999…4→0.5），
/// 消去浮点尾巴，同 Windows 展示。
pub fn snap_value(f: f64) -> Value {
    if !f.is_finite() || f.abs() >= 1e16 {
        return Value::Approx(f);
    }
    for d in 0..=3i32 {
        let m = 10f64.powi(d);
        let scaled = (f * m).round();
        if (f - scaled / m).abs() < 1e-12 {
            if d == 0 {
                return Value::from_int(scaled as i128);
            }
            if let Some(r) = Rational::new(scaled as i128, m as i128) {
                return Value::Exact(r);
            }
            return Value::Approx(scaled / m);
        }
    }
    Value::Approx(f)
}

/// [0,1) 伪随机数（xorshift64*，时钟播种），供 rand 键使用。
pub fn value_rand() -> Value {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x2545_F491_4F6C_DD1D);
    let mut x = nanos ^ (nanos >> 32) ^ 0x9E37_79B9_7F4A_7C15;
    if x == 0 {
        x = 0x853C_49E6_748F_EA9B;
    }
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    Value::Approx((r >> 11) as f64 / (1u64 << 53) as f64)
}
