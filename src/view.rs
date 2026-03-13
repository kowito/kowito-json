use crate::string::KString;

/// A structural representation of JSON data using offsets.
#[derive(Debug, Clone)]
pub enum KNode<'a> {
    Null,
    Bool(bool),
    Number(&'a [u8]), // Delayed number parsing
    String(KString<'a>),
    Array { start_idx: usize, len: usize },
    Object { start_idx: usize, len: usize },
}

/// Takes a `&'a [u8]` and a reference to the structural index (Tape).
/// Allows jumping directly to offsets without full deserialization.
pub struct KView<'a> {
    pub source: &'a [u8],
    /// The tape is a sequence of structural elements represented as indices or tokens.
    pub tape: &'a [u32],
}

impl<'a> KView<'a> {
    #[inline(always)]
    pub fn new(source: &'a [u8], tape: &'a [u32]) -> Self {
        Self { source, tape }
    }

    // Future methods for random access querying without decoding.
}

#[cfg(test)]
#[allow(clippy::identity_op)]
mod tests {
    use super::*;
    use crate::scanner::{TOKEN_COLON, TOKEN_LBRACE, TOKEN_QUOTE, TOKEN_RBRACE};

    #[test]
    fn test_view_initialization() {
        let json = b"{\"key\":\"value\"}";
        let tape = vec![
            TOKEN_LBRACE | 0,
            TOKEN_QUOTE | 1,
            TOKEN_QUOTE | 5,
            TOKEN_COLON | 6,
            TOKEN_QUOTE | 7,
            TOKEN_QUOTE | 13,
            TOKEN_RBRACE | 14,
        ];
        let view = KView::new(json, &tape);

        assert_eq!(view.source.len(), 15);
        assert_eq!(view.tape.len(), 7);
    }
}
