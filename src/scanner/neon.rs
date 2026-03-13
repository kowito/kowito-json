#![allow(unsafe_op_in_unsafe_fn)]
#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

// Positional bitmask used by both the bulk-reduction movemask and single-vector fallback.
// Layout: [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128]
// bit j of lane j%8 in each 8-byte half, allowing pairwise-add to accumulate a u8 bitmask.
static BITMASK: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];

// ---------------------------------------------------------------------------
// Bulk 4-vector movemask
// ---------------------------------------------------------------------------

/// Compute 16-bit masks for four 16-byte comparison result vectors in one pass.
///
/// Returns a u64 packed as:
///   bits  0-15 → mask for c0
///   bits 16-31 → mask for c1
///   bits 32-47 → mask for c2
///   bits 48-63 → mask for c3
///
/// Cost: 1 vld1q + 4 vand + 3 vpaddq + 1 fmov = **8 instructions total**,
/// vs the old approach of 4 × neon_movemask_u8x16 ≈ 4 × (1 vld + 1 ushr + 1 mul + 2 addv) = **20 instructions**.
#[inline(always)]
unsafe fn bulk_movemask_4x16(
    c0: uint8x16_t,
    c1: uint8x16_t,
    c2: uint8x16_t,
    c3: uint8x16_t,
) -> u64 {
    // AND isolates each lane's match into its positional bit (1, 2, 4 … 128 repeated twice).
    let bm = vld1q_u8(BITMASK.as_ptr());
    let t0 = vandq_u8(c0, bm);
    let t1 = vandq_u8(c1, bm);
    let t2 = vandq_u8(c2, bm);
    let t3 = vandq_u8(c3, bm);

    // Three rounds of pairwise byte-sum to reduce 64 positional bits down to 8 bytes.
    //
    // After round 1 (vpaddq t0,t1):
    //   byte k (k<8):  t0[2k] | t0[2k+1]   (2 adjacent bits of c0 OR'd via orthogonal weights)
    //   byte k (k≥8):  t1[2(k-8)] | t1[2(k-8)+1]
    //
    // After round 2 (vpaddq p01,p23): each output byte holds 4 bits from one original vector.
    //   byte 0: bits 0-3  of c0,  byte 1: bits 4-7  of c0
    //   byte 2: bits 8-11 of c0,  byte 3: bits 12-15 of c0
    //   byte 4-7: same for c1,  byte 8-11: c2,  byte 12-15: c3
    //
    // After round 3 (vpaddq p0123,p0123) — self-fold:
    //   byte 0: bits 0-7  of c0 (the low byte of c0's 16-bit mask)
    //   byte 1: bits 8-15 of c0 (the high byte)
    //   byte 2-3: c1's mask bytes,  byte 4-5: c2's,  byte 6-7: c3's
    //
    // fmov then extracts those 8 bytes as a u64 in one instruction.
    let p01 = vpaddq_u8(t0, t1);
    let p23 = vpaddq_u8(t2, t3);
    let p0123 = vpaddq_u8(p01, p23);
    let r = vpaddq_u8(p0123, p0123); // self-fold: low 8 bytes hold all 4 × 16-bit masks

    vgetq_lane_u64(vreinterpretq_u64_u8(r), 0)
}

// ---------------------------------------------------------------------------
// Structural OR accumulator (shared macro)
// ---------------------------------------------------------------------------

macro_rules! struct_or {
    ($v:expr) => {
        vorrq_u8(
            vorrq_u8(
                vorrq_u8(
                    vorrq_u8(
                        vorrq_u8(
                            vceqq_u8($v, vdupq_n_u8(b'{')),
                            vceqq_u8($v, vdupq_n_u8(b'}')),
                        ),
                        vceqq_u8($v, vdupq_n_u8(b'[')),
                    ),
                    vceqq_u8($v, vdupq_n_u8(b']')),
                ),
                vceqq_u8($v, vdupq_n_u8(b':')),
            ),
            vceqq_u8($v, vdupq_n_u8(b',')),
        )
    };
}

// ---------------------------------------------------------------------------
// Main scanner
// ---------------------------------------------------------------------------

/// ARM NEON scanner — 64-byte main loop, bulk movemask, full 64-bit PMULL.
///
/// Per 64-byte iteration (after this optimisation):
///   • 4 × vld1q_u8           — load 4 × 16 bytes
///   • 4 × vceqq_u8           — quote comparisons
///   • 4 × (5 cmeq + 4 orr)   — structural OR accumulators
///   • 2 × bulk_movemask_4x16 — 8 vand + 6 vpaddq + 2 fmov = **16 instr total**
///   • 1 × vmull_p64           — 64-bit string detection
///   ~68 NEON instructions per 64 bytes, vs ~104 before
///
/// # Safety
/// Caller must ensure `bytes.len()` covers the slice and `tape` has room.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn scan_neon(bytes: &[u8], tape: &mut [u32]) -> usize {
    use crate::scanner::tag_byte;
    let mut tape_idx = 0;
    let mut i = 0;
    let mut prev_in_string: u64 = 0;

    // Broadcast quote byte once — reused for all 4 comparisons each iteration.
    let q_splat = vdupq_n_u8(b'"');

    // -----------------------------------------------------------------------
    // 64-byte main loop
    // -----------------------------------------------------------------------
    while i + 64 <= bytes.len() {
        // Load four 16-byte vectors.
        let v0 = vld1q_u8(bytes.as_ptr().add(i));
        let v1 = vld1q_u8(bytes.as_ptr().add(i + 16));
        let v2 = vld1q_u8(bytes.as_ptr().add(i + 32));
        let v3 = vld1q_u8(bytes.as_ptr().add(i + 48));

        // --- Quote masks (4 comparisons → bulk reduction → quote64) ---
        let q64 = bulk_movemask_4x16(
            vceqq_u8(v0, q_splat),
            vceqq_u8(v1, q_splat),
            vceqq_u8(v2, q_splat),
            vceqq_u8(v3, q_splat),
        );

        // --- Structural masks (6 comparisons ORed per vector → bulk reduction) ---
        let s64 = bulk_movemask_4x16(
            struct_or!(v0),
            struct_or!(v1),
            struct_or!(v2),
            struct_or!(v3),
        );

        // --- Full 64-bit PMULL string detection ---
        // vmull_p64: poly64_t == u64, poly128_t == u128 — no casting needed.
        let cumulative: u64 = vmull_p64(q64, !0u64) as u64;
        let string64 = cumulative ^ prev_in_string;

        // Propagate: fill all 64 bits with the sign bit (= "still in string?").
        prev_in_string = ((string64 as i64) >> 63) as u64;

        let string1 = string64 as u32;
        let string2 = (string64 >> 32) as u32;
        let q1 = q64 as u32;
        let q2 = (q64 >> 32) as u32;
        let s1 = s64 as u32;
        let s2 = (s64 >> 32) as u32;

        // active = (structurals outside strings) ∪ (all quote positions)
        let mut active1 = (s1 & !string1) | q1;
        let mut active2 = (s2 & !string2) | q2;

        while active1 != 0 {
            let tz = active1.trailing_zeros();
            active1 &= active1 - 1;
            let pos = i + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }
        while active2 != 0 {
            let tz = active2.trailing_zeros();
            active2 &= active2 - 1;
            let pos = i + 32 + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }

        i += 64;
    }

    // -----------------------------------------------------------------------
    // 32-byte tail loop (single bulk_movemask pass on 2 vectors, 32-bit PMULL)
    // -----------------------------------------------------------------------
    while i + 32 <= bytes.len() {
        let v0 = vld1q_u8(bytes.as_ptr().add(i));
        let v1 = vld1q_u8(bytes.as_ptr().add(i + 16));

        // Use same bulk routine but pass zero vectors for c2/c3 — they go to the
        // upper 32 bits which we throw away.
        let zero = vdupq_n_u8(0);
        let q32 =
            bulk_movemask_4x16(vceqq_u8(v0, q_splat), vceqq_u8(v1, q_splat), zero, zero) as u32; // lower 32 bits = mask for v0 and v1

        let s32 = bulk_movemask_4x16(struct_or!(v0), struct_or!(v1), zero, zero) as u32;

        let cumulative: u64 = vmull_p64(q32 as u64, !0u64) as u64;
        let string32 = (cumulative ^ prev_in_string) as u32;
        prev_in_string = ((string32 as i32) >> 31) as u64;

        let mut active = (s32 & !string32) | q32;
        while active != 0 {
            let tz = active.trailing_zeros();
            active &= active - 1;
            let pos = i + tz as usize;
            *tape.get_unchecked_mut(tape_idx) = tag_byte(*bytes.get_unchecked(pos), pos);
            tape_idx += 1;
        }

        i += 32;
    }

    // -----------------------------------------------------------------------
    // Scalar tail for the final < 32 bytes.
    //
    // We inline the scan rather than delegating to generic::Scanner so that
    // the in-string carry from the SIMD loop above is preserved correctly.
    // -----------------------------------------------------------------------
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
                    b'{' | b'}' | b'[' | b']' | b':' | b',' if tape_idx < tape.len() => {
                        *tape.get_unchecked_mut(tape_idx) = tag_byte(b, i);
                        tape_idx += 1;
                    }
                    _ => {}
                }
            }
            i += 1;
        }
    }

    tape_idx
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe fn scan_neon(_bytes: &[u8], _tape: &mut [u32]) -> usize {
    unreachable!("scan_neon called on non-aarch64 architecture")
}
