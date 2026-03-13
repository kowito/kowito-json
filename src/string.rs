use std::borrow::Cow;

/// A lazy string wrapper that stores a slice and an `escaped` flag. Decoding only happens on access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KString<'a> {
    raw: &'a [u8],
    has_escapes: bool,
}

impl<'a> KString<'a> {
    #[inline(always)]
    pub fn new(raw: &'a [u8], has_escapes: bool) -> Self {
        Self { raw, has_escapes }
    }

    #[inline(always)]
    pub fn as_raw(&self) -> &'a [u8] {
        self.raw
    }

    #[inline(always)]
    pub fn has_escapes(&self) -> bool {
        self.has_escapes
    }

    /// Returns the decoded string as a `Cow`.
    /// If there are no escapes, it returns a borrowed string.
    /// If there are escapes, it allocates and returns an owned string.
    /// Uses a "clean run" optimization to bulk-copy non-escaped segments.
    pub fn decode(&self) -> Cow<'a, str> {
        if !self.has_escapes {
            // Unsafe is okay here if we validate UTF-8 during scanning.
            // For now, we assume valid UTF-8.
            let s = unsafe { std::str::from_utf8_unchecked(self.raw) };
            Cow::Borrowed(s)
        } else {
            let mut decoded = String::with_capacity(self.raw.len());
            let bytes = self.raw;
            let mut i = 0;
            let mut start = 0;

            while i < bytes.len() {
                // Find next backslash using optimized iterator position (often SIMD-backed by rustc)
                if let Some(rel_pos) = bytes[i..].iter().position(|&b| b == b'\\') {
                    let pos = i + rel_pos;
                    // Bulk-copy the clean run before the backslash
                    if start < pos {
                        decoded.push_str(unsafe {
                            std::str::from_utf8_unchecked(bytes.get_unchecked(start..pos))
                        });
                    }

                    // Process the escape sequence
                    if pos + 1 < bytes.len() {
                        match unsafe { *bytes.get_unchecked(pos + 1) } {
                            b'"' => decoded.push('"'),
                            b'\\' => decoded.push('\\'),
                            b'/' => decoded.push('/'),
                            b'b' => decoded.push('\x08'),
                            b'f' => decoded.push('\x0C'),
                            b'n' => decoded.push('\n'),
                            b'r' => decoded.push('\r'),
                            b't' => decoded.push('\t'),
                            b'u' => {
                                // Baseline behavior: skip unicode escapes (\uXXXX)
                                i = pos + 6;
                                start = i;
                                continue;
                            }
                            other => {
                                // Invalid escape, keep it as is (backslash + char)
                                decoded.push('\\');
                                decoded.push(other as char);
                            }
                        }
                        i = pos + 2;
                        start = i;
                    } else {
                        // Trailing backslash
                        i = bytes.len();
                        start = i;
                    }
                } else {
                    // No more backslashes found, copy the remaining tail
                    if start < bytes.len() {
                        decoded.push_str(unsafe {
                            std::str::from_utf8_unchecked(bytes.get_unchecked(start..))
                        });
                    }
                    break;
                }
            }
            Cow::Owned(decoded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_string() {
        let s = KString::new(b"", false);
        assert_eq!(s.as_raw(), b"");
        assert!(!s.has_escapes());
        assert_eq!(s.decode(), "");
    }

    #[test]
    fn test_basic_string() {
        let s = KString::new(b"hello world", false);
        assert_eq!(s.as_raw(), b"hello world");
        assert!(!s.has_escapes());
        assert_eq!(s.decode(), "hello world");
    }

    #[test]
    fn test_simple_escapes() {
        let s = KString::new(br#"line\nbreak"#, true);
        assert!(s.has_escapes());
        assert_eq!(s.decode(), "line\nbreak");
    }

    #[test]
    fn test_all_control_escapes() {
        let raw = br#"\"\/\b\f\n\r\t\\"#;
        let s = KString::new(raw, true);
        assert!(s.has_escapes());
        assert_eq!(
            s.decode(),
            "\"/\\x08\\x0C\n\r\t\\"
                .replace("\\x08", "\x08")
                .replace("\\x0C", "\x0C")
        );
    }

    #[test]
    fn test_unicode_escape_skip() {
        // Our baseline skips unicode evaluation for now, testing the skip behavior
        let raw = br#"hello\u1234world"#;
        let s = KString::new(raw, true);
        assert!(s.has_escapes());
        assert_eq!(s.decode(), "helloworld"); // Validates it skipped \u1234
    }

    #[test]
    fn test_invalid_escape_at_end() {
        let raw = br#"hello\"#;
        let s = KString::new(raw, true);
        assert!(s.has_escapes());
        // Since it's invalid it should just drop it/handle gracefully without panic
        assert_eq!(s.decode(), "hello");
    }
}
