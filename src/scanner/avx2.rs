#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// The AVX2 + PCLMULQDQ highly optimized scanner
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,pclmulqdq")]
pub unsafe fn scan_avx2(bytes: &[u8], tape: &mut [u32]) -> usize {
    // For now, redirect to the generic fallback. We will implement 
    // the simdjson PCLMULQDQ algorithm here shortly.
    let generic_scanner = crate::scanner::generic::Scanner::new(bytes);
    generic_scanner.scan(tape)
}

#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn scan_avx2(_bytes: &[u8], _tape: &mut [u32]) -> usize {
    unreachable!("scan_avx2 called on non-x86_64 architecture")
}
