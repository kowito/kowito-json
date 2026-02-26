use std::borrow::Cow;

/// A lazy string wrapper that stores a slice and an `escaped` flag. Decoding only happens on access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KowitString<'a> {
    raw: &'a [u8],
    has_escapes: bool,
}

impl<'a> KowitString<'a> {
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
    pub fn decode(&self) -> Cow<'a, str> {
        if !self.has_escapes {
            // Unsafe is okay here if we validate UTF-8 during scanning.
            // For now, we assume valid UTF-8.
            let s = unsafe { std::str::from_utf8_unchecked(self.raw) };
            Cow::Borrowed(s)
        } else {
            // Lazy decoding of escapes.
            let mut decoded = String::with_capacity(self.raw.len());
            let mut i = 0;
            while i < self.raw.len() {
                if self.raw[i] == b'\\' {
                    if i + 1 < self.raw.len() {
                        match self.raw[i + 1] {
                            b'"' => { decoded.push('"'); i += 2; },
                            b'\\' => { decoded.push('\\'); i += 2; },
                            b'/' => { decoded.push('/'); i += 2; },
                            b'b' => { decoded.push('\x08'); i += 2; },
                            b'f' => { decoded.push('\x0C'); i += 2; },
                            b'n' => { decoded.push('\n'); i += 2; },
                            b'r' => { decoded.push('\r'); i += 2; },
                            b't' => { decoded.push('\t'); i += 2; },
                            b'u' => {
                                // Unicode escape parsing would go here.
                                // Simplified for the baseline.
                                i += 6; // skip the \u and the 4 hex digits
                            }
                            _ => {
                                decoded.push(self.raw[i] as char);
                                i += 1;
                            }
                        }
                    } else {
                        // Trailing backslash, just ignore or push
                        i += 1;
                    }
                } else {
                    decoded.push(self.raw[i] as char); // Assuming ASCII for now
                    i += 1;
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
        let s = KowitString::new(b"", false);
        assert_eq!(s.as_raw(), b"");
        assert!(!s.has_escapes());
        assert_eq!(s.decode(), "");
    }

    #[test]
    fn test_basic_string() {
        let s = KowitString::new(b"hello world", false);
        assert_eq!(s.as_raw(), b"hello world");
        assert!(!s.has_escapes());
        assert_eq!(s.decode(), "hello world");
    }

    #[test]
    fn test_simple_escapes() {
        let s = KowitString::new(br#"line\nbreak"#, true);
        assert!(s.has_escapes());
        assert_eq!(s.decode(), "line\nbreak");
    }

    #[test]
    fn test_all_control_escapes() {
        let raw = br#"\"\/\b\f\n\r\t\\"#;
        let s = KowitString::new(raw, true);
        assert!(s.has_escapes());
        assert_eq!(s.decode(), "\"/\\x08\\x0C\n\r\t\\".replace("\\x08", "\x08").replace("\\x0C", "\x0C"));
    }

    #[test]
    fn test_unicode_escape_skip() {
        // Our baseline skips unicode evaluation for now, testing the skip behavior
        let raw = br#"hello\u1234world"#;
        let s = KowitString::new(raw, true);
        assert!(s.has_escapes());
        assert_eq!(s.decode(), "helloworld"); // Validates it skipped \u1234
    }

    #[test]
    fn test_invalid_escape_at_end() {
        let raw = br#"hello\"#;
        let s = KowitString::new(raw, true);
        assert!(s.has_escapes());
        // Since it's invalid it should just drop it/handle gracefully without panic
        assert_eq!(s.decode(), "hello");
    }
}

