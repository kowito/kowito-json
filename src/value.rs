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
