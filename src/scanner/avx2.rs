#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// The AVX2 + PCLMULQDQ highly optimized scanner
///
/// # Safety
/// The caller must ensure AVX2 and PCLMULQDQ are available on the current CPU.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,pclmulqdq")]
pub unsafe fn scan_avx2(bytes: &[u8], tape: &mut [u32]) -> usize {
    // For now, redirect to the generic fallback. We will implement
    // the simdjson PCLMULQDQ algorithm here shortly.
    let generic_scanner = crate::scanner::generic::Scanner::new(bytes);
    generic_scanner.scan(tape)
}

#[cfg(not(target_arch = "x86_64"))]
/// # Safety
/// This function is a non-x86_64 stub and always panics.
pub unsafe fn scan_avx2(_bytes: &[u8], _tape: &mut [u32]) -> usize {
    unreachable!("scan_avx2 called on non-x86_64 architecture")
}
