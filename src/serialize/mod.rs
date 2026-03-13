#![allow(unsafe_op_in_unsafe_fn)]

use std::string::String;
use std::vec::Vec;

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

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
        if i >= 0x20 {
            break;
        }
        t[i as usize] = b'u'; // generic \u00XX fallback
        i += 1;
    }
    // Overwrite the named ones with their short escape letter
    t[b'"' as usize] = b'"';
    t[b'\\' as usize] = b'\\';
    t[b'\n' as usize] = b'n';
    t[b'\r' as usize] = b'r';
    t[b'\t' as usize] = b't';
    t[0x08] = b'b'; // backspace
    t[0x0C] = b'f'; // form-feed
    t
}

/// Pre-computed escape table (256 bytes, lives in read-only data section).
pub static ESCAPE_TABLE: [u8; 256] = build_escape_table();

// ---------------------------------------------------------------------------
// Core fast-path string writer — SIMD escape-mask helpers
// ---------------------------------------------------------------------------

// Positional bitmask for NEON movemask via pairwise reduction.
#[cfg(target_arch = "aarch64")]
static BITMASK: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];

/// Single-vector NEON movemask: bit j of the result is set iff lane j is 0xFF.
/// Cost: 1 vld1q (hoisted) + 1 vand + 3 vpaddq + 1 fmov ≈ 5 instructions.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_movemask_u8x16(v: uint8x16_t) -> u16 {
    let bm = vld1q_u8(BITMASK.as_ptr());
    let t = vandq_u8(v, bm);
    let p1 = vpaddq_u8(t, t);
    let p2 = vpaddq_u8(p1, p1);
    let p3 = vpaddq_u8(p2, p2);
    vgetq_lane_u64(vreinterpretq_u64_u8(p3), 0) as u16
}

/// Bulk movemask for four 16-byte comparison result vectors.
/// Returns u64 packed: bits 0-15=c0, 16-31=c1, 32-47=c2, 48-63=c3.
/// Cost: 4 vand + 3 vpaddq + 1 fmov = 8 instructions total.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn bulk_movemask_4x16(
    c0: uint8x16_t,
    c1: uint8x16_t,
    c2: uint8x16_t,
    c3: uint8x16_t,
) -> u64 {
    let bm = vld1q_u8(BITMASK.as_ptr());
    let t0 = vandq_u8(c0, bm);
    let t1 = vandq_u8(c1, bm);
    let t2 = vandq_u8(c2, bm);
    let t3 = vandq_u8(c3, bm);
    let p01 = vpaddq_u8(t0, t1);
    let p23 = vpaddq_u8(t2, t3);
    let p0123 = vpaddq_u8(p01, p23);
    let r = vpaddq_u8(p0123, p0123);
    vgetq_lane_u64(vreinterpretq_u64_u8(r), 0)
}

/// 64-byte escape mask: bit j set ⟺ byte[ptr+j] is `"`, `\`, or control (<0x20).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn escape_mask_neon_x64(ptr: *const u8) -> u64 {
    let v0 = vld1q_u8(ptr);
    let v1 = vld1q_u8(ptr.add(16));
    let v2 = vld1q_u8(ptr.add(32));
    let v3 = vld1q_u8(ptr.add(48));
    let quote = vdupq_n_u8(b'"');
    let bslash = vdupq_n_u8(b'\\');
    let ctrl = vdupq_n_u8(0x20);
    let m0 = vorrq_u8(
        vorrq_u8(vceqq_u8(v0, quote), vceqq_u8(v0, bslash)),
        vcltq_u8(v0, ctrl),
    );
    let m1 = vorrq_u8(
        vorrq_u8(vceqq_u8(v1, quote), vceqq_u8(v1, bslash)),
        vcltq_u8(v1, ctrl),
    );
    let m2 = vorrq_u8(
        vorrq_u8(vceqq_u8(v2, quote), vceqq_u8(v2, bslash)),
        vcltq_u8(v2, ctrl),
    );
    let m3 = vorrq_u8(
        vorrq_u8(vceqq_u8(v3, quote), vceqq_u8(v3, bslash)),
        vcltq_u8(v3, ctrl),
    );
    bulk_movemask_4x16(m0, m1, m2, m3)
}

/// 32-byte escape mask.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn escape_mask_neon_x32(ptr: *const u8) -> u32 {
    let v0 = vld1q_u8(ptr);
    let v1 = vld1q_u8(ptr.add(16));
    let quote = vdupq_n_u8(b'"');
    let bslash = vdupq_n_u8(b'\\');
    let ctrl = vdupq_n_u8(0x20);
    let m0 = vorrq_u8(
        vorrq_u8(vceqq_u8(v0, quote), vceqq_u8(v0, bslash)),
        vcltq_u8(v0, ctrl),
    );
    let m1 = vorrq_u8(
        vorrq_u8(vceqq_u8(v1, quote), vceqq_u8(v1, bslash)),
        vcltq_u8(v1, ctrl),
    );
    (neon_movemask_u8x16(m0) as u32) | ((neon_movemask_u8x16(m1) as u32) << 16)
}

/// 16-byte escape mask.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn escape_mask_neon_x16(ptr: *const u8) -> u16 {
    let v = vld1q_u8(ptr);
    let quote = vceqq_u8(v, vdupq_n_u8(b'"'));
    let bslash = vceqq_u8(v, vdupq_n_u8(b'\\'));
    let ctrl = vcltq_u8(v, vdupq_n_u8(0x20));
    neon_movemask_u8x16(vorrq_u8(vorrq_u8(quote, bslash), ctrl))
}

/// 64-byte escape mask (two AVX2 registers).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn escape_mask_avx2_x2(ptr: *const u8) -> u64 {
    use core::arch::x86_64::*;
    let d0 = _mm256_loadu_si256(ptr as *const __m256i);
    let d1 = _mm256_loadu_si256(ptr.add(32) as *const __m256i);
    let q = _mm256_set1_epi8(b'"' as i8);
    let b = _mm256_set1_epi8(b'\\' as i8);
    let c = _mm256_set1_epi8(0x20);
    let m0 = _mm256_or_si256(
        _mm256_cmpeq_epi8(d0, q),
        _mm256_or_si256(_mm256_cmpeq_epi8(d0, b), _mm256_cmpgt_epi8(c, d0)),
    );
    let m1 = _mm256_or_si256(
        _mm256_cmpeq_epi8(d1, q),
        _mm256_or_si256(_mm256_cmpeq_epi8(d1, b), _mm256_cmpgt_epi8(c, d1)),
    );
    (_mm256_movemask_epi8(m0) as u32 as u64) | ((_mm256_movemask_epi8(m1) as u32 as u64) << 32)
}

/// 32-byte escape mask (single AVX2 register).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn escape_mask_avx2(ptr: *const u8) -> u32 {
    use core::arch::x86_64::*;
    let data = _mm256_loadu_si256(ptr as *const __m256i);
    let q = _mm256_set1_epi8(b'"' as i8);
    let b = _mm256_set1_epi8(b'\\' as i8);
    let c = _mm256_set1_epi8(0x20);
    let mask = _mm256_or_si256(
        _mm256_cmpeq_epi8(data, q),
        _mm256_or_si256(_mm256_cmpeq_epi8(data, b), _mm256_cmpgt_epi8(c, data)),
    );
    _mm256_movemask_epi8(mask) as u32
}

// ---------------------------------------------------------------------------
// Bitmask-draining macros (shared by write_str_escape & _raw)
// ---------------------------------------------------------------------------

/// Drain escape bitmask → bulk-copy clean runs via extend_from_slice,
/// then write escape sequence for each flagged byte.
macro_rules! drain_escape_vec {
    ($buf:expr, $bytes:expr, $start:expr, $base:expr, $mask:expr) => {{
        let mut m = $mask;
        while m != 0 {
            let tz = m.trailing_zeros() as usize;
            m &= m.wrapping_sub(1);
            let pos = $base + tz;
            if $start < pos {
                $buf.extend_from_slice(unsafe { $bytes.get_unchecked($start..pos) });
            }
            let byte = unsafe { *$bytes.get_unchecked(pos) };
            let esc = unsafe { *ESCAPE_TABLE.get_unchecked(byte as usize) };
            match esc {
                b'u' => {
                    $buf.extend_from_slice(b"\\u00");
                    let hi = (byte >> 4) & 0xF;
                    let lo = byte & 0xF;
                    $buf.push(if hi < 10 { b'0' + hi } else { b'a' + hi - 10 });
                    $buf.push(if lo < 10 { b'0' + lo } else { b'a' + lo - 10 });
                }
                c => {
                    $buf.push(b'\\');
                    $buf.push(c);
                }
            }
            $start = pos + 1;

            // Manual unroll: process second bit if present
            if m != 0 {
                let tz = m.trailing_zeros() as usize;
                m &= m.wrapping_sub(1);
                let pos = $base + tz;
                if $start < pos {
                    $buf.extend_from_slice(unsafe { $bytes.get_unchecked($start..pos) });
                }
                let byte = unsafe { *$bytes.get_unchecked(pos) };
                let esc = unsafe { *ESCAPE_TABLE.get_unchecked(byte as usize) };
                match esc {
                    b'u' => {
                        $buf.extend_from_slice(b"\\u00");
                        let hi = (byte >> 4) & 0xF;
                        let lo = byte & 0xF;
                        $buf.push(if hi < 10 { b'0' + hi } else { b'a' + hi - 10 });
                        $buf.push(if lo < 10 { b'0' + lo } else { b'a' + lo - 10 });
                    }
                    c => {
                        $buf.push(b'\\');
                        $buf.push(c);
                    }
                }
                $start = pos + 1;
            }
        }
    }};
}

/// Drain escape bitmask using raw pointer arithmetic.
macro_rules! drain_escape_raw {
    ($curr:expr, $bytes:expr, $start:expr, $base:expr, $mask:expr) => {{
        let mut m = $mask;
        while m != 0 {
            let tz = m.trailing_zeros() as usize;
            m &= m.wrapping_sub(1);
            let pos = $base + tz;
            if $start < pos {
                let chunk = $bytes.get_unchecked($start..pos);
                std::ptr::copy_nonoverlapping(chunk.as_ptr(), $curr, chunk.len());
                $curr = $curr.add(chunk.len());
            }
            let byte = *$bytes.get_unchecked(pos);
            let esc = *ESCAPE_TABLE.get_unchecked(byte as usize);
            match esc {
                b'u' => {
                    std::ptr::copy_nonoverlapping(b"\\u00".as_ptr(), $curr, 4);
                    $curr = $curr.add(4);
                    let hi = (byte >> 4) & 0xF;
                    let lo = byte & 0xF;
                    *$curr = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
                    $curr = $curr.add(1);
                    *$curr = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
                    $curr = $curr.add(1);
                }
                c => {
                    *$curr = b'\\';
                    $curr = $curr.add(1);
                    *$curr = c;
                    $curr = $curr.add(1);
                }
            }
            $start = pos + 1;

            // Manual unroll: process second bit if present
            if m != 0 {
                let tz = m.trailing_zeros() as usize;
                m &= m.wrapping_sub(1);
                let pos = $base + tz;
                if $start < pos {
                    let chunk = $bytes.get_unchecked($start..pos);
                    std::ptr::copy_nonoverlapping(chunk.as_ptr(), $curr, chunk.len());
                    $curr = $curr.add(chunk.len());
                }
                let byte = *$bytes.get_unchecked(pos);
                let esc = *ESCAPE_TABLE.get_unchecked(byte as usize);
                match esc {
                    b'u' => {
                        std::ptr::copy_nonoverlapping(b"\\u00".as_ptr(), $curr, 4);
                        $curr = $curr.add(4);
                        let hi = (byte >> 4) & 0xF;
                        let lo = byte & 0xF;
                        *$curr = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
                        $curr = $curr.add(1);
                        *$curr = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
                        $curr = $curr.add(1);
                    }
                    c => {
                        *$curr = b'\\';
                        $curr = $curr.add(1);
                        *$curr = c;
                        $curr = $curr.add(1);
                    }
                }
                $start = pos + 1;
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// String writers
// ---------------------------------------------------------------------------

/// Write `bytes` as a JSON string into `buf`.
///
/// SIMD (NEON / AVX2) produces a bitmask of **all** escape positions in each
/// 64/32/16-byte chunk. Clean runs between escapes are bulk-copied; only the
/// flagged bytes get the escape logic.
#[inline]
pub fn write_str_escape(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.push(b'"');
    let len = bytes.len();
    let mut i = 0usize;
    let mut start = 0usize;
    buf.reserve(len + 2);

    #[cfg(target_arch = "aarch64")]
    {
        while i + 64 <= len {
            let mask = unsafe { escape_mask_neon_x64(bytes.as_ptr().add(i)) };
            if mask == 0 {
                i += 64;
                continue;
            }
            drain_escape_vec!(buf, bytes, start, i, mask);
            i += 64;
        }
        while i + 32 <= len {
            let mask = unsafe { escape_mask_neon_x32(bytes.as_ptr().add(i)) } as u64;
            if mask == 0 {
                i += 32;
                continue;
            }
            drain_escape_vec!(buf, bytes, start, i, mask);
            i += 32;
        }
        while i + 16 <= len {
            let mask = unsafe { escape_mask_neon_x16(bytes.as_ptr().add(i)) } as u64;
            if mask == 0 {
                i += 16;
                continue;
            }
            drain_escape_vec!(buf, bytes, start, i, mask);
            i += 16;
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            while i + 64 <= len {
                let mask = unsafe { escape_mask_avx2_x2(bytes.as_ptr().add(i)) };
                if mask == 0 {
                    i += 64;
                    continue;
                }
                drain_escape_vec!(buf, bytes, start, i, mask);
                i += 64;
            }
            while i + 32 <= len {
                let mask = unsafe { escape_mask_avx2(bytes.as_ptr().add(i)) } as u64;
                if mask == 0 {
                    i += 32;
                    continue;
                }
                drain_escape_vec!(buf, bytes, start, i, mask);
                i += 32;
            }
        }
    }

    #[cold]
    fn scalar_tail(buf: &mut Vec<u8>, bytes: &[u8], mut i: usize, mut start: usize) {
        let len = bytes.len();
        while i < len {
            let b = unsafe { *bytes.get_unchecked(i) };
            let esc = unsafe { *ESCAPE_TABLE.get_unchecked(b as usize) };
            if esc != 0 {
                if start < i {
                    buf.extend_from_slice(unsafe { bytes.get_unchecked(start..i) });
                }
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
    }

    if i < len {
        scalar_tail(buf, bytes, i, start);
    } else {
        if start < len {
            buf.extend_from_slice(unsafe { bytes.get_unchecked(start..len) });
        }
    }
    buf.push(b'"');
}

/// Write `bytes` as a JSON string into a raw pointer. Returns the new pointer.
///
/// # Safety
/// `curr` must point to a writable buffer with capacity ≥ `bytes.len() * 6 + 2`.
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
            let mask = escape_mask_neon_x64(bytes.as_ptr().add(i));
            if mask == 0 {
                i += 64;
                continue;
            }
            drain_escape_raw!(curr, bytes, start, i, mask);
            i += 64;
        }
        while i + 32 <= len {
            let mask = escape_mask_neon_x32(bytes.as_ptr().add(i)) as u64;
            if mask == 0 {
                i += 32;
                continue;
            }
            drain_escape_raw!(curr, bytes, start, i, mask);
            i += 32;
        }
        while i + 16 <= len {
            let mask = escape_mask_neon_x16(bytes.as_ptr().add(i)) as u64;
            if mask == 0 {
                i += 16;
                continue;
            }
            drain_escape_raw!(curr, bytes, start, i, mask);
            i += 16;
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            while i + 64 <= len {
                let mask = escape_mask_avx2_x2(bytes.as_ptr().add(i));
                if mask == 0 {
                    i += 64;
                    continue;
                }
                drain_escape_raw!(curr, bytes, start, i, mask);
                i += 64;
            }
            while i + 32 <= len {
                let mask = escape_mask_avx2(bytes.as_ptr().add(i)) as u64;
                if mask == 0 {
                    i += 32;
                    continue;
                }
                drain_escape_raw!(curr, bytes, start, i, mask);
                i += 32;
            }
        }
    }

    #[cold]
    unsafe fn scalar_tail_raw(
        mut curr: *mut u8,
        bytes: &[u8],
        mut i: usize,
        mut start: usize,
    ) -> *mut u8 {
        let len = bytes.len();
        while i < len {
            let b = *bytes.get_unchecked(i);
            let esc = *ESCAPE_TABLE.get_unchecked(b as usize);
            if esc != 0 {
                if start < i {
                    let chunk = bytes.get_unchecked(start..i);
                    std::ptr::copy_nonoverlapping(chunk.as_ptr(), curr, chunk.len());
                    curr = curr.add(chunk.len());
                }
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
        curr
    }

    if i < len {
        curr = scalar_tail_raw(curr, bytes, i, start);
    } else {
        if start < len {
            let chunk = bytes.get_unchecked(start..len);
            std::ptr::copy_nonoverlapping(chunk.as_ptr(), curr, chunk.len());
            curr = curr.add(chunk.len());
        }
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
    /// Serialize the value directly to raw memory.
    ///
    /// # Safety
    /// This function is unsafe because it dereferences raw pointers.  
    /// The caller must ensure that `curr` points to a valid, properly aligned, and writable buffer with sufficient capacity.
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8;
}

impl SerializeRaw for String {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe { write_str_escape_raw(curr, self.as_bytes()) }
    }
}

impl SerializeRaw for str {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe { write_str_escape_raw(curr, self.as_bytes()) }
    }
}

impl SerializeRaw for bool {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe {
            if *self {
                std::ptr::copy_nonoverlapping(b"true".as_ptr(), curr, 4);
                curr.add(4)
            } else {
                std::ptr::copy_nonoverlapping(b"false".as_ptr(), curr, 5);
                curr.add(5)
            }
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
                    unsafe {
                        std::ptr::copy_nonoverlapping(s.as_ptr(), curr, len);
                        curr.add(len)
                    }
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
                    unsafe {
                        std::ptr::copy_nonoverlapping(s.as_ptr(), curr, len);
                        curr.add(len)
                    }
                }
            }
        )*
    };
}

impl_serialize_float!(f32, f64);

/// Raw global dispatcher
///
/// # Safety
/// This function is unsafe because it calls unsafe trait methods that dereference raw pointers.
/// The caller must ensure that `curr` points to a valid, properly aligned, and writable buffer with sufficient capacity.
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
        unsafe { self.as_slice().serialize_raw(curr) }
    }
}

impl<T: SerializeRaw> SerializeRaw for [T] {
    #[inline]
    unsafe fn serialize_raw(&self, mut curr: *mut u8) -> *mut u8 {
        unsafe {
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
}

impl<T: SerializeRaw + ?Sized> SerializeRaw for &T {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe { (**self).serialize_raw(curr) }
    }
}

impl<T: SerializeRaw> SerializeRaw for Option<T> {
    #[inline]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe {
            match self {
                Some(v) => v.serialize_raw(curr),
                None => {
                    std::ptr::copy_nonoverlapping(b"null".as_ptr(), curr, 4);
                    curr.add(4)
                }
            }
        }
    }
}

impl<T: SerializeRaw + ?Sized> SerializeRaw for Box<T> {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe { self.as_ref().serialize_raw(curr) }
    }
}

impl<'a, T: SerializeRaw + ?Sized + ToOwned> SerializeRaw for Cow<'a, T> {
    #[inline(always)]
    unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
        unsafe { self.as_ref().serialize_raw(curr) }
    }
}
