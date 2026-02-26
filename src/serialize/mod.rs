use std::vec::Vec;
use std::string::String;

// ---------------------------------------------------------------------------
// Compile-time escape lookup table
// ---------------------------------------------------------------------------
// Each entry is `0` if the byte is JSON-safe (no escaping needed), or the
// ASCII code of the *escape character* that follows the backslash (e.g. `n`
// for newline, `u` as a sentinel for the generic \u00XX path).
const fn build_escape_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    // All C0 control characters need escaping
    let mut i = 0u8;
    // We use a while loop because for-loops aren't stable in const fn yet
    loop {
        if i >= 0x20 { break; }
        t[i as usize] = b'u'; // generic \u00XX fallback
        i += 1;
    }
    // Overwrite the named ones with their short escape letter
    t[b'"'  as usize] = b'"';
    t[b'\\' as usize] = b'\\';
    t[b'\n' as usize] = b'n';
    t[b'\r' as usize] = b'r';
    t[b'\t' as usize] = b't';
    t[0x08]           = b'b'; // backspace
    t[0x0C]           = b'f'; // form-feed
    t
}

/// Pre-computed escape table (256 bytes, lives in read-only data section).
pub static ESCAPE_TABLE: [u8; 256] = build_escape_table();

// ---------------------------------------------------------------------------
// Core fast-path string writer
// ---------------------------------------------------------------------------
/// Write `bytes` as a JSON string into `buf` (without allocating).
///
/// Strategy: scan forward to find the first byte that needs escaping using
/// the lookup table (single array index + branch — very branch-predictor
/// friendly), bulk-copy the safe prefix with `extend_from_slice`, emit the
/// escape sequence, then continue.
#[inline]
pub fn write_str_escape(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.push(b'"');
    let mut start = 0usize;
    let mut i = 0usize;
    let len = bytes.len();

    // Reserve enough room for the whole string unescaped + opening/closing
    // quotes so the tight loop below generally avoids reallocations.
    buf.reserve(len + 2);

    while i < len {
        // SAFETY: i < len ensures in-bounds access.
        let b = unsafe { *bytes.get_unchecked(i) };
        let esc = unsafe { *ESCAPE_TABLE.get_unchecked(b as usize) };
        if esc != 0 {
            // Bulk-copy the safe prefix before this byte
            buf.extend_from_slice(unsafe { bytes.get_unchecked(start..i) });
            // Emit the two-byte (or six-byte) escape sequence
            match esc {
                b'u' => {
                    // Generic \u00XX path for remaining control chars
                    buf.extend_from_slice(b"\\u00");
                    let hi = (b >> 4) & 0xF;
                    let lo = b & 0xF;
                    buf.push(if hi < 10 { b'0' + hi } else { b'a' + hi - 10 });
                    buf.push(if lo < 10 { b'0' + lo } else { b'a' + lo - 10 });
                }
                c => {
                    buf.push(b'\\');
                    buf.push(c);
                }
            }
            start = i + 1;
        }
        i += 1;
    }

    // Bulk-copy the final safe chunk (or the whole string if no escapes found)
    if start < len {
        buf.extend_from_slice(unsafe { bytes.get_unchecked(start..len) });
    }
    buf.push(b'"');
}

// ---------------------------------------------------------------------------
// FastWrite trait
// ---------------------------------------------------------------------------
pub trait FastWrite {
    fn write_fast(&self, buf: &mut Vec<u8>);
}

impl FastWrite for String {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        write_str_escape(buf, self.as_bytes());
    }
}

impl FastWrite for str {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        write_str_escape(buf, self.as_bytes());
    }
}

impl FastWrite for bool {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        if *self {
            buf.extend_from_slice(b"true");
        } else {
            buf.extend_from_slice(b"false");
        }
    }
}

macro_rules! impl_fast_write_int {
    ($($t:ty),*) => {
        $(
            impl FastWrite for $t {
                #[inline(always)]
                fn write_fast(&self, buf: &mut Vec<u8>) {
                    let mut buffer = itoa::Buffer::new();
                    buf.extend_from_slice(buffer.format(*self).as_bytes());
                }
            }
        )*
    };
}

impl_fast_write_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

macro_rules! impl_fast_write_float {
    ($($t:ty),*) => {
        $(
            impl FastWrite for $t {
                #[inline(always)]
                fn write_fast(&self, buf: &mut Vec<u8>) {
                    let mut buffer = ryu::Buffer::new();
                    buf.extend_from_slice(buffer.format(*self).as_bytes());
                }
            }
        )*
    };
}

impl_fast_write_float!(f32, f64);

/// Global dispatcher used by the derive macro
#[inline(always)]
pub fn write_value<T: FastWrite + ?Sized>(val: &T, buf: &mut Vec<u8>) {
    val.write_fast(buf);
}

// ---------------------------------------------------------------------------
// Advanced FastWrite implementations (Collections, Option, etc.)
// ---------------------------------------------------------------------------

impl<T: FastWrite> FastWrite for Vec<T> {
    #[inline]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        self.as_slice().write_fast(buf);
    }
}

impl<T: FastWrite> FastWrite for [T] {
    #[inline]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        buf.push(b'[');
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                buf.push(b',');
            }
            item.write_fast(buf);
        }
        buf.push(b']');
    }
}

impl<T: FastWrite + ?Sized> FastWrite for &T {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        (**self).write_fast(buf);
    }
}

impl<T: FastWrite> FastWrite for Option<T> {
    #[inline]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        match self {
            Some(v) => v.write_fast(buf),
            None => buf.extend_from_slice(b"null"),
        }
    }
}

impl<T: FastWrite + ?Sized> FastWrite for Box<T> {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        self.as_ref().write_fast(buf);
    }
}

use std::borrow::Cow;
impl<'a, T: FastWrite + ?Sized + ToOwned> FastWrite for Cow<'a, T> {
    #[inline(always)]
    fn write_fast(&self, buf: &mut Vec<u8>) {
        self.as_ref().write_fast(buf);
    }
}
