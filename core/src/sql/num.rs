//! Exact decimal numbers: no floating point, so `9.99 < 10` is exactly true and
//! `1 = 1.0`. At most 18 digits in total, so scaling to compare can't overflow
//! an `i128`.

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Num {
    mant: i128,
    scale: u8,
}

const MAX_DIGITS: usize = 18;

impl Num {
    pub fn int(v: i64) -> Num { Num { mant: v as i128, scale: 0 } }

    /// Parse `[-]digits[.digits]`.
    pub fn parse(s: &str) -> Result<Num, String> {
        let (neg, body) = match s.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, s),
        };
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, f),
            None => (body, ""),
        };
        let valid = |p: &str| p.chars().all(|c| c.is_ascii_digit());
        if int_part.is_empty() || !valid(int_part) || !valid(frac_part) || (body.contains('.') && frac_part.is_empty()) {
            return Err(format!("'{s}' is not a number"));
        }
        let digits = int_part.len() + frac_part.len();
        if digits > MAX_DIGITS {
            return Err(format!("number '{s}' has more than {MAX_DIGITS} digits"));
        }
        let mut mant: i128 = 0;
        for c in int_part.chars().chain(frac_part.chars()) {
            mant = mant * 10 + c.to_digit(10).unwrap_or(0) as i128;
        }
        Ok(Num { mant: if neg { -mant } else { mant }, scale: frac_part.len() as u8 })
    }

    pub fn is_integer(&self) -> bool { self.scale == 0 }

    pub fn cmp_num(&self, other: &Num) -> Ordering {
        let scale = self.scale.max(other.scale);
        let a = self.mant * 10i128.pow((scale - self.scale) as u32);
        let b = other.mant * 10i128.pow((scale - other.scale) as u32);
        a.cmp(&b)
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.mant < 0;
        let digits = self.mant.unsigned_abs().to_string();
        let scale = self.scale as usize;
        let sign = if neg { "-" } else { "" };
        if scale == 0 {
            return write!(f, "{sign}{digits}");
        }
        let padded = if digits.len() <= scale { format!("{}{digits}", "0".repeat(scale + 1 - digits.len())) } else { digits };
        let (i, frac) = padded.split_at(padded.len() - scale);
        write!(f, "{sign}{i}.{frac}")
    }
}
