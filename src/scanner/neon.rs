use core::arch::aarch64::*;
use std::simd::prelude::*;

/// The ARM NEON (Apple Silicon) highly optimized scanner
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn scan_neon(bytes: &[u8], tape: &mut [u32]) -> usize {
    let mut tape_idx = 0;
    let mut i = 0;
    
    // The previous quote state (1 if we ended the last chunk inside a string)
    let mut prev_in_string: u64 = 0;
    
    while i + 32 <= bytes.len() {

        let chunk = u8x32::from_slice(&bytes[i..]);
        
        // Find structural and string-boundary characters
        let m_quote = chunk.simd_eq(u8x32::splat(b'"')).to_bitmask() as u64;
        let m_esc = chunk.simd_eq(u8x32::splat(b'\\')).to_bitmask() as u64;

        // Simplify escape tracking for now (assume no weird \" escapes inside strings for raw max speed, 
        // a full simdjson port handles the odd-backslash sequences via bitwise shifts)
        // We focus on PCLMULQDQ / PMULL speed evaluation:
        
        // Carry-less multiplication to calculate prefix parity
        // This instantly tells us which bytes are inside a string without a scalar loop
        let quote_mask = unsafe { vmull_p64(m_quote, !0) };
        
        // Extract the lower 64 bits natively (on AArch64 this requires casting)
        // poly128_t -> u128 -> lower u64
        let pmull_res: u128 = unsafe { core::mem::transmute(quote_mask) };
        let string_mask = (pmull_res as u64) ^ prev_in_string;
        
        // Update state for next chunk (are we still inside a string at the end of this mask?)
        // If bit 31 is 1, (string_mask as i32) is negative, >> 31 makes it -1 (all 1s).
        prev_in_string = ((string_mask as i32) >> 31) as u64;

        // Find structurals `{ } [ ] : ,`
        let m_lcb = chunk.simd_eq(u8x32::splat(b'{')).to_bitmask() as u64;
        let m_rcb = chunk.simd_eq(u8x32::splat(b'}')).to_bitmask() as u64;
        let m_lsb = chunk.simd_eq(u8x32::splat(b'[')).to_bitmask() as u64;
        let m_rsb = chunk.simd_eq(u8x32::splat(b']')).to_bitmask() as u64;
        let m_col = chunk.simd_eq(u8x32::splat(b':')).to_bitmask() as u64;
        let m_com = chunk.simd_eq(u8x32::splat(b',')).to_bitmask() as u64;

        let structurals = m_lcb | m_rcb | m_lsb | m_rsb | m_col | m_com;
        
        // Only keep structurals that are NOT inside a string, but ALWAYS keep quotes!
        let mut active_structurals = (structurals & !string_mask) | m_quote;

        // Write directly to tape using trailing zeros
        while active_structurals != 0 {
            let tz = active_structurals.trailing_zeros();
            active_structurals &= active_structurals - 1; // clear lowest bit
            
            unsafe { *tape.get_unchecked_mut(tape_idx) = (i as u32) + tz; }
            tape_idx += 1;
        }
        
        i += 32;
    }

    // Scalar fallback for tail bytes (omitted for brevity of core loop optimization)
    // In production we would process the tail here.
    
    // Since this is a prototype of PMULL, we just use the generic scanner for the tail
    if i < bytes.len() {
        let generic_tail = crate::scanner::generic::Scanner::new(&bytes[i..]);
        let mut temp_tape = vec![0; tape.len() - tape_idx];
        let tail_count = generic_tail.scan(&mut temp_tape);
        for j in 0..tail_count {
            if tape_idx < tape.len() {
                tape[tape_idx] = (i as u32) + temp_tape[j];
                tape_idx += 1;
            }
        }
    }

    tape_idx
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe fn scan_neon(_bytes: &[u8], _tape: &mut [u32]) -> usize {
    unreachable!("scan_neon called on non-aarch64 architecture")
}
