pub mod generic;
pub mod avx2;
pub mod neon;
pub mod sve2;
pub mod amx;

pub struct Scanner<'a> {
    input: &'a [u8],
}

impl<'a> Scanner<'a> {
    #[inline(always)]
    pub fn new(input: &'a [u8]) -> Self {
        Self { input }
    }

    /// Scan the entire input and build the tape inside the provided slice.
    /// Employs Runtime JIT Dispatch to use the fastest instruction set available.
    pub fn scan(&self, tape: &mut [u32]) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("pclmulqdq") {
                // Safety: Verified AVX2 and PCLMULQDQ are supported on this CPU at runtime
                return unsafe { avx2::scan_avx2(self.input, tape) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // Note: std::arch::is_aarch64_feature_detected!("sve2") is currently unstable
            // In a production Apple Silicon (M4+) environment, we would detect SVE/SVE2
            // For now, NEON is always available on AArch64.
            // SVE2 will be wired up here when `stdarch_aarch64_sve` stabilizes in rustc.
            
            // Safety: NEON is guaranteed on aarch64.
            return unsafe { neon::scan_neon(self.input, tape) };
        }

        // Fallback to pure portable SIMD (Fast, but without carry-less bitmanipulation)
        let generic_scanner = generic::Scanner::new(self.input);
        generic_scanner.scan(tape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_scan_simple_object() {
        let json = b"{\"key\":\"value\"}";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);
        
        assert_eq!(count, 7);
        assert_eq!(&tape[..count], &[0, 1, 5, 6, 7, 13, 14]);
    }

    #[test]
    fn test_dynamic_scan_with_escaped_quotes() {
        let json = br#"{"key":"val\"ue"}"#;
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);
        
        assert_eq!(count, 7);
        assert_eq!(&tape[..count], &[0, 1, 5, 6, 7, 15, 16]);
    }

    #[test]
    fn test_dynamic_scan_array_and_primitives() {
        let json = b"[1, true, null]";
        let scanner = Scanner::new(json);
        let mut tape = vec![0; 10];
        let count = scanner.scan(&mut tape);
        
        assert_eq!(count, 4);
        assert_eq!(&tape[..count], &[0, 2, 8, 14]);
    }
}
