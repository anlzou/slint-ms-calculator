use super::value::Value;

/// 单个记忆槽，对应原版 MS/M+/M-/MR/MC。
#[derive(Clone, Copy, Default)]
pub struct Memory {
    value: Option<Value>,
}

impl Memory {
    pub fn busy(&self) -> bool {
        self.value.is_some()
    }

    pub fn store(&mut self, v: Value) {
        self.value = Some(v);
    }

    pub fn add(&mut self, v: Value) {
        self.value = Some(match self.value {
            Some(old) => old.add(v),
            None => v,
        });
    }

    pub fn sub(&mut self, v: Value) {
        self.value = Some(match self.value {
            Some(old) => old.sub(v),
            None => v.neg(),
        });
    }

    pub fn recall(&self) -> Option<Value> {
        self.value
    }

    pub fn clear(&mut self) {
        self.value = None;
    }
}
