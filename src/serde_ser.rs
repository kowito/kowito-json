//! `serde`-compatible serializer backend for kowito-json.
//!
//! Any type implementing `serde::Serialize` can be serialized to JSON using
//! kowito-json's SIMD-accelerated string escape path and `itoa`/`ryu` numeric
//! formatting. The API mirrors `serde_json`:
//!
//! ```rust,ignore
//! use kowito_json::{to_string, to_string_pretty, to_writer};
//!
//! #[derive(serde::Serialize)]
//! struct User { id: u64, name: String }
//!
//! let u = User { id: 1, name: "Alice".into() };
//! let json = to_string(&u)?;
//! ```

use crate::error::{Error, Result};
use crate::serialize::write_str_escape_writer;
use serde::ser::{self, Serialize};
use std::io;

// ---------------------------------------------------------------------------
// Formatter trait
// ---------------------------------------------------------------------------

/// Controls the whitespace emitted around JSON structural tokens.
///
/// Two built-in implementations are provided:
/// - [`CompactFormatter`] — no extra whitespace (default).
/// - [`PrettyFormatter`] — indented output with configurable indent string.
pub trait Formatter {
    fn begin_array<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b"[")
    }
    fn end_array<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b"]")
    }
    fn begin_array_value<W: io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if !first {
            w.write_all(b",")?;
        }
        Ok(())
    }
    fn end_array_value<W: io::Write>(&mut self, _w: &mut W) -> io::Result<()> {
        Ok(())
    }

    fn begin_object<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b"{")
    }
    fn end_object<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b"}")
    }
    fn begin_object_key<W: io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if !first {
            w.write_all(b",")?;
        }
        Ok(())
    }
    fn end_object_key<W: io::Write>(&mut self, _w: &mut W) -> io::Result<()> {
        Ok(())
    }
    fn begin_object_value<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b":")
    }
    fn end_object_value<W: io::Write>(&mut self, _w: &mut W) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CompactFormatter
// ---------------------------------------------------------------------------

/// Emits JSON with no additional whitespace (the default).
pub struct CompactFormatter;

impl Formatter for CompactFormatter {}

// ---------------------------------------------------------------------------
// PrettyFormatter
// ---------------------------------------------------------------------------

/// Emits JSON with newlines and indentation.
pub struct PrettyFormatter {
    /// Bytes used as one level of indentation (e.g. `b"  "` for two spaces).
    indent: &'static [u8],
    current_indent: usize,
    /// True immediately after opening a `{` or `[` that had at least one item.
    has_value: bool,
}

impl PrettyFormatter {
    /// Create a pretty-formatter with the given indent string.
    pub fn with_indent(indent: &'static [u8]) -> Self {
        Self { indent, current_indent: 0, has_value: false }
    }

    fn write_indent<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        for _ in 0..self.current_indent {
            w.write_all(self.indent)?;
        }
        Ok(())
    }
}

impl Default for PrettyFormatter {
    fn default() -> Self {
        Self::with_indent(b"  ")
    }
}

impl Formatter for PrettyFormatter {
    fn begin_array<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        self.has_value = false;
        w.write_all(b"[")
    }
    fn end_array<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        if self.has_value {
            w.write_all(b"\n")?;
            self.write_indent(w)?;
        }
        w.write_all(b"]")
    }
    fn begin_array_value<W: io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if first {
            w.write_all(b"\n")?;
        } else {
            w.write_all(b",\n")?;
        }
        self.has_value = true;
        self.write_indent(w)
    }

    fn begin_object<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        self.has_value = false;
        w.write_all(b"{")
    }
    fn end_object<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        if self.has_value {
            w.write_all(b"\n")?;
            self.write_indent(w)?;
        }
        w.write_all(b"}")
    }
    fn begin_object_key<W: io::Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if first {
            w.write_all(b"\n")?;
        } else {
            w.write_all(b",\n")?;
        }
        self.has_value = true;
        self.write_indent(w)
    }
    fn begin_object_value<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }
}

// ---------------------------------------------------------------------------
// KowitoSerializer
// ---------------------------------------------------------------------------

/// Core serializer. Wraps a writer `W` and a formatter `F`.
pub struct KowitoSerializer<W: io::Write, F: Formatter> {
    pub(crate) writer: W,
    pub(crate) formatter: F,
}

impl<W: io::Write, F: Formatter> KowitoSerializer<W, F> {
    pub fn new(writer: W, formatter: F) -> Self {
        Self { writer, formatter }
    }

    #[inline]
    fn write_bytes(&mut self, b: &[u8]) -> Result<()> {
        self.writer.write_all(b).map_err(Error::Io)
    }
}

// ---------------------------------------------------------------------------
// serde::Serializer for &mut KowitoSerializer<W, F>
// ---------------------------------------------------------------------------

impl<'a, W: io::Write, F: Formatter> ser::Serializer for &'a mut KowitoSerializer<W, F> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a, W, F>;
    type SerializeTuple = SerializeSeq<'a, W, F>;
    type SerializeTupleStruct = SerializeSeq<'a, W, F>;
    type SerializeTupleVariant = SerializeTupleVariant<'a, W, F>;
    type SerializeMap = SerializeMap<'a, W, F>;
    type SerializeStruct = SerializeMap<'a, W, F>;
    type SerializeStructVariant = SerializeStructVariant<'a, W, F>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.write_bytes(if v { b"true" } else { b"false" })
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_i16(self, v: i16) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_i32(self, v: i32) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_i64(self, v: i64) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_i128(self, v: i128) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_u8(self, v: u8) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_u16(self, v: u16) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_u32(self, v: u32) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_u64(self, v: u64) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }
    #[inline]
    fn serialize_u128(self, v: u128) -> Result<()> {
        let mut buf = itoa::Buffer::new();
        self.write_bytes(buf.format(v).as_bytes())
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<()> {
        if v.is_finite() {
            let mut buf = ryu::Buffer::new();
            self.write_bytes(buf.format_finite(v).as_bytes())
        } else {
            // serde_json also rejects non-finite floats; we match that behaviour.
            Err(Error::Custom("float value is infinite or NaN".into()))
        }
    }
    #[inline]
    fn serialize_f64(self, v: f64) -> Result<()> {
        if v.is_finite() {
            let mut buf = ryu::Buffer::new();
            self.write_bytes(buf.format_finite(v).as_bytes())
        } else {
            Err(Error::Custom("float value is infinite or NaN".into()))
        }
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<()> {
        // Encode to UTF-8 then serialize as a JSON string.
        let mut buf = [0u8; 4];
        let s = v.encode_utf8(&mut buf);
        write_str_escape_writer(&mut self.writer, s.as_bytes()).map_err(Error::Io)
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<()> {
        write_str_escape_writer(&mut self.writer, v.as_bytes()).map_err(Error::Io)
    }

    /// Serializes byte slices as a JSON array of integers, matching serde_json.
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        use serde::ser::SerializeSeq as _;
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for b in v {
            seq.serialize_element(b)?;
        }
        seq.end()
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
        self.write_bytes(b"null")
    }
    #[inline]
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }
    #[inline]
    fn serialize_unit(self) -> Result<()> {
        self.write_bytes(b"null")
    }
    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.write_bytes(b"null")
    }

    /// Externally-tagged unit variant: `"VariantName"`.
    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        write_str_escape_writer(&mut self.writer, variant.as_bytes()).map_err(Error::Io)
    }

    /// Newtype struct — transparent, just serialize the inner value.
    #[inline]
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }

    /// Externally-tagged newtype variant: `{"VariantName":value}`.
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_key(&mut self.writer, true).map_err(Error::Io)?;
        write_str_escape_writer(&mut self.writer, variant.as_bytes()).map_err(Error::Io)?;
        self.formatter.end_object_key(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_value(&mut self.writer).map_err(Error::Io)?;
        value.serialize(&mut *self)?;
        self.formatter.end_object_value(&mut self.writer).map_err(Error::Io)?;
        self.formatter.end_object(&mut self.writer).map_err(Error::Io)
    }

    // ------------------------------------------------------------------
    // Compound types
    // ------------------------------------------------------------------

    fn serialize_seq(self, _len: Option<usize>) -> Result<SerializeSeq<'a, W, F>> {
        self.formatter.begin_array(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeSeq { ser: self, first: true })
    }

    fn serialize_tuple(self, _len: usize) -> Result<SerializeSeq<'a, W, F>> {
        self.formatter.begin_array(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeSeq { ser: self, first: true })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<SerializeSeq<'a, W, F>> {
        self.formatter.begin_array(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeSeq { ser: self, first: true })
    }

    /// Externally-tagged tuple variant: `{"VariantName":[...]}`.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeTupleVariant<'a, W, F>> {
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_key(&mut self.writer, true).map_err(Error::Io)?;
        write_str_escape_writer(&mut self.writer, variant.as_bytes()).map_err(Error::Io)?;
        self.formatter.end_object_key(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_value(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_array(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeTupleVariant { ser: self, first: true })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<SerializeMap<'a, W, F>> {
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeMap { ser: self, first: true })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<SerializeMap<'a, W, F>> {
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeMap { ser: self, first: true })
    }

    /// Externally-tagged struct variant: `{"VariantName":{...}}`.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeStructVariant<'a, W, F>> {
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_key(&mut self.writer, true).map_err(Error::Io)?;
        write_str_escape_writer(&mut self.writer, variant.as_bytes()).map_err(Error::Io)?;
        self.formatter.end_object_key(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object_value(&mut self.writer).map_err(Error::Io)?;
        self.formatter.begin_object(&mut self.writer).map_err(Error::Io)?;
        Ok(SerializeStructVariant { ser: self, first: true })
    }
}

// ---------------------------------------------------------------------------
// Sub-serializers
// ---------------------------------------------------------------------------

/// State for serializing a JSON array (`[...]`).
pub struct SerializeSeq<'a, W: io::Write, F: Formatter> {
    ser: &'a mut KowitoSerializer<W, F>,
    first: bool,
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeSeq for SerializeSeq<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.ser.formatter.begin_array_value(&mut self.ser.writer, self.first).map_err(Error::Io)?;
        self.first = false;
        value.serialize(&mut *self.ser)?;
        self.ser.formatter.end_array_value(&mut self.ser.writer).map_err(Error::Io)
    }

    fn end(self) -> Result<()> {
        self.ser.formatter.end_array(&mut self.ser.writer).map_err(Error::Io)
    }
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeTuple for SerializeSeq<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeTupleStruct for SerializeSeq<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

/// State for serializing an externally-tagged tuple variant `{"V":[...]}`.
pub struct SerializeTupleVariant<'a, W: io::Write, F: Formatter> {
    ser: &'a mut KowitoSerializer<W, F>,
    first: bool,
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeTupleVariant for SerializeTupleVariant<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.ser.formatter.begin_array_value(&mut self.ser.writer, self.first).map_err(Error::Io)?;
        self.first = false;
        value.serialize(&mut *self.ser)?;
        self.ser.formatter.end_array_value(&mut self.ser.writer).map_err(Error::Io)
    }

    fn end(self) -> Result<()> {
        self.ser.formatter.end_array(&mut self.ser.writer).map_err(Error::Io)?;
        self.ser.formatter.end_object_value(&mut self.ser.writer).map_err(Error::Io)?;
        self.ser.formatter.end_object(&mut self.ser.writer).map_err(Error::Io)
    }
}

/// State for serializing a JSON object (`{...}`), used for maps and structs.
pub struct SerializeMap<'a, W: io::Write, F: Formatter> {
    ser: &'a mut KowitoSerializer<W, F>,
    first: bool,
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeMap for SerializeMap<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.ser.formatter.begin_object_key(&mut self.ser.writer, self.first).map_err(Error::Io)?;
        self.first = false;
        key.serialize(&mut *self.ser)?;
        self.ser.formatter.end_object_key(&mut self.ser.writer).map_err(Error::Io)
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.ser.formatter.begin_object_value(&mut self.ser.writer).map_err(Error::Io)?;
        value.serialize(&mut *self.ser)?;
        self.ser.formatter.end_object_value(&mut self.ser.writer).map_err(Error::Io)
    }

    fn end(self) -> Result<()> {
        self.ser.formatter.end_object(&mut self.ser.writer).map_err(Error::Io)
    }
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeStruct for SerializeMap<'a, W, F> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        use serde::ser::SerializeMap as _;
        self.serialize_key(key)?;
        self.serialize_value(value)
    }

    fn end(self) -> Result<()> {
        serde::ser::SerializeMap::end(self)
    }
}

/// State for serializing an externally-tagged struct variant `{"V":{...}}`.
pub struct SerializeStructVariant<'a, W: io::Write, F: Formatter> {
    ser: &'a mut KowitoSerializer<W, F>,
    first: bool,
}

impl<'a, W: io::Write, F: Formatter> ser::SerializeStructVariant
    for SerializeStructVariant<'a, W, F>
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.ser.formatter.begin_object_key(&mut self.ser.writer, self.first).map_err(Error::Io)?;
        self.first = false;
        write_str_escape_writer(&mut self.ser.writer, key.as_bytes()).map_err(Error::Io)?;
        self.ser.formatter.end_object_key(&mut self.ser.writer).map_err(Error::Io)?;
        self.ser.formatter.begin_object_value(&mut self.ser.writer).map_err(Error::Io)?;
        value.serialize(&mut *self.ser)?;
        self.ser.formatter.end_object_value(&mut self.ser.writer).map_err(Error::Io)
    }

    fn end(self) -> Result<()> {
        self.ser.formatter.end_object(&mut self.ser.writer).map_err(Error::Io)?;
        self.ser.formatter.end_object_value(&mut self.ser.writer).map_err(Error::Io)?;
        self.ser.formatter.end_object(&mut self.ser.writer).map_err(Error::Io)
    }
}

// ---------------------------------------------------------------------------
// Top-level convenience functions
// ---------------------------------------------------------------------------

/// Serialize `value` to a compact JSON byte vector.
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(128);
    let mut ser = KowitoSerializer::new(&mut out, CompactFormatter);
    value.serialize(&mut ser)?;
    Ok(out)
}

/// Serialize `value` to a compact JSON string.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    let bytes = to_vec(value)?;
    // Safety: our serializer only emits valid UTF-8 JSON.
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}

/// Serialize `value` to a pretty-printed JSON byte vector (2-space indent).
pub fn to_vec_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(256);
    let mut ser = KowitoSerializer::new(&mut out, PrettyFormatter::default());
    value.serialize(&mut ser)?;
    Ok(out)
}

/// Serialize `value` to a pretty-printed JSON string (2-space indent).
pub fn to_string_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    let bytes = to_vec_pretty(value)?;
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}

/// Serialize `value` as compact JSON into the given `io::Write` target.
pub fn to_writer<W: io::Write, T: Serialize + ?Sized>(writer: W, value: &T) -> Result<()> {
    let mut ser = KowitoSerializer::new(writer, CompactFormatter);
    value.serialize(&mut ser)
}

/// Serialize `value` as pretty-printed JSON into the given `io::Write` target.
pub fn to_writer_pretty<W: io::Write, T: Serialize + ?Sized>(writer: W, value: &T) -> Result<()> {
    let mut ser = KowitoSerializer::new(writer, PrettyFormatter::default());
    value.serialize(&mut ser)
}
