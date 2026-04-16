//! # JSON Parsing / Deserialization
//!
//! This module provides a zero-copy, SIMD-accelerated JSON parser that
//! deserializes JSON bytes into Rust types via the [`Deserialize`] trait.
//!
//! ## Usage
//! ```ignore
//! use kowito_json::{from_str, from_slice};
//! use kowito_json_derive::KJson;
//!
//! #[derive(KJson, Debug)]
//! struct User { id: u64, name: String, active: bool }
//!
//! let user: User = from_str(r#"{"id":1,"name":"Alice","active":true}"#).unwrap();
//! ```

use crate::error::{Error, Result};
use crate::scanner::{Scanner, OFFSET_MASK, TOKEN_COLON, TOKEN_LBRACE, TOKEN_LBRACKET, TOKEN_QUOTE, TOKEN_RBRACE, TOKEN_RBRACKET};
use crate::string::KString;
use std::collections::{BTreeMap, HashMap};

// ---------------------------------------------------------------------------
// Deserialize trait
// ---------------------------------------------------------------------------

/// Types that can be deserialized from JSON.
pub trait Deserialize: Sized {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self>;
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Streaming JSON parser backed by the SIMD tape scanner.
pub struct Parser<'a> {
    pub src: &'a [u8],
    pub tape: Vec<u32>,
    pub pos: usize, // current tape index
}

impl<'a> Parser<'a> {
    /// Create a new parser from a byte slice.
    pub fn new(src: &'a [u8]) -> Self {
        let mut tape = vec![0u32; src.len() + 8];
        let n = Scanner::new(src).scan(&mut tape);
        tape.truncate(n);
        Parser { src, tape, pos: 0 }
    }

    // --- Low-level helpers ---

    #[inline]
    fn peek_tag(&self) -> Option<u32> {
        self.tape.get(self.pos).map(|t| t & !OFFSET_MASK)
    }

    #[inline]
    fn peek_offset(&self) -> Option<usize> {
        self.tape.get(self.pos).map(|t| (t & OFFSET_MASK) as usize)
    }

    #[inline]
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Skip optional whitespace (not needed since the tape strips whitespace).
    #[inline]
    fn skip_ws(&mut self) {}

    /// Expect a specific tag, advance, and return the byte offset.
    fn expect_tag(&mut self, expected: u32, label: &'static str) -> Result<usize> {
        match self.tape.get(self.pos) {
            Some(&t) if (t & !OFFSET_MASK) == expected => {
                let off = (t & OFFSET_MASK) as usize;
                self.pos += 1;
                Ok(off)
            }
            other => {
                let byte_off = other.map(|&t| (t & OFFSET_MASK) as usize).unwrap_or(self.src.len());
                let (line, col) = crate::parse::byte_offset_to_line_col(self.src, byte_off);
                Err(Error::parse_at(
                    format!("expected {label}, got {:?}", other.map(|t| t & !OFFSET_MASK)),
                    line,
                    col,
                ))
            }
        }
    }

    // --- String parsing ---

    /// Parse a JSON string. The tape has two TOKEN_QUOTE entries: open and close.
    pub fn parse_string(&mut self) -> Result<KString<'a>> {
        let open_off = self.expect_tag(TOKEN_QUOTE, "opening '\"'")?;
        let close_entry = *self.tape.get(self.pos).ok_or_else(|| {
            Error::custom("unexpected end of tape after opening '\"'")
        })?;
        if (close_entry & !OFFSET_MASK) != TOKEN_QUOTE {
            return Err(Error::custom("expected closing '\"'"));
        }
        let close_off = (close_entry & OFFSET_MASK) as usize;
        self.pos += 1;

        let raw = &self.src[open_off + 1..close_off];
        let has_escapes = raw.iter().any(|&b| b == b'\\');
        Ok(KString::new(raw, has_escapes))
    }

    /// Parse and return an owned String.
    pub fn parse_string_owned(&mut self) -> Result<String> {
        let s = self.parse_string()?;
        Ok(s.decode().into_owned())
    }

    // --- Number parsing ---

    /// Consume raw bytes for the current number value.
    fn parse_number_bytes(&mut self) -> Result<&'a [u8]> {
        // Numbers are not tracked in the tape — we read ahead from the current src offset.
        // The tape pos points to either the next structural or end.
        // The number starts at the offset of the *previous* structural + 1 (after ':' or ',' or '[').
        // Strategy: derive the start from src position after the last consumed token.
        let start = self.current_value_start()?;
        let end = self.scan_number_end(start);
        Ok(&self.src[start..end])
    }

    /// Return the byte offset where the next value starts (after structural separator).
    pub fn current_value_start(&self) -> Result<usize> {
        // The value is immediately after the most recently consumed token.
        // We track this by looking backwards in the tape for the last consumed entry.
        if self.pos == 0 {
            return Ok(0);
        }
        let prev = self.tape[self.pos - 1];
        let prev_off = (prev & OFFSET_MASK) as usize;
        // Skip whitespace from prev_off + 1
        let mut i = prev_off + 1;
        while i < self.src.len() && matches!(self.src[i], b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        }
        Ok(i)
    }

    fn scan_number_end(&self, start: usize) -> usize {
        let mut i = start;
        while i < self.src.len()
            && !matches!(self.src[i], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
        {
            i += 1;
        }
        i
    }

    /// Public accessor for `scan_number_end` (used by `Value` deserializer).
    pub fn scan_number_end_pub(&self, start: usize) -> usize {
        self.scan_number_end(start)
    }

    // --- Value-level parsing ---

    /// Parse a `null` literal; returns true if null was consumed.
    pub fn parse_null(&mut self) -> Result<bool> {
        let start = self.current_value_start()?;
        if self.src.get(start..start + 4) == Some(b"null") {
            // Advance tape past any trailing structural
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse a boolean literal.
    pub fn parse_bool(&mut self) -> Result<bool> {
        let start = self.current_value_start()?;
        if self.src.get(start..start + 4) == Some(b"true") {
            Ok(true)
        } else if self.src.get(start..start + 5) == Some(b"false") {
            Ok(false)
        } else {
            Err(Error::custom(format!(
                "expected boolean at offset {start}, got {:?}",
                self.src.get(start..start + 5).map(|s| std::str::from_utf8(s))
            )))
        }
    }

    // --- Object helpers ---

    /// Expect `{` — returns offset.
    pub fn begin_object(&mut self) -> Result<usize> {
        self.expect_tag(TOKEN_LBRACE, "'{'")
    }

    /// Returns true if the next token is `}`, and consumes it.
    pub fn end_object_or_comma(&mut self) -> Result<ObjectStep> {
        match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
            Some(t) if t == TOKEN_RBRACE => {
                self.pos += 1;
                Ok(ObjectStep::End)
            }
            Some(t) if t == TOKEN_COLON => {
                // shouldn't happen here
                Ok(ObjectStep::Continue)
            }
            _ => Ok(ObjectStep::Continue),
        }
    }

    /// After reading a key, skip `:` colon.
    pub fn expect_colon(&mut self) -> Result<()> {
        self.expect_tag(TOKEN_COLON, "':'").map(|_| ())
    }

    /// After reading a value in an object, look for `,` (advance) or `}` (stop).
    pub fn object_next(&mut self) -> Result<bool> {
        match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
            Some(t) if t == TOKEN_RBRACE => {
                self.pos += 1;
                Ok(false) // done
            }
            Some(t) if t == (7 << 28) => {
                // TOKEN_COMMA
                self.pos += 1;
                Ok(true) // more fields
            }
            _ => Err(Error::custom(format!(
                "expected ',' or '}}' at tape[{}]",
                self.pos
            ))),
        }
    }

    // --- Array helpers ---

    pub fn begin_array(&mut self) -> Result<()> {
        self.expect_tag(TOKEN_LBRACKET, "'['").map(|_| ())
    }

    /// Returns true if more items remain (advances past `,`), false on `]`.
    pub fn array_next(&mut self) -> Result<bool> {
        match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
            Some(t) if t == TOKEN_RBRACKET => {
                self.pos += 1;
                Ok(false)
            }
            Some(t) if t == (7 << 28) => {
                self.pos += 1;
                Ok(true)
            }
            _ => Err(Error::custom(format!(
                "expected ',' or ']' at tape[{}]",
                self.pos
            ))),
        }
    }

    // --- Check what value is next ---
    pub fn peek_is_string(&self) -> bool {
        matches!(self.peek_tag(), Some(t) if t == TOKEN_QUOTE)
    }

    pub fn peek_is_object(&self) -> bool {
        matches!(self.peek_tag(), Some(t) if t == TOKEN_LBRACE)
    }

    pub fn peek_is_array(&self) -> bool {
        matches!(self.peek_tag(), Some(t) if t == TOKEN_LBRACKET)
    }

    /// Skip a single value (object, array, string, or primitive).
    pub fn skip_value(&mut self) -> Result<()> {
        match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
            Some(t) if t == TOKEN_QUOTE => {
                // string: consume open + close quote
                self.pos += 2;
            }
            Some(t) if t == TOKEN_LBRACE => {
                self.pos += 1;
                // skip until matching }
                let mut depth = 1usize;
                while depth > 0 {
                    match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
                        Some(t) if t == TOKEN_LBRACE => {
                            depth += 1;
                            self.pos += 1;
                        }
                        Some(t) if t == TOKEN_RBRACE => {
                            depth -= 1;
                            self.pos += 1;
                        }
                        None => {
                            return Err(Error::custom("unexpected end of tape in object skip"))
                        }
                        _ => {
                            self.pos += 1;
                        }
                    }
                }
            }
            Some(t) if t == TOKEN_LBRACKET => {
                self.pos += 1;
                let mut depth = 1usize;
                while depth > 0 {
                    match self.tape.get(self.pos).map(|t| t & !OFFSET_MASK) {
                        Some(t) if t == TOKEN_LBRACKET => {
                            depth += 1;
                            self.pos += 1;
                        }
                        Some(t) if t == TOKEN_RBRACKET => {
                            depth -= 1;
                            self.pos += 1;
                        }
                        None => {
                            return Err(Error::custom("unexpected end of tape in array skip"))
                        }
                        _ => {
                            self.pos += 1;
                        }
                    }
                }
            }
            _ => {
                // primitive: already consumed by prior structural, nothing in tape
            }
        }
        Ok(())
    }

    // --- Typed parsers for primitives ---

    pub fn parse_i64(&mut self) -> Result<i64> {
        let bytes = self.parse_number_bytes()?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error::custom("non-utf8 number"))?;
        s.parse::<i64>().map_err(|e| Error::custom(format!("invalid i64: {e}")))
    }

    pub fn parse_u64(&mut self) -> Result<u64> {
        let bytes = self.parse_number_bytes()?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error::custom("non-utf8 number"))?;
        s.parse::<u64>().map_err(|e| Error::custom(format!("invalid u64: {e}")))
    }

    pub fn parse_f64(&mut self) -> Result<f64> {
        let bytes = self.parse_number_bytes()?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error::custom("non-utf8 number"))?;
        s.parse::<f64>().map_err(|e| Error::custom(format!("invalid f64: {e}")))
    }

    pub fn parse_f32(&mut self) -> Result<f32> {
        let bytes = self.parse_number_bytes()?;
        let s = std::str::from_utf8(bytes).map_err(|_| Error::custom("non-utf8 number"))?;
        s.parse::<f32>().map_err(|e| Error::custom(format!("invalid f32: {e}")))
    }
}

// ---------------------------------------------------------------------------
// ObjectStep helper
// ---------------------------------------------------------------------------

pub enum ObjectStep {
    Continue,
    End,
}

// ---------------------------------------------------------------------------
// Deserialize implementations for standard types
// ---------------------------------------------------------------------------

impl Deserialize for bool {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.parse_bool()
    }
}

macro_rules! impl_deser_int {
    ($($t:ty => $parse:ident),* $(,)?) => {
        $(impl Deserialize for $t {
            fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
                let v = parser.$parse()?;
                v.try_into().map_err(|_| Error::custom(concat!("overflow converting to ", stringify!($t))))
            }
        })*
    };
}

impl_deser_int! {
    i8   => parse_i64,
    i16  => parse_i64,
    i32  => parse_i64,
    i64  => parse_i64,
    i128 => parse_i64,
    isize => parse_i64,
    u8   => parse_u64,
    u16  => parse_u64,
    u32  => parse_u64,
    u64  => parse_u64,
    u128 => parse_u64,
    usize => parse_u64,
}

impl Deserialize for f32 {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.parse_f32()
    }
}

impl Deserialize for f64 {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.parse_f64()
    }
}

impl Deserialize for String {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.parse_string_owned()
    }
}

impl<'b> Deserialize for std::borrow::Cow<'b, str> {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.parse_string_owned().map(std::borrow::Cow::Owned)
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        if parser.parse_null()? {
            Ok(None)
        } else {
            Ok(Some(T::deserialize(parser)?))
        }
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.begin_array()?;
        let mut items = Vec::new();

        // Check immediately for empty array
        if let Some(t) = parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK) {
            if t == TOKEN_RBRACKET {
                parser.pos += 1;
                return Ok(items);
            }
        }

        loop {
            items.push(T::deserialize(parser)?);
            if !parser.array_next()? {
                break;
            }
        }
        Ok(items)
    }
}

impl<V: Deserialize> Deserialize for HashMap<String, V> {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.begin_object()?;
        let mut map = HashMap::new();
        // Empty object check
        if parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK) == Some(TOKEN_RBRACE) {
            parser.pos += 1;
            return Ok(map);
        }
        loop {
            let key = parser.parse_string_owned()?;
            parser.expect_colon()?;
            let val = V::deserialize(parser)?;
            map.insert(key, val);
            if !parser.object_next()? {
                break;
            }
        }
        Ok(map)
    }
}

impl<V: Deserialize> Deserialize for BTreeMap<String, V> {
    fn deserialize(parser: &mut Parser<'_>) -> Result<Self> {
        parser.begin_object()?;
        let mut map = BTreeMap::new();
        if parser.tape.get(parser.pos).map(|t| t & !OFFSET_MASK) == Some(TOKEN_RBRACE) {
            parser.pos += 1;
            return Ok(map);
        }
        loop {
            let key = parser.parse_string_owned()?;
            parser.expect_colon()?;
            let val = V::deserialize(parser)?;
            map.insert(key, val);
            if !parser.object_next()? {
                break;
            }
        }
        Ok(map)
    }
}

// ---------------------------------------------------------------------------
// Line/column helper
// ---------------------------------------------------------------------------

/// Convert a byte offset in `src` to a (1-based line, 1-based column) pair.
pub fn byte_offset_to_line_col(src: &[u8], offset: usize) -> (usize, usize) {
    let safe = offset.min(src.len());
    let line = src[..safe].iter().filter(|&&b| b == b'\n').count() + 1;
    let col = match src[..safe].iter().rposition(|&b| b == b'\n') {
        Some(nl) => safe - nl,
        None => safe + 1,
    };
    (line, col)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Deserialize a type from a JSON byte slice.
pub fn from_slice<T: Deserialize>(src: &[u8]) -> Result<T> {
    // Validate UTF-8 at the boundary before parsing.
    std::str::from_utf8(src).map_err(|e| Error::custom(format!("invalid UTF-8: {e}")))?;
    let mut parser = Parser::new(src);
    T::deserialize(&mut parser)
}

/// Deserialize a type from a JSON string.
pub fn from_str<T: Deserialize>(s: &str) -> Result<T> {
    // str is already valid UTF-8; skip the check.
    let mut parser = Parser::new(s.as_bytes());
    T::deserialize(&mut parser)
}
