//! Official 42 identity, run-clock, source-header and inclusion-guard support.
//!
//! This crate is deliberately independent of project discovery. It can
//! recognize and prepare a conservative single-file guard rename, but it never
//! claims that such a rename is safe across an entire project.

#![forbid(unsafe_code)]

mod clock;
mod guard;
mod header;
mod identity;

pub use clock::{ClockError, ClockSource, RunClock};
pub use guard::{
    CanonicalGuard, GuardRenameCandidate, apply_guard_rename, canonical_guard, expected_guard,
    guard_rename_candidate, header_guard_matches,
};
pub use header::{
    C_HEADER_EDGE, HeaderBuildError, HeaderTransform, build_c_header, c_header_filename_matches,
    c_header_fits, c_header_span, ensure_c_header, identity_fits_c_header, update_c_header,
};
pub use identity::{
    Identity42, IdentityResolution, IdentityResolver, canonical_42_email, identity_from_email,
    resolve_identity,
};

/// A half-open UTF-8 byte range.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteRange {
    /// Inclusive byte offset.
    pub start: usize,
    /// Exclusive byte offset.
    pub end: usize,
}

impl ByteRange {
    /// Creates a byte range, asserting no ordering at construction time.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the range length, saturating malformed ranges to zero.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether this is an insertion point.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// An English issue that remains after a conservative operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issue {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable English explanation.
    pub message: String,
    /// Relevant range in the input snapshot.
    pub range: ByteRange,
    /// Concrete next action.
    pub suggestion: String,
}

/// An accepted source edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fix {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable English description.
    pub description: String,
    /// Replaced range in the input snapshot; insertions are empty ranges.
    pub range: ByteRange,
}
