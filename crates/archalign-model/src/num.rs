//! A JSON number that is written as an integer when it holds a whole value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Largest magnitude at which every integer is exactly representable as `f64` (2^53).
const MAX_EXACT: f64 = 9_007_199_254_740_992.0;

/// A JSON number. Whole values serialize as integers (`40`, not `40.0`), so a load→save
/// round-trip yields an equal `serde_json::Value` for every input that wrote integers
/// (FR-VAL-03).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Num(pub f64);

impl Num {
    /// The value as `f64`.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl From<f64> for Num {
    fn from(v: f64) -> Self {
        Num(v)
    }
}

impl From<i64> for Num {
    fn from(v: i64) -> Self {
        Num(v as f64)
    }
}

impl Serialize for Num {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        if v.is_finite() && v.fract() == 0.0 && v.abs() <= MAX_EXACT {
            s.serialize_i64(v as i64)
        } else {
            s.serialize_f64(v)
        }
    }
}

impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        f64::deserialize(d).map(Num)
    }
}

/// A 2D point or size, `[x, y]` or `[w, h]`.
pub type Pair = [Num; 2];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_values_serialize_as_integers() {
        assert_eq!(serde_json::to_string(&Num(40.0)).unwrap(), "40");
        assert_eq!(serde_json::to_string(&Num(-3.0)).unwrap(), "-3");
        assert_eq!(serde_json::to_string(&Num(40.5)).unwrap(), "40.5");
        assert_eq!(
            serde_json::to_value(Num(12.0)).unwrap(),
            serde_json::json!(12)
        );
    }

    #[test]
    fn deserializes_ints_and_floats() {
        let n: Num = serde_json::from_str("7").unwrap();
        assert_eq!(n, Num(7.0));
        let n: Num = serde_json::from_str("7.25").unwrap();
        assert_eq!(n, Num(7.25));
    }
}
