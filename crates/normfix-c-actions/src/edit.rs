//! Deterministic byte edit application.

use normfix_core::{TextRange, TextSize};
use thiserror::Error;

/// One replacement against a specific UTF-8 source version.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Edit {
    /// Half-open UTF-8 byte range.
    pub range: TextRange,
    /// Replacement text.
    pub replacement: String,
    /// Stable rule identifier.
    pub rule_id: String,
    /// Concise English description.
    pub description: String,
    /// One-based physical line when known.
    pub line: Option<u32>,
}

impl Edit {
    /// Creates an edit after validating that its byte offsets fit the shared
    /// compact range representation.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::RangeTooLarge`] for offsets above `u32::MAX`, or
    /// [`EditError::ReversedRange`] when `start > end`.
    pub fn new(
        start: usize,
        end: usize,
        replacement: impl Into<String>,
        rule_id: impl Into<String>,
        description: impl Into<String>,
        line: Option<u32>,
    ) -> Result<Self, EditError> {
        if start > end {
            return Err(EditError::ReversedRange { start, end });
        }
        let start_size =
            TextSize::try_from(start).map_err(|_| EditError::RangeTooLarge { offset: start })?;
        let end_size =
            TextSize::try_from(end).map_err(|_| EditError::RangeTooLarge { offset: end })?;
        let range =
            TextRange::new(start_size, end_size).ok_or(EditError::ReversedRange { start, end })?;
        Ok(Self {
            range,
            replacement: replacement.into(),
            rule_id: rule_id.into(),
            description: description.into(),
            line,
        })
    }

    pub(crate) fn start(&self) -> usize {
        self.range.start().get() as usize
    }

    pub(crate) fn end(&self) -> usize {
        self.range.end().get() as usize
    }
}

/// Invalid edit set.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EditError {
    /// A byte offset exceeded the compact source range representation.
    #[error("edit byte offset {offset} exceeds the supported range")]
    RangeTooLarge {
        /// Rejected byte offset.
        offset: usize,
    },
    /// End preceded start.
    #[error("edit range is reversed: {start}..{end}")]
    ReversedRange {
        /// Start byte.
        start: usize,
        /// End byte.
        end: usize,
    },
    /// Range exceeded the current source.
    #[error("edit range {start}..{end} exceeds source length {source_len}")]
    OutOfBounds {
        /// Start byte.
        start: usize,
        /// End byte.
        end: usize,
        /// Source length.
        source_len: usize,
    },
    /// Offset split a UTF-8 scalar.
    #[error("edit range {start}..{end} does not follow UTF-8 boundaries")]
    InvalidUtf8Boundary {
        /// Start byte.
        start: usize,
        /// End byte.
        end: usize,
    },
    /// Two different edits overlap or insert at one boundary.
    #[error("conflicting edits {first_start}..{first_end} and {second_start}..{second_end}")]
    Conflict {
        /// First range start.
        first_start: usize,
        /// First range end.
        first_end: usize,
        /// Second range start.
        second_start: usize,
        /// Second range end.
        second_end: usize,
    },
}

/// Applies a deterministic, non-overlapping edit set from right to left.
///
/// Exact duplicates and no-op replacements are omitted. Unlike the legacy
/// Python implementation, conflicting edits are rejected rather than silently
/// dropped.
///
/// # Errors
///
/// Returns an error for invalid UTF-8 ranges, out-of-bounds offsets, or
/// overlapping edits.
pub fn apply_edits(source: &str, edits: &[Edit]) -> Result<(String, Vec<Edit>), EditError> {
    let mut ordered = edits.to_vec();
    ordered.sort_by(|left, right| {
        (
            left.start(),
            left.end(),
            &left.replacement,
            &left.rule_id,
            &left.description,
        )
            .cmp(&(
                right.start(),
                right.end(),
                &right.replacement,
                &right.rule_id,
                &right.description,
            ))
    });
    ordered.dedup_by(|left, right| {
        left.start() == right.start()
            && left.end() == right.end()
            && left.replacement == right.replacement
    });
    for edit in &ordered {
        if edit.end() > source.len() {
            return Err(EditError::OutOfBounds {
                start: edit.start(),
                end: edit.end(),
                source_len: source.len(),
            });
        }
        if !source.is_char_boundary(edit.start()) || !source.is_char_boundary(edit.end()) {
            return Err(EditError::InvalidUtf8Boundary {
                start: edit.start(),
                end: edit.end(),
            });
        }
    }
    ordered.retain(|edit| {
        source
            .get(edit.start()..edit.end())
            .is_some_and(|current| current != edit.replacement)
    });
    for pair in ordered.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let overlaps = right.start() < left.end();
        let same_insertion = left.start() == left.end()
            && right.start() == right.end()
            && left.start() == right.start();
        let insertion_inside_left = right.start() == right.end()
            && right.start() < left.end()
            && right.start() >= left.start();
        let insertion_inside_right = left.start() == left.end()
            && left.start() >= right.start()
            && left.start() < right.end();
        if overlaps || same_insertion || insertion_inside_left || insertion_inside_right {
            return Err(EditError::Conflict {
                first_start: left.start(),
                first_end: left.end(),
                second_start: right.start(),
                second_end: right.end(),
            });
        }
    }

    let mut result = source.to_owned();
    for edit in ordered.iter().rev() {
        result.replace_range(edit.start()..edit.end(), &edit.replacement);
    }
    Ok((result, ordered))
}

#[cfg(test)]
mod tests {
    use super::{Edit, EditError, apply_edits};

    #[test]
    fn applies_adjacent_edits_deterministically() {
        let edits = vec![
            Edit::new(1, 2, "B", "B", "second", Some(1)).unwrap(),
            Edit::new(0, 1, "A", "A", "first", Some(1)).unwrap(),
        ];
        let (result, accepted) = apply_edits("xy", &edits).unwrap();
        assert_eq!(result, "AB");
        assert_eq!(accepted[0].rule_id, "A");
    }

    #[test]
    fn rejects_conflicting_edits() {
        let edits = vec![
            Edit::new(0, 2, "x", "A", "first", None).unwrap(),
            Edit::new(1, 2, "y", "B", "second", None).unwrap(),
        ];
        assert!(matches!(
            apply_edits("abc", &edits),
            Err(EditError::Conflict { .. })
        ));
    }

    #[test]
    fn rejects_out_of_bounds_and_split_utf8_ranges() {
        let out_of_bounds = Edit::new(0, 8, "", "A", "bad range", None).unwrap();
        assert!(matches!(
            apply_edits("abc", &[out_of_bounds]),
            Err(EditError::OutOfBounds { .. })
        ));

        let split_scalar = Edit::new(1, 2, "", "B", "split scalar", None).unwrap();
        assert!(matches!(
            apply_edits("é", &[split_scalar]),
            Err(EditError::InvalidUtf8Boundary { .. })
        ));
    }
}
