#![allow(unsafe_op_in_unsafe_fn)]
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// AVX2 + PCLMULQDQ scanner — 64-byte main loop, simdjson-style string detection.
///
/// Per 64-byte iteration:
///   • 2  × _mm256_loadu_si256       — load 64 bytes into two 256-bit registers
///   • 14 × _mm256_cmpeq_epi8        — 2 quote + 12 structural comparisons
///   • 6  × _mm256_or_si256          — structural OR reduction (3 per half)
///   • 4  × _mm256_movemask_epi8     — extract 4 × 32-bit masks → two u64
///   • 1  × _mm_clmulepi64_si128     — PCLMULQDQ prefix-parity string detection
///   • trailing_zeros drain loops    — write tape entries
///
/// String detection follows the simdjson `find_quote_mask_and_bits` pattern:
/// CLMUL(quote_mask, 0xFFFF…) computes the XOR prefix-sum so that bit i is set
/// iff the number of quotes in positions 0..=i is odd (= "inside a string").
/// XOR with `prev_in_string` (0 or !0) propagates the carry across 64-byte blocks.
///
/// # Safety
/// The caller must ensure AVX2 and PCLMULQDQ are available on the current CPU.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,pclmulqdq")]
pub unsafe fn scan_avx2(bytes: &[u8], tape: &mut [u32]) -> usize {
    use crate::scanner::tag_byte;
    let mut tape_idx = 0usize;
    let mut i = 0usize;
    // All-zeros  → not in a string at start; all-ones → in a string at start.
    let mut prev_in_string: u64 = 0;

    // Splatted comparison constants — hoisted out of the loop.
    let q_splat = _mm256_set1_epi8(b'"' as i8);
    let lb_splat = _mm256_set1_epi8(b'{' as i8);
    let rb_splat = _mm256_set1_epi8(b'}' as i8);
    let lsb_splat = _mm256_set1_epi8(b'[' as i8);
    let rsb_splat = _mm256_set1_epi8(b']' as i8);
    let col_splat = _mm256_set1_epi8(b':' as i8);
    let com_splat = _mm256_set1_epi8(b',' as i8);

    // PCLMULQDQ multiplicand: low 64 bits all-ones.
    // CLMUL(quote_mask, this) = XOR prefix-sum of quote_mask bits.
    let clmul_ones = _mm_cvtsi64_si128(!0i64);

    // Macro: OR-reduce six structural character comparisons for one __m256i.
    // Defined once; usable in both the 64-byte and 32-byte loops below.
    macro_rules! struct_or {
        ($v:expr) => {
            _mm256_or_si256(
                _mm256_or_si256(
                    _mm256_or_si256(
                        _mm256_cmpeq_epi8($v, lb_splat),
                        _mm256_cmpeq_epi8($v, rb_splat),
                    ),
                    _mm256_or_si256(
                        _mm256_cmpeq_epi8($v, lsb_splat),
                        _mm256_cmpeq_epi8($v, rsb_splat),
                    ),
                ),
                _mm256_or_si256(
                    _mm256_cmpeq_epi8($v, col_splat),
                    _mm256_cmpeq_epi8($v, com_splat),
                ),
            )
        };
    }

    // ------------------------------------------------------------------
    // 64-byte main loop
    // ------------------------------------------------------------------
    while i + 64 <= bytes.len() {
        let ptr = bytes.as_ptr().add(i);

        // Load two 32-byte registers spanning the full 64-byte block.
        let v0 = _mm256_loadu_si256(ptr as *const __m256i);
        let v1 = _mm256_loadu_si256(ptr.add(32) as *const __m256i);

        // Quote mask: combine two 32-bit movemask results into one u64.
        let qm0 = _mm256_movemask_epi8(_mm256_cmpeq_epi8(v0, q_splat)) as u32;
        let qm1 = _mm256_movemask_epi8(_mm256_cmpeq_epi8(v1, q_splat)) as u32;
        let quote_mask: u64 = (qm1 as u64) << 32 | (qm0 as u64);

        // Structural mask: same layout.
        let sm0 = _mm256_movemask_epi8(struct_or!(v0)) as u32;
        let sm1 = _mm256_movemask_epi8(struct_or!(v1)) as u32;
        let struct_mask: u64 = (sm1 as u64) << 32 | (sm0 as u64);

        // PCLMULQDQ string detection.
        // Place quote_mask in the low 64 bits of a __m128i, then carry-less
        // multiply with all-ones.  Low 64 bits of the 128-bit product = the
        // XOR prefix-sum: bit i = parity of quotes at positions 0..=i.
        let q_vec = _mm_cvtsi64_si128(quote_mask as i64);
        let prod = _mm_clmulepi64_si128(q_vec, clmul_ones, 0x00);
        let cumulative = _mm_cvtsi128_si64(prod) as u64;

        // XOR with prev_in_string to account for cross-block carry, then
        // sign-extend the MSB to prepare the carry for the next iteration.
        let string64 = cumulative ^ prev_in_string;
        prev_in_string = ((string64 as i64) >> 63) as u64;

        // Split into two 32-bit halves — mirrors the NEON drain pattern.
        let string_lo = string64 as u32;
        let string_hi = (string64 >> 32) as u32;
        let q_lo = quote_mask as u32;
        let q_hi = (quote_mask >> 32) as u32;
        let s_lo = struct_mask as u32;
        let s_hi = (struct_mask >> 32) as u32;

        // active = (structurals outside strings) | all quote positions.
        let mut active_lo = (s_lo & !string_lo) | q_lo;
        let mut active_hi = (s_hi & !string_hi) | q_hi;

        while active_lo != 0 {
            let tz = active_lo.trailing_zeros();
            active_lo &= active_lo - 1;
            let pos = i + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }
        while active_hi != 0 {
            let tz = active_hi.trailing_zeros();
            active_hi &= active_hi - 1;
            let pos = i + 32 + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }

        i += 64;
    }

    // ------------------------------------------------------------------
    // 32-byte tail loop (single AVX2 register)
    // ------------------------------------------------------------------
    while i + 32 <= bytes.len() {
        let ptr = bytes.as_ptr().add(i);
        let v0 = _mm256_loadu_si256(ptr as *const __m256i);

        let quote_mask32 = _mm256_movemask_epi8(_mm256_cmpeq_epi8(v0, q_splat)) as u32;
        let struct_mask32 = _mm256_movemask_epi8(struct_or!(v0)) as u32;

        let q_vec = _mm_cvtsi64_si128(quote_mask32 as i64);
        let prod = _mm_clmulepi64_si128(q_vec, clmul_ones, 0x00);
        // Only the low 32 bits matter for a 32-byte window.
        let cumulative32 = _mm_cvtsi128_si64(prod) as u32;
        let string32 = cumulative32 ^ prev_in_string as u32;
        prev_in_string = ((string32 as i32) >> 31) as u64;

        let mut active = (struct_mask32 & !string32) | quote_mask32;
        while active != 0 {
            let tz = active.trailing_zeros();
            active &= active - 1;
            let pos = i + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }

        i += 32;
    }

    // ------------------------------------------------------------------
    // Scalar tail for the final < 32 bytes.
    //
    // We inline the scan rather than delegating to generic::Scanner so that
    // the in-string carry from the SIMD loop above is preserved correctly.
    // ------------------------------------------------------------------
    {
        let mut in_string = prev_in_string != 0;
        let mut escape = false;
        while i < bytes.len() {
            let b = *bytes.get_unchecked(i);
            if escape {
                escape = false;
            } else if b == b'\\' && in_string {
                escape = true;
            } else if b == b'"' {
                if tape_idx < tape.len() {
                    *tape.get_unchecked_mut(tape_idx) = tag_byte(b, i);
                    tape_idx += 1;
                }
                in_string = !in_string;
            } else if !in_string {
                match b {
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                        if tape_idx < tape.len() {
                            *tape.get_unchecked_mut(tape_idx) = tag_byte(b, i);
                            tape_idx += 1;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
    }

    tape_idx
}

#[cfg(not(target_arch = "x86_64"))]
/// # Safety
/// This function is a non-x86_64 stub and always panics.
pub unsafe fn scan_avx2(_bytes: &[u8], _tape: &mut [u32]) -> usize {
    unreachable!("scan_avx2 called on non-x86_64 architecture")
}
