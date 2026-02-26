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

        use std::simd::prelude::*;

        while i + 32 <= bytes.len() {
            let chunk = u8x32::from_slice(&bytes[i..]);

            if in_string {
                let m_quote = chunk.simd_eq(u8x32::splat(b'"')).to_bitmask();
                let m_esc = chunk.simd_eq(u8x32::splat(b'\\')).to_bitmask();

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
                let m_quote = chunk.simd_eq(u8x32::splat(b'"')).to_bitmask();
                let m_lcb = chunk.simd_eq(u8x32::splat(b'{')).to_bitmask();
                let m_rcb = chunk.simd_eq(u8x32::splat(b'}')).to_bitmask();
                let m_lsb = chunk.simd_eq(u8x32::splat(b'[')).to_bitmask();
                let m_rsb = chunk.simd_eq(u8x32::splat(b']')).to_bitmask();
                let m_col = chunk.simd_eq(u8x32::splat(b':')).to_bitmask();
                let m_com = chunk.simd_eq(u8x32::splat(b',')).to_bitmask();

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
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                        if tape_idx < tape.len() {
                            tape[tape_idx] = i as u32;
                            tape_idx += 1;
                        }
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

