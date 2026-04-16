//! Full-document JSON `Value` type, analogous to `serde_json::Value`.

use crate::error::{Error, Result};
use crate::parse::{Deserialize, Parser, byte_offset_to_line_col};
use crate::scanner::{
    OFFSET_MASK, TOKEN_LBRACE, TOKEN_LBRACKET, TOKEN_QUOTE, TOKEN_RBRACE, TOKEN_RBRACKET,
};

/// A dynamically-typed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Number stored as the raw JSON source string to preserve integer precision.
    Number(String),
    Str(String),
    Array(Vec<Value>),
    /// JSON object as ordered key-value pairs.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// Return `true` if this value is `null`.
    pub fn is_null(&self) -> bool { matches!(self, Value::Null) }

    /// Try to get a reference to the value at a given object key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        if let Value::Object(pairs) = self {
            pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }

    /// Try to get a reference to the value at a given array index.
    pub fn index(&self, i: usize) -> Option<&Value> {
        if let Value::Array(arr) = self { arr.get(i) } else { None }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => f.write_str(n),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Array(arr) => {
                f.write_str("[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { f.write_str(",")?; }
                    write!(f, "{v}")?;
                }
                f.write_str("]")
            }
            Value::Object(pairs) => {
                f.write_str("{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 { f.write_str(",")?; }
                    write!(f, "{k:?}:{v}")?;
                }
                f.write_str("}")
            }
        }
    }
}

impl Deserialize for Value {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        let tag = parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK);

        match tag {
            // Object
            Some(t) if t == TOKEN_LBRACE => {
                parser.pos += 1; // consume '{'
                let mut pairs: Vec<(String, Value)> = Vec::new();
                // Empty object
                if parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK) == Some(TOKEN_RBRACE) {
                    parser.pos += 1;
                    return Ok(Value::Object(pairs));
                }
                loop {
                    let key = parser.parse_string_owned()?;
                    parser.expect_colon()?;
                    let val = Value::deserialize(parser)?;
                    pairs.push((key, val));
                    if !parser.object_next()? {
                        break;
                    }
                }
                Ok(Value::Object(pairs))
            }
            // Array
            Some(t) if t == TOKEN_LBRACKET => {
                parser.pos += 1; // consume '['
                let mut items: Vec<Value> = Vec::new();
                // Empty array
                if parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK) == Some(TOKEN_RBRACKET) {
                    parser.pos += 1;
                    return Ok(Value::Array(items));
                }
                loop {
                    items.push(Value::deserialize(parser)?);
                    if !parser.array_next()? {
                        break;
                    }
                }
                Ok(Value::Array(items))
            }
            // String
            Some(t) if t == TOKEN_QUOTE => {
                Ok(Value::Str(parser.parse_string_owned()?))
            }
            // Primitive: null / true / false / number — read raw bytes
            _ => {
                let start = parser.current_value_start()?;
                let raw = &parser.src[start..];
                if raw.starts_with(b"null") {
                    Ok(Value::Null)
                } else if raw.starts_with(b"true") {
                    Ok(Value::Bool(true))
                } else if raw.starts_with(b"false") {
                    Ok(Value::Bool(false))
                } else if raw.first().map(|&b| b == b'-' || b.is_ascii_digit()).unwrap_or(false) {
                    let end = parser.scan_number_end_pub(start);
                    let num = std::str::from_utf8(&parser.src[start..end])
                        .map_err(|_| Error::custom("non-UTF-8 in number"))?
                        .to_owned();
                    Ok(Value::Number(num))
                } else {
                    let (line, col) = byte_offset_to_line_col(parser.src, start);
                    Err(Error::parse_at(
                        format!("unexpected byte {:?}", raw.first()),
                        line, col,
                    ))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// From<T> conversions — used by the `json!` macro fallback arm
// ---------------------------------------------------------------------------

impl From<bool> for Value {
    fn from(b: bool) -> Self { Value::Bool(b) }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self { Value::Str(s.to_owned()) }
}

impl From<String> for Value {
    fn from(s: String) -> Self { Value::Str(s) }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self { Value::Str(s.clone()) }
}

macro_rules! impl_value_from_int {
    ($($t:ty),*) => {
        $(impl From<$t> for Value {
            fn from(n: $t) -> Self { Value::Number(n.to_string()) }
        })*
    };
}
impl_value_from_int!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl From<f32> for Value {
    fn from(n: f32) -> Self { Value::Number(n.to_string()) }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self { Value::Number(n.to_string()) }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self { Value::Array(v.into_iter().map(Into::into).collect()) }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}

// ---------------------------------------------------------------------------
// `json!` macro — construct a `Value` with JSON-like literal syntax.
//
// Supports:
//   json!(null)                          → Value::Null
//   json!(true) / json!(false)           → Value::Bool
//   json!(42) / json!(3.14)              → Value::Number
//   json!("hello")                       → Value::Str
//   json!([1, "two", null])              → Value::Array
//   json!({ "key": "value", "n": 1 })   → Value::Object
//   json!((some_rust_expr))              → Value::from(expr)
// ---------------------------------------------------------------------------

/// Construct a [`Value`] with JSON-like syntax.
///
/// # Examples
/// ```rust
/// use kowito_json::{json, Value};
///
/// let v = json!({
///     "name": "Alice",
///     "age": 30,
///     "active": true,
///     "tags": ["admin", "user"],
///     "score": null
/// });
/// assert!(matches!(v, Value::Object(_)));
/// ```
#[macro_export]
macro_rules! json {
    // ---- atoms ----
    (null)  => { $crate::Value::Null };
    (true)  => { $crate::Value::Bool(true) };
    (false) => { $crate::Value::Bool(false) };

    // ---- empty containers ----
    ([]) => { $crate::Value::Array(::std::vec![]) };
    ({}) => { $crate::Value::Object(::std::vec![]) };

    // ---- array ----
    ([ $($elem:tt),+ $(,)? ]) => {
        $crate::Value::Array(::std::vec![ $( $crate::json!($elem) ),+ ])
    };

    // ---- object ----
    ({ $($key:tt : $val:tt),+ $(,)? }) => {
        $crate::Value::Object(::std::vec![
            $( (::std::string::String::from($key), $crate::json!($val)) ),+
        ])
    };

    // ---- fallback: any Rust expression ----
    // Wrap complex expressions in parens: json!((some + expr))
    ($other:expr) => {
        $crate::Value::from($other)
    };
}
