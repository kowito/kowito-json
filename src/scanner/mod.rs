#[cfg(target_arch = "aarch64")]
pub mod amx;
#[cfg(target_arch = "x86_64")]
pub mod avx2;
pub mod generic;
#[cfg(target_arch = "aarch64")]
pub mod neon;
#[cfg(target_arch = "aarch64")]
pub mod sve2;

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
        // Reachable on non-aarch64 and non-x86_64_with_avx2 architectures
        #[allow(unreachable_code)]
        {
            let generic_scanner = generic::Scanner::new(self.input);
            generic_scanner.scan(tape)
        }
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

    /// Regression test for cross-block string detection.
    ///
    /// The input is 133 bytes, forcing two full 64-byte SIMD iterations.
    /// A `[` sits at byte 64 (first byte of the second 64-byte block) inside a
    /// string value that opens at byte 61 and closes at byte 121.  The scanner
    /// must:
    ///   1. NOT emit position 64 (`[` inside string).
    ///   2. Carry the in-string state correctly from block 0 into block 1.
    ///   3. Handle the 5-byte scalar tail (bytes 128-132) with the correct
    ///      out-of-string carry (the value string was already closed at 121).
    ///
    /// Layout (block boundaries at byte 0 / 64 / 128):
    ///   block 0 (bytes   0– 63)
    ///     0      : {
    ///     1      : " (open key)
    ///     2– 58  : A × 57  (key chars)
    ///     59     : " (close key)
    ///     60     : :
    ///     61     : " (open value)
    ///     62– 63 : a × 2   (string interior, end of block 0)
    ///   block 1 (bytes  64–127)
    ///     64     : [  ← inside string → SUPPRESSED
    ///     65–120 : a × 56  (string interior)
    ///     121    : " (close value)
    ///     122    : ,
    ///     123    : " (open k2)
    ///     124–125: k 2
    ///     126    : " (close k2)
    ///     127    : :
    ///   scalar tail (bytes 128–132)  — starts OUTSIDE string
    ///     128    : " (open v2)
    ///     129–130: v 2
    ///     131    : " (close v2)
    ///     132    : }
    ///
    /// Expected tape: [0,1,59,60,61, 121,122,123,126,127,128,131,132]
    #[test]
    fn test_dynamic_scan_string_spans_64byte_boundary() {
        let mut json = Vec::with_capacity(140);
        json.extend_from_slice(b"{\""); //   0– 1 : { "
        json.extend(std::iter::repeat_n(b'A', 57)); //   2–58 : key (57 As)
        json.extend_from_slice(b"\":\""); // 59–61 : " : "
        json.extend(std::iter::repeat_n(b'a', 2)); // 62–63 : end of block 0 (inside string)
        // --- 64-byte block boundary ---
        json.extend_from_slice(b"["); //    64 : structural INSIDE string → suppressed
        json.extend(std::iter::repeat_n(b'a', 56)); // 65–120: string interior
        json.extend_from_slice(b"\",\"k2\":"); // 121–127: " , " k 2 " :
        // --- scalar tail (5 bytes, starts outside string) ---
        json.extend_from_slice(b"\"v2\"}"); // 128–132: " v 2 " }

        assert_eq!(json.len(), 133, "input length sanity check");

        let scanner = Scanner::new(&json);
        let mut tape = vec![0u32; 20];
        let count = scanner.scan(&mut tape);

        assert_eq!(
            &tape[..count],
            &[0, 1, 59, 60, 61, 121, 122, 123, 126, 127, 128, 131, 132],
            "unexpected tape; count={count}"
        );
    }
}
