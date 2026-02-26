use crate::string::KowitoStr;

/// A structural representation of JSON data using offsets.
#[derive(Debug, Clone)]
pub enum KowitoNode<'a> {
    Null,
    Bool(bool),
    Number(&'a [u8]), // Delayed number parsing
    String(KowitoStr<'a>),
    Array {
        start_idx: usize,
        len: usize,
    },
    Object {
        start_idx: usize,
        len: usize,
    },
}

/// Takes a `&'a [u8]` and a reference to the structural index (Tape). 
/// Allows jumping directly to offsets without full deserialization.
pub struct KowitoView<'a> {
    pub source: &'a [u8],
    /// The tape is a sequence of structural elements represented as indices or tokens.
    pub tape: &'a [u32], 
}

impl<'a> KowitoView<'a> {
    #[inline(always)]
    pub fn new(source: &'a [u8], tape: &'a [u32]) -> Self {
        Self { source, tape }
    }
    
    // Future methods for random access querying without decoding.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_initialization() {
        let json = b"{\"key\":\"value\"}";
        let tape = vec![0, 1, 5, 6, 7, 13, 14];
        let view = KowitoView::new(json, &tape);
        
        assert_eq!(view.source.len(), 15);
        assert_eq!(view.tape.len(), 7);
    }
}

