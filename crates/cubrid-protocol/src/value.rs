//! Value type for representing CUBRID data values.
//!
//! [`Value`] is the Rust representation of a CUBRID column value or bind
//! parameter.  It covers all the scalar types the CAS wire protocol supports.

use std::fmt;

/// A CUBRID data value.
///
/// Used for:
/// - Bind parameters in [`crate::request::encode_bind_params`]
/// - Decoded column values from [`crate::response`]
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL.
    Null,
    /// Boolean value — encoded as `SMALLINT` on the wire (0/1).
    Bool(bool),
    /// 16-bit signed integer (`SMALLINT`).
    Short(i16),
    /// 32-bit signed integer (`INTEGER`).
    Int(i32),
    /// 64-bit signed integer (`BIGINT`).
    Long(i64),
    /// 32-bit IEEE float.
    Float(f32),
    /// 64-bit IEEE double.
    Double(f64),
    /// Character string (`CHAR`, `VARCHAR`, `STRING`, `NCHAR`, `VARNCHAR`, `NUMERIC`, `ENUM`).
    String(String),
    /// Raw byte buffer (`BIT`, `VARBIT`, `BLOB`, `CLOB`).
    Bytes(Vec<u8>),
    /// Date value (year, month, day).
    Date {
        /// Year component.
        year: i16,
        /// Month component (1–12).
        month: i16,
        /// Day component (1–31).
        day: i16,
    },
    /// Time value (hour, minute, second).
    Time {
        /// Hour component (0–23).
        hour: i16,
        /// Minute component (0–59).
        minute: i16,
        /// Second component (0–59).
        second: i16,
    },
    /// Timestamp value (date + time, no milliseconds).
    Timestamp {
        /// Year component.
        year: i16,
        /// Month component (1–12).
        month: i16,
        /// Day component (1–31).
        day: i16,
        /// Hour component (0–23).
        hour: i16,
        /// Minute component (0–59).
        minute: i16,
        /// Second component (0–59).
        second: i16,
    },
    /// Datetime value (date + time with milliseconds).
    Datetime {
        /// Year component.
        year: i16,
        /// Month component (1–12).
        month: i16,
        /// Day component (1–31).
        day: i16,
        /// Hour component (0–23).
        hour: i16,
        /// Minute component (0–59).
        minute: i16,
        /// Second component (0–59).
        second: i16,
        /// Millisecond component (0–999).
        ms: i16,
    },
}

impl Value {
    /// Returns `true` if this value is [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Try to extract an `i64` from integer-like variants.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Short(v) => Some(*v as i64),
            Self::Int(v) => Some(*v as i64),
            Self::Long(v) => Some(*v),
            Self::Bool(v) => Some(if *v { 1 } else { 0 }),
            _ => None,
        }
    }

    /// Try to extract an `f64` from float-like variants.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v as f64),
            Self::Double(v) => Some(*v),
            _ => None,
        }
    }

    /// Try to extract a string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Try to extract a byte-slice reference.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(v) => Some(v),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "NULL"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Short(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Long(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Double(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v}"),
            Self::Bytes(v) => write!(f, "<{} bytes>", v.len()),
            Self::Date { year, month, day } => write!(f, "{year:04}-{month:02}-{day:02}"),
            Self::Time {
                hour,
                minute,
                second,
            } => write!(f, "{hour:02}:{minute:02}:{second:02}"),
            Self::Timestamp {
                year,
                month,
                day,
                hour,
                minute,
                second,
            } => write!(
                f,
                "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
            ),
            Self::Datetime {
                year,
                month,
                day,
                hour,
                minute,
                second,
                ms,
            } => write!(
                f,
                "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{ms:03}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience From impls
// ---------------------------------------------------------------------------

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Self::Short(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Long(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Self::Float(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Double(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => Self::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null() {
        let v = Value::Null;
        assert!(v.is_null());
        assert_eq!(format!("{v}"), "NULL");
    }

    #[test]
    fn test_bool() {
        let v = Value::Bool(true);
        assert!(!v.is_null());
        assert_eq!(v.as_i64(), Some(1));
        assert_eq!(format!("{v}"), "true");
    }

    #[test]
    fn test_short() {
        let v = Value::Short(42);
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn test_int() {
        let v = Value::Int(100_000);
        assert_eq!(v.as_i64(), Some(100_000));
    }

    #[test]
    fn test_long() {
        let v = Value::Long(i64::MAX);
        assert_eq!(v.as_i64(), Some(i64::MAX));
    }

    #[test]
    fn test_float() {
        let v = Value::Float(3.14);
        assert!(v.as_f64().is_some());
        assert!((v.as_f64().unwrap() - 3.14).abs() < 0.01);
    }

    #[test]
    fn test_double() {
        let v = Value::Double(3.14159);
        assert!((v.as_f64().unwrap() - 3.14159).abs() < 1e-10);
    }

    #[test]
    fn test_string() {
        let v = Value::String("hello".to_string());
        assert_eq!(v.as_str(), Some("hello"));
        assert_eq!(format!("{v}"), "hello");
    }

    #[test]
    fn test_bytes() {
        let v = Value::Bytes(vec![0xDE, 0xAD]);
        assert_eq!(v.as_bytes(), Some([0xDE, 0xAD].as_slice()));
        assert_eq!(format!("{v}"), "<2 bytes>");
    }

    #[test]
    fn test_date_display() {
        let v = Value::Date {
            year: 2024,
            month: 3,
            day: 15,
        };
        assert_eq!(format!("{v}"), "2024-03-15");
    }

    #[test]
    fn test_time_display() {
        let v = Value::Time {
            hour: 10,
            minute: 30,
            second: 45,
        };
        assert_eq!(format!("{v}"), "10:30:45");
    }

    #[test]
    fn test_timestamp_display() {
        let v = Value::Timestamp {
            year: 2024,
            month: 3,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
        };
        assert_eq!(format!("{v}"), "2024-03-15 10:30:45");
    }

    #[test]
    fn test_datetime_display() {
        let v = Value::Datetime {
            year: 2024,
            month: 3,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            ms: 123,
        };
        assert_eq!(format!("{v}"), "2024-03-15 10:30:45.123");
    }

    #[test]
    fn test_from_i32() {
        let v: Value = 42i32.into();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn test_from_str() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::String("hello".to_string()));
    }

    #[test]
    fn test_from_option_some() {
        let v: Value = Some(42i32).into();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn test_from_option_none() {
        let v: Value = Option::<i32>::None.into();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn test_as_i64_returns_none_for_string() {
        let v = Value::String("not a number".to_string());
        assert_eq!(v.as_i64(), None);
    }

    #[test]
    fn test_as_f64_returns_none_for_string() {
        let v = Value::String("not a number".to_string());
        assert_eq!(v.as_f64(), None);
    }

    #[test]
    fn test_as_str_returns_none_for_int() {
        let v = Value::Int(42);
        assert_eq!(v.as_str(), None);
    }

    #[test]
    fn test_as_bytes_returns_none_for_string() {
        let v = Value::String("hello".to_string());
        assert_eq!(v.as_bytes(), None);
    }

    #[test]
    fn test_clone_and_eq() {
        let v1 = Value::Datetime {
            year: 2024,
            month: 3,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            ms: 123,
        };
        let v2 = v1.clone();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_display_short() {
        assert_eq!(format!("{}", Value::Short(42)), "42");
    }

    #[test]
    fn test_display_float() {
        let s = format!("{}", Value::Float(1.5));
        assert!(s.starts_with("1.5"), "Float display: {s}");
    }

    #[test]
    fn test_display_bool_false() {
        assert_eq!(format!("{}", Value::Bool(false)), "false");
    }

    #[test]
    fn test_as_i64_bool_false() {
        assert_eq!(Value::Bool(false).as_i64(), Some(0));
    }

    #[test]
    fn test_as_f64_returns_none_for_int() {
        assert_eq!(Value::Int(42).as_f64(), None);
    }

    #[test]
    fn test_as_str_returns_none_for_null() {
        assert_eq!(Value::Null.as_str(), None);
    }

    #[test]
    fn test_as_bytes_returns_none_for_null() {
        assert_eq!(Value::Null.as_bytes(), None);
    }

    #[test]
    fn test_display_long() {
        assert_eq!(format!("{}", Value::Long(999_999)), "999999");
    }

    #[test]
    fn test_display_double() {
        let s = format!("{}", Value::Double(2.5));
        assert!(s.starts_with("2.5"), "Double display: {s}");
    }

    #[test]
    fn test_display_bytes_empty() {
        assert_eq!(format!("{}", Value::Bytes(vec![])), "<0 bytes>");
    }

    #[test]
    fn test_from_bool() {
        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));
        let v: Value = false.into();
        assert_eq!(v, Value::Bool(false));
    }

    #[test]
    fn test_from_i16() {
        let v: Value = 100i16.into();
        assert_eq!(v, Value::Short(100));
    }

    #[test]
    fn test_from_i64() {
        let v: Value = 42i64.into();
        assert_eq!(v, Value::Long(42));
    }

    #[test]
    fn test_from_f32() {
        let v: Value = 1.5f32.into();
        assert_eq!(v, Value::Float(1.5));
    }

    #[test]
    fn test_from_f64() {
        let v: Value = 3.14f64.into();
        assert_eq!(v, Value::Double(3.14));
    }

    #[test]
    fn test_from_string() {
        let v: Value = String::from("owned").into();
        assert_eq!(v, Value::String("owned".to_string()));
    }

    #[test]
    fn test_from_vec_u8() {
        let v: Value = vec![1u8, 2, 3].into();
        assert_eq!(v, Value::Bytes(vec![1, 2, 3]));
    }

    #[test]
    fn test_is_null() {
        assert!(Value::Null.is_null());
        assert!(!Value::Int(0).is_null());
        assert!(!Value::Bool(false).is_null());
        assert!(!Value::String(String::new()).is_null());
    }

    #[test]
    fn test_as_i64_short() {
        assert_eq!(Value::Short(i16::MAX).as_i64(), Some(i16::MAX as i64));
        assert_eq!(Value::Short(i16::MIN).as_i64(), Some(i16::MIN as i64));
    }

    #[test]
    fn test_as_f64_float() {
        let v = Value::Float(1.0);
        assert!((v.as_f64().unwrap() - 1.0).abs() < f64::EPSILON);
    }
}
