use std::fmt;
use std::ops::{Add, Sub};

use super::error::DomainError;

/// Trait representing a monetary amount with fixed precision
pub trait AmountType:
    Copy + Ord + Add<Output = Self> + Sub<Output = Self> + Default + Send + Sync + fmt::Debug
{
    /// Parse from decimal string (e.g., "1.5000")
    fn from_decimal_str(s: &str) -> Result<Self, DomainError>;

    /// Convert to decimal string with 4 decimal places
    fn to_decimal_string(&self) -> String;

    /// Checked addition, returns None on overflow
    fn checked_add(&self, other: Self) -> Option<Self>;

    /// Checked subtraction, returns None on underflow
    fn checked_sub(&self, other: Self) -> Option<Self>;

    /// Zero value
    fn zero() -> Self;
}

/// Fixed-point decimal representation using i64 (multiply by 10,000)
/// Represents amounts with 4 decimal places of precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct FixedPoint(i64);

impl FixedPoint {
    const SCALE: i64 = 10_000;

    /// Create from raw scaled value (for internal use)
    pub fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    /// Get raw scaled value
    pub fn raw(&self) -> i64 {
        self.0
    }
}

impl AmountType for FixedPoint {
    fn from_decimal_str(s: &str) -> Result<Self, DomainError> {
        let s = s.trim();

        // Handle negative sign
        let (is_negative, s) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };

        // Split on decimal point
        let parts: Vec<&str> = s.split('.').collect();

        let (integer_part, decimal_part) = match parts.len() {
            1 => (parts[0], ""),
            2 => (parts[0], parts[1]),
            _ => return Err(DomainError::InvalidAmount),
        };

        // Validate decimal places (max 4)
        if decimal_part.len() > 4 {
            return Err(DomainError::InvalidAmount);
        }

        // Parse integer part
        let integer: i64 = integer_part
            .parse()
            .map_err(|_| DomainError::InvalidAmount)?;

        // Parse decimal part and pad to 4 digits
        let decimal_str = format!("{:0<4}", decimal_part);
        let decimal: i64 = decimal_str
            .parse()
            .map_err(|_| DomainError::InvalidAmount)?;

        // Combine: integer * 10000 + decimal
        let scaled = integer
            .checked_mul(Self::SCALE)
            .and_then(|v| v.checked_add(decimal))
            .ok_or(DomainError::Overflow)?;

        let result = if is_negative { -scaled } else { scaled };

        Ok(Self(result))
    }

    fn to_decimal_string(&self) -> String {
        let abs_value = self.0.abs();
        let integer_part = abs_value / Self::SCALE;
        let decimal_part = abs_value % Self::SCALE;

        let sign = if self.0 < 0 { "-" } else { "" };
        format!("{}{}.{:04}", sign, integer_part, decimal_part)
    }

    fn checked_add(&self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    fn checked_sub(&self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    fn zero() -> Self {
        Self(0)
    }
}

impl Add for FixedPoint {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for FixedPoint {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_integers() {
        assert_eq!(
            FixedPoint::from_decimal_str("1").unwrap(),
            FixedPoint(10_000)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("10").unwrap(),
            FixedPoint(100_000)
        );
        assert_eq!(FixedPoint::from_decimal_str("0").unwrap(), FixedPoint(0));
    }

    #[test]
    fn parse_decimals() {
        assert_eq!(
            FixedPoint::from_decimal_str("1.0").unwrap(),
            FixedPoint(10_000)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("1.5").unwrap(),
            FixedPoint(15_000)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("1.5000").unwrap(),
            FixedPoint(15_000)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("0.0001").unwrap(),
            FixedPoint(1)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("123.4567").unwrap(),
            FixedPoint(1_234_567)
        );
    }

    #[test]
    fn parse_with_whitespace() {
        assert_eq!(
            FixedPoint::from_decimal_str("  1.5  ").unwrap(),
            FixedPoint(15_000)
        );
    }

    #[test]
    fn parse_negative_amounts() {
        assert_eq!(
            FixedPoint::from_decimal_str("-1.5").unwrap(),
            FixedPoint(-15_000)
        );
        assert_eq!(
            FixedPoint::from_decimal_str("-10").unwrap(),
            FixedPoint(-100_000)
        );
    }

    #[test]
    fn reject_too_many_decimal_places() {
        assert!(FixedPoint::from_decimal_str("1.00001").is_err());
        assert!(FixedPoint::from_decimal_str("1.123456").is_err());
    }

    #[test]
    fn reject_invalid_formats() {
        assert!(FixedPoint::from_decimal_str("").is_err());
        assert!(FixedPoint::from_decimal_str("abc").is_err());
        assert!(FixedPoint::from_decimal_str("1.2.3").is_err());
        assert!(FixedPoint::from_decimal_str("1..2").is_err());
    }

    #[test]
    fn to_string_formats_correctly() {
        assert_eq!(FixedPoint(10_000).to_decimal_string(), "1.0000");
        assert_eq!(FixedPoint(15_000).to_decimal_string(), "1.5000");
        assert_eq!(FixedPoint(1).to_decimal_string(), "0.0001");
        assert_eq!(FixedPoint(0).to_decimal_string(), "0.0000");
        assert_eq!(FixedPoint(1_234_567).to_decimal_string(), "123.4567");
    }

    #[test]
    fn to_string_negative_amounts() {
        assert_eq!(FixedPoint(-15_000).to_decimal_string(), "-1.5000");
        assert_eq!(FixedPoint(-1).to_decimal_string(), "-0.0001");
    }

    #[test]
    fn round_trip_parsing() {
        let values = vec!["1.0000", "1.5000", "0.0001", "123.4567", "0.0000"];

        for val in values {
            let parsed = FixedPoint::from_decimal_str(val).unwrap();
            assert_eq!(parsed.to_decimal_string(), val);
        }
    }

    #[test]
    fn checked_add_works() {
        let a = FixedPoint(10_000);
        let b = FixedPoint(5_000);
        assert_eq!(a.checked_add(b), Some(FixedPoint(15_000)));
    }

    #[test]
    fn checked_add_detects_overflow() {
        let max = FixedPoint(i64::MAX);
        let one = FixedPoint(1);
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn checked_sub_works() {
        let a = FixedPoint(10_000);
        let b = FixedPoint(5_000);
        assert_eq!(a.checked_sub(b), Some(FixedPoint(5_000)));
    }

    #[test]
    fn checked_sub_detects_underflow() {
        let min = FixedPoint(i64::MIN);
        let one = FixedPoint(1);
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn zero_value() {
        assert_eq!(FixedPoint::zero(), FixedPoint(0));
    }

    #[test]
    fn add_operator() {
        let a = FixedPoint(10_000);
        let b = FixedPoint(5_000);
        assert_eq!(a + b, FixedPoint(15_000));
    }

    #[test]
    fn sub_operator() {
        let a = FixedPoint(10_000);
        let b = FixedPoint(5_000);
        assert_eq!(a - b, FixedPoint(5_000));
    }

    #[test]
    fn ordering_works() {
        assert!(FixedPoint(10_000) > FixedPoint(5_000));
        assert!(FixedPoint(5_000) < FixedPoint(10_000));
        assert!(FixedPoint(5_000) == FixedPoint(5_000));
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(FixedPoint::default(), FixedPoint(0));
    }
}
