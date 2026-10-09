//! Exact offline comparison primitives for checked-in reference fixtures.

use std::fmt;

/// First observable difference between expected and actual fixture output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactMismatch {
    pub offset: usize,
    pub expected_len: usize,
    pub actual_len: usize,
    pub expected_byte: Option<u8>,
    pub actual_byte: Option<u8>,
}

impl fmt::Display for ExactMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fixture output differs at byte {} (expected_len={}, actual_len={}, expected={:?}, actual={:?})",
            self.offset,
            self.expected_len,
            self.actual_len,
            self.expected_byte,
            self.actual_byte
        )
    }
}

impl std::error::Error for ExactMismatch {}

/// Compare fixture output byte-for-byte without sorting, tolerance, or normalization.
///
/// Any semantic normalization must happen before this boundary and be declared by
/// the fixture manifest. This comparator deliberately cannot hide ordering or
/// missing/present differences.
pub fn compare_exact(expected: &[u8], actual: &[u8]) -> Result<(), ExactMismatch> {
    if expected == actual {
        return Ok(());
    }

    let common_len = expected.len().min(actual.len());
    let offset = expected
        .iter()
        .zip(actual)
        .position(|(left, right)| left != right)
        .unwrap_or(common_len);

    Err(ExactMismatch {
        offset,
        expected_len: expected.len(),
        actual_len: actual.len(),
        expected_byte: expected.get(offset).copied(),
        actual_byte: actual.get(offset).copied(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ExactMismatch, compare_exact};

    #[test]
    fn exact_match_passes() {
        assert_eq!(compare_exact(b"abc\n", b"abc\n"), Ok(()));
    }

    #[test]
    fn reports_first_content_difference() {
        assert_eq!(
            compare_exact(b"abc", b"axc"),
            Err(ExactMismatch {
                offset: 1,
                expected_len: 3,
                actual_len: 3,
                expected_byte: Some(b'b'),
                actual_byte: Some(b'x'),
            })
        );
    }

    #[test]
    fn reports_length_only_difference_at_common_boundary() {
        assert_eq!(
            compare_exact(b"abc", b"abc!"),
            Err(ExactMismatch {
                offset: 3,
                expected_len: 3,
                actual_len: 4,
                expected_byte: None,
                actual_byte: Some(b'!'),
            })
        );
    }
}
