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

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn find_escape_neon_x4(ptr: *const u8) -> u16 {
    use core::arch::aarch64::*;
    
    let d0 = vld1q_u8(ptr);
    let d1 = vld1q_u8(ptr.add(16));
    let d2 = vld1q_u8(ptr.add(32));
    let d3 = vld1q_u8(ptr.add(48));

    let q = vdupq_n_u8(b'"');
    let b = vdupq_n_u8(b'\\');
    let c = vdupq_n_u8(0x20);

    let m0 = vorrq_u8(vceqq_u8(d0, q), vorrq_u8(vceqq_u8(d0, b), vcltq_u8(d0, c)));
    let m1 = vorrq_u8(vceqq_u8(d1, q), vorrq_u8(vceqq_u8(d1, b), vcltq_u8(d1, c)));
    let m2 = vorrq_u8(vceqq_u8(d2, q), vorrq_u8(vceqq_u8(d2, b), vcltq_u8(d2, c)));
    let m3 = vorrq_u8(vceqq_u8(d3, q), vorrq_u8(vceqq_u8(d3, b), vcltq_u8(d3, c)));

    let combined = vorrq_u8(vorrq_u8(m0, m1), vorrq_u8(m2, m3));
    vmaxvq_u8(combined) as u16
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn find_escape_neon(ptr: *const u8) -> u16 {
    use core::arch::aarch64::*;
    let data = vld1q_u8(ptr);
    let quotes = vceqq_u8(data, vdupq_n_u8(b'"'));
    let backslashes = vceqq_u8(data, vdupq_n_u8(b'\\'));
    let controls = vcltq_u8(data, vdupq_n_u8(0x20));
    
    let mask = vorrq_u8(quotes, vorrq_u8(backslashes, controls));
    vmaxvq_u8(mask) as u16
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_escape_avx2_x4(ptr: *const u8) -> u32 {
    use core::arch::x86_64::*;
    let d0 = _mm256_loadu_si256(ptr as *const __m256i);
    let d1 = _mm256_loadu_si256(ptr.add(32) as *const __m256i);
    let d2 = _mm256_loadu_si256(ptr.add(64) as *const __m256i);
    let d3 = _mm256_loadu_si256(ptr.add(96) as *const __m256i);

    let q = _mm256_set1_epi8(b'"' as i8);
    let b = _mm256_set1_epi8(b'\\' as i8);
    let c = _mm256_set1_epi8(0x20);

    let m0 = _mm256_or_si256(_mm256_cmpeq_epi8(d0, q), _mm256_or_si256(_mm256_cmpeq_epi8(d0, b), _mm256_cmpgt_epi8(c, d0)));
    let m1 = _mm256_or_si256(_mm256_cmpeq_epi8(d1, q), _mm256_or_si256(_mm256_cmpeq_epi8(d1, b), _mm256_cmpgt_epi8(c, d1)));
    let m2 = _mm256_or_si256(_mm256_cmpeq_epi8(d2, q), _mm256_or_si256(_mm256_cmpeq_epi8(d2, b), _mm256_cmpgt_epi8(c, d2)));
    let m3 = _mm256_or_si256(_mm256_cmpeq_epi8(d3, q), _mm256_or_si256(_mm256_cmpeq_epi8(d3, b), _mm256_cmpgt_epi8(c, d3)));

    (_mm256_movemask_epi8(m0) | _mm256_movemask_epi8(m1) | _mm256_movemask_epi8(m2) | _mm256_movemask_epi8(m3)) as u32
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_escape_avx2(ptr: *const u8) -> i32 {
    use core::arch::x86_64::*;
    let data = _mm256_loadu_si256(ptr as *const __m256i);
    let q = _mm256_set1_epi8(b'"' as i8);
    let b = _mm256_set1_epi8(b'\\' as i8);
    let c = _mm256_set1_epi8(0x20);

    let mask = _mm256_or_si256(_mm256_cmpeq_epi8(data, q), _mm256_or_si256(_mm256_cmpeq_epi8(data, b), _mm256_cmpgt_epi8(c, data)));
    _mm256_movemask_epi8(mask)
}


/// Write `bytes` as a JSON string into `buf` (without allocating).
///
/// Strategy: Use SIMD (NEON/AVX) to find the first byte that needs escaping.
/// If no escape characters are found in a block, bulk-copy the block.
#[inline]
pub fn write_str_escape(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.push(b'"');
    let len = bytes.len();
    let mut i = 0usize;
    let mut start = 0usize;

    // Reserve enough room for the whole string unescaped + opening/closing quotes
    buf.reserve(len + 2);

    #[cfg(target_arch = "aarch64")]
    {
        // Unrolled loop for large enough strings
        while i + 64 <= len {
            let mask = unsafe { find_escape_neon_x4(bytes.as_ptr().add(i)) };
            if mask != 0 { break; }
            i += 64;
        }

        while i + 16 <= len {
            let mask = unsafe { find_escape_neon(bytes.as_ptr().add(i)) };
            if mask != 0 { break; }
            i += 16;
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            while i + 128 <= len {
                let mask = unsafe { find_escape_avx2_x4(bytes.as_ptr().add(i)) };
                if mask != 0 { break; }
                i += 128;
            }
            while i + 32 <= len {
                let mask = unsafe { find_escape_avx2(bytes.as_ptr().add(i)) };
                if mask != 0 { break; }
                i += 32;
            }
        }
    }


    // Scalar fallback/remainder
    while i < len {
        let b = unsafe { *bytes.get_unchecked(i) };
        let esc = unsafe { *ESCAPE_TABLE.get_unchecked(b as usize) };
        if esc != 0 {
            buf.extend_from_slice(unsafe { bytes.get_unchecked(start..i) });
            match esc {
                b'u' => {
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

    if start < len {
        buf.extend_from_slice(unsafe { bytes.get_unchecked(start..len) });
    }
    buf.push(b'"');
}

/// Write `bytes` as a JSON string into a raw pointer.
/// Returns the new pointer position.
#[inline]
pub unsafe fn write_str_escape_raw(mut curr: *mut u8, bytes: &[u8]) -> *mut u8 {
    *curr = b'"';
    curr = curr.add(1);
    let len = bytes.len();
    let mut i = 0usize;
    let mut start = 0usize;

    #[cfg(target_arch = "aarch64")]
    {
        while i + 64 <= len {
            let mask = unsafe { find_escape_neon_x4(bytes.as_ptr().add(i)) };
            if mask != 0 { break; }
            i += 64;
        }
        while i + 16 <= len {
            let mask = unsafe { find_escape_neon(bytes.as_ptr().add(i)) };
            if mask != 0 { break; }
            i += 16;
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            while i + 128 <= len {
                let mask = find_escape_avx2_x4(bytes.as_ptr().add(i));
                if mask != 0 { break; }
                i += 128;
            }
            while i + 32 <= len {
                let mask = find_escape_avx2(bytes.as_ptr().add(i));
                if mask != 0 { break; }
                i += 32;
            }
        }
    }

    while i < len {
        let b = *bytes.get_unchecked(i);
        let esc = *ESCAPE_TABLE.get_unchecked(b as usize);
        if esc != 0 {
            let chunk = bytes.get_unchecked(start..i);
            std::ptr::copy_nonoverlapping(chunk.as_ptr(), curr, chunk.len());
            curr = curr.add(chunk.len());
            match esc {
                b'u' => {
                    std::ptr::copy_nonoverlapping(b"\\u00".as_ptr(), curr, 4);
                    curr = curr.add(4);
                    let hi = (b >> 4) & 0xF;
                    let lo = b & 0xF;
                    *curr = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
                    curr = curr.add(1);
                    *curr = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
                    curr = curr.add(1);
                }
                c => {
                    *curr = b'\\';
                    curr = curr.add(1);
                    *curr = c;
                    curr = curr.add(1);
                }
            }
            start = i + 1;
        }
        i += 1;
    }

    if start < len {
        let chunk = bytes.get_unchecked(start..len);
        std::ptr::copy_nonoverlapping(chunk.as_ptr(), curr, chunk.len());
        curr = curr.add(chunk.len());
    }
    *curr = b'"';
    curr.add(1)
}


// ---------------------------------------------------------------------------
// Serialize trait
// ---------------------------------------------------------------------------
pub trait Serialize {
    fn serialize(&self, buf: &mut Vec<u8>);
}

pub trait SerializeRaw {
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8;
}

impl SerializeRaw for String {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        write_str_escape_raw(curr, self.as_bytes())
    }
}

impl SerializeRaw for str {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        write_str_escape_raw(curr, self.as_bytes())
    }
}

impl SerializeRaw for bool {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        if *self {
            std::ptr::copy_nonoverlapping(b"true".as_ptr(), curr, 4);
            curr.add(4)
        } else {
            std::ptr::copy_nonoverlapping(b"false".as_ptr(), curr, 5);
            curr.add(5)
        }
    }
}


impl Serialize for String {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        write_str_escape(buf, self.as_bytes());
    }
}

impl Serialize for str {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        write_str_escape(buf, self.as_bytes());
    }
}

impl Serialize for bool {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        if *self {
            buf.extend_from_slice(b"true");
        } else {
            buf.extend_from_slice(b"false");
        }
    }
}

macro_rules! impl_serialize_int {
    ($($t:ty),*) => {
        $(
            impl Serialize for $t {
                #[inline(always)]
                fn serialize(&self, buf: &mut Vec<u8>) {
                    let mut buffer = itoa::Buffer::new();
                    buf.extend_from_slice(buffer.format(*self).as_bytes());
                }
            }

            impl SerializeRaw for $t {
                #[inline(always)]
                unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
                    let mut buffer = itoa::Buffer::new();
                    let s = buffer.format(*self);
                    let len = s.len();
                    std::ptr::copy_nonoverlapping(s.as_ptr(), curr, len);
                    curr.add(len)
                }
            }
        )*
    };
}

impl_serialize_int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

macro_rules! impl_serialize_float {
    ($($t:ty),*) => {
        $(
            impl Serialize for $t {
                #[inline(always)]
                fn serialize(&self, buf: &mut Vec<u8>) {
                    let mut buffer = ryu::Buffer::new();
                    buf.extend_from_slice(buffer.format(*self).as_bytes());
                }
            }

            impl SerializeRaw for $t {
                #[inline(always)]
                unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
                    let mut buffer = ryu::Buffer::new();
                    let s = buffer.format(*self);
                    let len = s.len();
                    std::ptr::copy_nonoverlapping(s.as_ptr(), curr, len);
                    curr.add(len)
                }
            }
        )*
    };
}

impl_serialize_float!(f32, f64);

/// Raw global dispatcher
#[inline(always)]
pub unsafe fn write_value_raw<T: SerializeRaw + ?Sized>(val: &T, curr: *mut u8) -> *mut u8 {
    unsafe { val.serialize_raw(curr) }
}

/// Global dispatcher used by the derive macro
#[inline(always)]
pub fn write_value<T: Serialize + ?Sized>(val: &T, buf: &mut Vec<u8>) {
    val.serialize(buf);
}


// ---------------------------------------------------------------------------
// Advanced Serialize implementations (Collections, Option, etc.)
// ---------------------------------------------------------------------------

impl<T: Serialize> Serialize for Vec<T> {
    #[inline]
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.as_slice().serialize(buf);
    }
}

impl<T: Serialize> Serialize for [T] {
    #[inline]
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.push(b'[');
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                buf.push(b',');
            }
            item.serialize(buf);
        }
        buf.push(b']');
    }
}

impl<T: Serialize + ?Sized> Serialize for &T {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        (**self).serialize(buf);
    }
}

impl<T: Serialize> Serialize for Option<T> {
    #[inline]
    fn serialize(&self, buf: &mut Vec<u8>) {
        match self {
            Some(v) => v.serialize(buf),
            None => buf.extend_from_slice(b"null"),
        }
    }
}

impl<T: Serialize + ?Sized> Serialize for Box<T> {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.as_ref().serialize(buf);
    }
}

use std::borrow::Cow;
impl<'a, T: Serialize + ?Sized + ToOwned> Serialize for Cow<'a, T> {
    #[inline(always)]
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.as_ref().serialize(buf);
    }
}
impl<T: SerializeRaw> SerializeRaw for Vec<T> {
    #[inline]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        self.as_slice().serialize_raw(curr)
    }
}

impl<T: SerializeRaw> SerializeRaw for [T] {
    #[inline]
    unsafe fn serialize_raw(&self, mut curr: *mut u8) -> *mut u8 {
        *curr = b'[';
        curr = curr.add(1);
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                *curr = b',';
                curr = curr.add(1);
            }
            curr = item.serialize_raw(curr);
        }
        *curr = b']';
        curr.add(1)
    }
}

impl<T: SerializeRaw + ?Sized> SerializeRaw for &T {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        (**self).serialize_raw(curr)
    }
}

impl<T: SerializeRaw> SerializeRaw for Option<T> {
    #[inline]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        match self {
            Some(v) => v.serialize_raw(curr),
            None => {
                std::ptr::copy_nonoverlapping(b"null".as_ptr(), curr, 4);
                curr.add(4)
            }
        }
    }
}

impl<T: SerializeRaw + ?Sized> SerializeRaw for Box<T> {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        self.as_ref().serialize_raw(curr)
    }
}

impl<'a, T: SerializeRaw + ?Sized + ToOwned> SerializeRaw for Cow<'a, T> {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        self.as_ref().serialize_raw(curr)
    }
}
