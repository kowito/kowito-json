// ---------------------------------------------------------------------------
// Stable SIMD helpers (no nightly features required)
// ---------------------------------------------------------------------------
#![allow(unsafe_op_in_unsafe_fn)]

/// Compute 8 bitmasks for the 32 bytes at `bytes[offset..]`:
/// [m_quote, m_esc, m_lcb, m_rcb, m_lsb, m_rsb, m_col, m_com]
#[inline(always)]
#[allow(unreachable_code)]
fn compute_masks(bytes: &[u8], offset: usize) -> [u32; 8] {
    // x86_64: runtime dispatch for AVX2
    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        return unsafe { compute_masks_avx2(bytes, offset) };
    }

    // aarch64: NEON is always available on AArch64
    #[cfg(target_arch = "aarch64")]
    return unsafe { compute_masks_neon(bytes, offset) };

    // Scalar fallback for other architectures
    compute_masks_scalar(bytes, offset)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn compute_masks_avx2(bytes: &[u8], offset: usize) -> [u32; 8] {
    use core::arch::x86_64::*;
    let chunk = _mm256_loadu_si256(bytes.as_ptr().add(offset) as *const __m256i);
    let eq = |b: u8| -> u32 {
        _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, _mm256_set1_epi8(b as i8))) as u32
    };
    [
        eq(b'"'),
        eq(b'\\'),
        eq(b'{'),
        eq(b'}'),
        eq(b'['),
        eq(b']'),
        eq(b':'),
        eq(b','),
    ]
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn compute_masks_neon(bytes: &[u8], offset: usize) -> [u32; 8] {
    use core::arch::aarch64::*;
    let lo = vld1q_u8(bytes.as_ptr().add(offset));
    let hi = vld1q_u8(bytes.as_ptr().add(offset + 16));
    let eq32 = |b: u8| -> u32 {
        let t = vdupq_n_u8(b);
        let lo_mask = neon_movemask_u8x16(vceqq_u8(lo, t)) as u32;
        let hi_mask = (neon_movemask_u8x16(vceqq_u8(hi, t)) as u32) << 16;
        lo_mask | hi_mask
    };
    [
        eq32(b'"'),
        eq32(b'\\'),
        eq32(b'{'),
        eq32(b'}'),
        eq32(b'['),
        eq32(b']'),
        eq32(b':'),
        eq32(b','),
    ]
}

/// NEON horizontal movemask: extracts the MSB of each byte into a `u16`.
/// byte[0].bit7 → result.bit0, byte[1].bit7 → result.bit1, …, byte[15].bit7 → result.bit15.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn neon_movemask_u8x16(v: core::arch::aarch64::uint8x16_t) -> u16 {
    use core::arch::aarch64::*;
    // Each byte after vshrq_n_u8(_, 7) is 0x01 (MSB was set) or 0x00.
    // Multiply by positional weight (1, 2, 4, 8 … 128) then sum both halves.
    const POSITIONAL: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
    let pos = vld1q_u8(POSITIONAL.as_ptr());
    let bits = vshrq_n_u8(v, 7); // 0x01 or 0x00 per lane
    let weighted = vmulq_u8(bits, pos); // lane i carries its bit-weight or 0
    let lo = vaddv_u8(vget_low_u8(weighted)) as u16; // bits 0-7
    let hi = (vaddv_u8(vget_high_u8(weighted)) as u16) << 8; // bits 8-15
    lo | hi
}

fn compute_masks_scalar(bytes: &[u8], offset: usize) -> [u32; 8] {
    let mut masks = [0u32; 8];
    for j in 0..32usize {
        match bytes[offset + j] {
            b'"' => masks[0] |= 1 << j,
            b'\\' => masks[1] |= 1 << j,
            b'{' => masks[2] |= 1 << j,
            b'}' => masks[3] |= 1 << j,
            b'[' => masks[4] |= 1 << j,
            b']' => masks[5] |= 1 << j,
            b':' => masks[6] |= 1 << j,
            b',' => masks[7] |= 1 << j,
            _ => {}
        }
    }
    masks
}

// ---------------------------------------------------------------------------

/// Implement a SIMD block processor (e.g., using pulp) to find structural characters ({, }, :, ,, ").
/// Include whitespace skippers and bitmask index generation for the tape.
pub struct Scanner<'a> {
    input: &'a [u8],
}

impl<'a> Scanner<'a> {
    #[inline(always)]
    pub fn new(input: &'a [u8]) -> Self {
        Self { input }
    }

    /// Scan the entire input and build the tape inside the provided slice.
    /// Returns the number of tape elements written.
    pub fn scan(&self, tape: &mut [u32]) -> usize {
        let mut tape_idx = 0;
        let mut in_string = false;
        let mut escape = false;

        let mut i = 0;
        let bytes = self.input;

        while i + 32 <= bytes.len() {
            // Cache prefetch — stable on x86_64; no-op on other arches.
            #[cfg(target_arch = "x86_64")]
            if i + 128 < bytes.len() {
                unsafe {
                    use core::arch::x86_64::{_MM_HINT_T2, _mm_prefetch};
                    _mm_prefetch(bytes.as_ptr().add(i + 128) as *const i8, _MM_HINT_T2);
                }
            }

            let [m_quote, m_esc, m_lcb, m_rcb, m_lsb, m_rsb, m_col, m_com] =
                compute_masks(bytes, i);

            if in_string {
                if m_quote == 0 && m_esc == 0 && !escape {
                    i += 32;
                    continue;
                }

                // Fallback scalar for string bodies if there are quotes or escapes.
                let end = i + 32;
                while i < end {
                    let b = bytes[i];
                    if escape {
                        escape = false;
                    } else if b == b'\\' {
                        escape = true;
                    } else if b == b'"' {
                        in_string = false;
                        if tape_idx < tape.len() {
                            tape[tape_idx] = i as u32; // End of string quote
                            tape_idx += 1;
                        }
                        i += 1;
                        break; // exit scalar loop back to SIMD
                    }
                    i += 1;
                }
            } else {
                let _ = m_esc; // not used in the structural path

                let mut structurals = m_quote | m_lcb | m_rcb | m_lsb | m_rsb | m_col | m_com;

                if structurals == 0 {
                    i += 32;
                    continue;
                }

                // Process bitmask efficiently
                while structurals != 0 {
                    let tz = structurals.trailing_zeros();
                    structurals &= structurals - 1; // clear lowest set bit
                    let pos = i + tz as usize;

                    if bytes[pos] == b'"' {
                        in_string = true;
                        if tape_idx < tape.len() {
                            tape[tape_idx] = pos as u32;
                            tape_idx += 1;
                        }
                        i = pos + 1;
                        break; // back to main loop to handle string parsing
                    } else {
                        if tape_idx < tape.len() {
                            tape[tape_idx] = pos as u32;
                            tape_idx += 1;
                        }
                    }
                }
                if !in_string {
                    i += 32;
                }
            }
        }

        // Tail processing for leftover bytes
        while i < bytes.len() {
            let b = bytes[i];
            if in_string {
                if escape {
                    escape = false;
                } else if b == b'\\' {
                    escape = true;
                } else if b == b'"' {
                    in_string = false;
                    if tape_idx < tape.len() {
                        tape[tape_idx] = i as u32;
                        tape_idx += 1;
                    }
                }
            } else {
                match b {
                    b'"' => {
                        in_string = true;
                        if tape_idx < tape.len() {
                            tape[tape_idx] = i as u32;
                            tape_idx += 1;
                        }
                    }
                    b'{' | b'}' | b'[' | b']' | b':' | b',' if tape_idx < tape.len() => {
                        tape[tape_idx] = i as u32;
                        tape_idx += 1;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        tape_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_empty() {
        let scanner = Scanner::new(b"");
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_scan_simple_object() {
        let json = b"{\"key\":\"value\"}";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);

        // Expected structural characters:
        // '{' at 0
        // '"' at 1 (start string)
        // '"' at 5 (end string)
        // ':' at 6
        // '"' at 7 (start string)
        // '"' at 13 (end string)
        // '}' at 14
        assert_eq!(count, 7);
        assert_eq!(&tape[..count], &[0, 1, 5, 6, 7, 13, 14]);
    }

    #[test]
    fn test_scan_with_escaped_quotes() {
        let json = br#"{"key":"val\"ue"}"#;
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);

        // Expected structural characters:
        // '{' at 0
        // '"' at 1 (start string)
        // '"' at 5 (end string)
        // ':' at 6
        // '"' at 7 (start string)
        // The \" at 11,12 should NOT trigger a tape entry
        // '"' at 15 (end string)
        // '}' at 16
        assert_eq!(count, 7);
        assert_eq!(&tape[..count], &[0, 1, 5, 6, 7, 15, 16]);
    }

    #[test]
    fn test_scan_array_and_primitives() {
        let json = b"[1, true, null]";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);

        // '[' at 0, ',' at 2, ',' at 8, ']' at 14
        assert_eq!(count, 4);
        assert_eq!(&tape[..count], &[0, 2, 8, 14]);
    }

    #[test]
    fn test_tape_overflow_safety() {
        let json = b"[1,2,3,4,5]";
        let scanner = Scanner::new(json);
        // tape too small
        let mut tape = vec![0; 2];
        let count = scanner.scan(&mut tape);

        // Scanner should not panic, and count should not exceed tape.len()
        assert_eq!(count, 2);
    }

    #[test]
    fn test_scan_numbers() {
        let json = b"[123, -4.56, 7e8, -9.01E-2]";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);

        // Expected structural characters:
        // '[' at 0
        // ',' at 4
        // ',' at 11
        // ',' at 16
        // ']' at 26
        assert_eq!(count, 5);
        assert_eq!(&tape[..count], &[0, 4, 11, 16, 26]);
    }

    #[test]
    fn test_scan_incomplete_string() {
        let json = b"{\"key\":\"val"; // Missing closing quote
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);

        // Should capture:
        // '{' at 0
        // '"' at 1
        // '"' at 5
        // ':' at 6
        // '"' at 7
        assert_eq!(count, 5);
        assert_eq!(&tape[..count], &[0, 1, 5, 6, 7]);
    }

    #[test]
    fn test_scan_only_whitespace() {
        let json = b" \n \t \r ";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);
        assert_eq!(count, 0); // No structural characters
    }
}
