//! Deterministic, backend-neutral data types shared by the Rust engine.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use unicode_width::UnicodeWidthChar as _;

/// A UTF-8 byte offset small enough for compact syntax data structures.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct TextSize(u32);

impl TextSize {
    /// Creates an offset from its compact representation.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying byte count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Adds two offsets without wrapping.
    #[must_use]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl From<u32> for TextSize {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<TextSize> for u32 {
    fn from(value: TextSize) -> Self {
        value.0
    }
}

impl TryFrom<usize> for TextSize {
    type Error = TextSizeOverflow;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| TextSizeOverflow { bytes: value })
    }
}

impl TryFrom<TextSize> for usize {
    type Error = TextSizeOverflow;

    fn try_from(value: TextSize) -> Result<Self, Self::Error> {
        usize::try_from(value.0).map_err(|_| TextSizeOverflow {
            bytes: value.0 as usize,
        })
    }
}

/// A source was too large for [`TextSize`].
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("source contains {bytes} bytes, exceeding compact text offsets")]
pub struct TextSizeOverflow {
    /// The rejected byte count.
    pub bytes: usize,
}

/// A half-open UTF-8 byte range.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

impl TextRange {
    /// Creates `start..end`, or returns `None` when the bounds are reversed.
    #[must_use]
    pub const fn new(start: TextSize, end: TextSize) -> Option<Self> {
        if start.get() <= end.get() {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Creates an empty range at `offset`.
    #[must_use]
    pub const fn empty(offset: TextSize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Returns the inclusive start offset.
    #[must_use]
    pub const fn start(self) -> TextSize {
        self.start
    }

    /// Returns the exclusive end offset.
    #[must_use]
    pub const fn end(self) -> TextSize {
        self.end
    }

    /// Returns the range length.
    #[must_use]
    pub const fn len(self) -> TextSize {
        TextSize::new(self.end.get() - self.start.get())
    }

    /// Returns whether this range is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.get() == self.end.get()
    }

    /// Returns whether `offset` lies in this half-open range.
    #[must_use]
    pub const fn contains(self, offset: TextSize) -> bool {
        self.start.get() <= offset.get() && offset.get() < self.end.get()
    }

    /// Returns whether this range overlaps `other`.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.start.get() < other.end.get() && other.start.get() < self.end.get()
    }
}

/// Stable identifier assigned to one file within a project snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FileId(u32);

impl FileId {
    /// Creates a file identifier.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the numeric representation.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One-based source position.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LineColumn {
    /// One-based physical line.
    pub line: u32,
    /// One-based UTF-8 byte column.
    pub byte_column: u32,
    /// One-based visual column using four-column tab stops.
    pub visual_column: u32,
}

/// Index for deterministic UTF-8 byte-offset to line/column conversion.
#[derive(Clone, Debug)]
pub struct LineIndex {
    source: Arc<str>,
    line_starts: Vec<TextSize>,
}

impl LineIndex {
    /// Indexes `source`.
    ///
    /// # Errors
    ///
    /// Returns [`TextSizeOverflow`] when the source cannot use compact offsets.
    pub fn new(source: Arc<str>) -> Result<Self, TextSizeOverflow> {
        TextSize::try_from(source.len())?;
        let mut line_starts = vec![TextSize::new(0)];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(TextSize::try_from(index + 1)?);
            }
        }
        Ok(Self {
            source,
            line_starts,
        })
    }

    /// Returns the number of physical lines, including an empty line after a final newline.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Converts a UTF-8 byte offset to a one-based source position.
    ///
    /// Returns `None` for offsets outside the source or inside a multibyte
    /// character.
    #[must_use]
    pub fn line_column(&self, offset: TextSize) -> Option<LineColumn> {
        let offset = usize::try_from(offset).ok()?;
        if offset > self.source.len() || !self.source.is_char_boundary(offset) {
            return None;
        }
        let line_index = self
            .line_starts
            .partition_point(|start| usize::try_from(*start).is_ok_and(|start| start <= offset))
            .checked_sub(1)?;
        let line_start = usize::try_from(self.line_starts[line_index]).ok()?;
        let prefix = self.source.get(line_start..offset)?;
        let byte_column = u32::try_from(offset - line_start).ok()?.checked_add(1)?;
        let visual_column = visual_width(prefix, 1);
        Some(LineColumn {
            line: u32::try_from(line_index).ok()?.checked_add(1)?,
            byte_column,
            visual_column,
        })
    }

    /// Returns the source bytes covered by a one-based physical line.
    ///
    /// The returned range includes the physical line ending when one exists.
    #[must_use]
    pub fn line_range(&self, line: u32) -> Option<TextRange> {
        let index = usize::try_from(line.checked_sub(1)?).ok()?;
        let start = *self.line_starts.get(index)?;
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(TextSize::try_from(self.source.len()).ok()?);
        TextRange::new(start, end)
    }
}

/// Computes the next visual column after `text`.
///
/// Columns are one-based and tabs advance to the next four-column tab stop.
/// Non-tab characters advance by their terminal display width. This treats
/// combining marks as zero columns and wide characters as two columns.
#[must_use]
pub fn visual_width(text: &str, start_column: u32) -> u32 {
    text.chars().fold(start_column, |column, character| {
        if character == '\t' {
            column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
        } else {
            let width = u32::try_from(character.width().unwrap_or_default()).unwrap_or(u32::MAX);
            column.saturating_add(width)
        }
    })
}

/// Immutable bytes and derived indexes for one analyzed file.
#[derive(Clone, Debug)]
pub struct SourceSnapshot {
    file_id: FileId,
    relative_path: Utf8PathBuf,
    text: Arc<str>,
    content_hash: blake3::Hash,
    line_index: LineIndex,
}

impl SourceSnapshot {
    /// Creates a validated project-relative source snapshot.
    ///
    /// # Errors
    ///
    /// Rejects absolute paths, parent traversal and oversized source text.
    pub fn new(
        file_id: FileId,
        relative_path: Utf8PathBuf,
        text: Arc<str>,
    ) -> Result<Self, SnapshotError> {
        validate_relative_path(&relative_path)?;
        let line_index = LineIndex::new(Arc::clone(&text))?;
        let content_hash = blake3::hash(text.as_bytes());
        Ok(Self {
            file_id,
            relative_path,
            text,
            content_hash,
            line_index,
        })
    }

    /// Returns the snapshot-local file identifier.
    #[must_use]
    pub const fn file_id(&self) -> FileId {
        self.file_id
    }

    /// Returns the normalized project-relative path supplied by discovery.
    #[must_use]
    pub fn relative_path(&self) -> &camino::Utf8Path {
        &self.relative_path
    }

    /// Returns the immutable UTF-8 source.
    #[must_use]
    pub fn text(&self) -> &Arc<str> {
        &self.text
    }

    /// Returns the BLAKE3 content hash.
    #[must_use]
    pub const fn content_hash(&self) -> blake3::Hash {
        self.content_hash
    }

    /// Returns the line index for this exact source.
    #[must_use]
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

fn validate_relative_path(path: &camino::Utf8Path) -> Result<(), SnapshotError> {
    if path.as_str().is_empty() || path.is_absolute() {
        return Err(SnapshotError::InvalidRelativePath(path.to_owned()));
    }
    if path
        .components()
        .any(|component| matches!(component, camino::Utf8Component::ParentDir))
    {
        return Err(SnapshotError::InvalidRelativePath(path.to_owned()));
    }
    Ok(())
}

/// A source snapshot could not be constructed safely.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SnapshotError {
    /// The supplied path was absolute, empty or contained `..`.
    #[error("source path must be non-empty, relative and contain no parent traversal: {0}")]
    InvalidRelativePath(Utf8PathBuf),
    /// The source exceeded compact text offsets.
    #[error(transparent)]
    SourceTooLarge(#[from] TextSizeOverflow),
}

/// Safety class attached to every proposed action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    /// Trivia-only formatting with token and diagnostic proofs.
    SafeLayout,
    /// Token-changing action with a complete semantic proof.
    SafeSemantic,
    /// Useful proposal that still needs human judgment.
    ReviewRequired,
    /// Deletion, movement or behavior/API change requiring explicit consent.
    UnsafeDestructive,
}

/// Origin of one diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "version", rename_all = "snake_case")]
pub enum DiagnosticSource {
    /// Native interpretation of the 42 Norm.
    NativeNorm41,
    /// Compatibility behavior of a specific official Norminette release.
    NorminetteCompat(String),
    /// Syntax parser or recovery.
    Parser,
    /// External compiler verification.
    Compiler,
    /// Project graph or filesystem policy.
    Project,
    /// Makefile-specific analysis.
    Makefile,
    /// Markdown-specific analysis.
    Markdown,
    /// A leak checker that observed the program while it ran.
    LeakChecker,
}

/// Diagnostic severity, ordered from most to least severe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Processing or correctness error.
    Error,
    /// Problem requiring attention.
    Warning,
    /// Informational result.
    Info,
}

/// One user-visible record describing an accepted transformation.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FixRecord {
    /// Stable Norm or native rule identifier.
    pub rule_id: String,
    /// English explanation of the change.
    pub description: String,
    /// One-based source line when a single location is meaningful.
    pub line: Option<u32>,
    /// Number of equivalent edits represented by this record.
    pub count: u32,
}

/// A deterministic replacement in one immutable source snapshot.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SourceEdit {
    /// Half-open UTF-8 byte range replaced by this edit.
    pub range: TextRange,
    /// Exact UTF-8 replacement.
    pub replacement: String,
    /// Rule responsible for the edit.
    pub rule_id: String,
    /// English explanation shown in verbose output.
    pub description: String,
    /// Safety classification assigned by the proposing rule.
    pub applicability: Applicability,
}

/// Evidence required before an action may leave a shadow buffer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofRequirement {
    /// Source ranges are valid, non-overlapping UTF-8 byte boundaries.
    ValidRanges,
    /// The lossless tape reconstructs the complete candidate.
    LosslessRoundTrip,
    /// Significant C tokens are identical before and after.
    SignificantTokensUnchanged,
    /// C parsing did not introduce a new recovery region.
    NoNewSyntaxRecovery,
    /// No new compatibility diagnostic was introduced.
    NoNewDiagnostics,
    /// Every explicitly targeted diagnostic improved.
    TargetDiagnosticsImproved,
    /// The same rule set proposes no second change.
    Idempotent,
    /// Every affected line respects the configured visual width.
    VisualWidth,
    /// A compiler accepted all known translation units.
    CompilerAccepted,
    /// A project-wide semantic proof established equivalent bindings and values.
    SemanticEquivalence,
    /// Explicit destructive authorization was granted.
    DestructiveAuthorization,
}

/// The result of checking one proof requirement.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProofResult {
    /// Requirement that was evaluated.
    pub requirement: ProofRequirement,
    /// Whether the requirement was proven.
    pub passed: bool,
    /// Deterministic explanation or evidence summary.
    pub detail: String,
}

/// One rule proposal before conflict resolution and validation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionPlan {
    /// Stable action identifier.
    pub action_id: String,
    /// Project-relative UTF-8 path.
    pub path: Utf8PathBuf,
    /// Safety class of the whole action.
    pub applicability: Applicability,
    /// Replacements sorted during validation.
    pub edits: Vec<SourceEdit>,
    /// Stable diagnostic identities that this action intends to improve.
    pub target_diagnostics: Vec<String>,
    /// Proofs required before application.
    pub required_proofs: Vec<ProofRequirement>,
    /// Proof results collected against the shadow buffer.
    pub proofs: Vec<ProofResult>,
}

impl ActionPlan {
    /// Returns whether all required proofs have a passing result.
    #[must_use]
    pub fn is_proven(&self) -> bool {
        self.required_proofs.iter().all(|required| {
            self.proofs
                .iter()
                .any(|proof| proof.requirement == *required && proof.passed)
        })
    }
}

/// An edit batch could not be applied safely.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EditError {
    /// A range exceeded the source or split a UTF-8 scalar.
    #[error("edit from byte {start} to {end} is not a valid UTF-8 source range")]
    InvalidRange {
        /// Inclusive start byte.
        start: u32,
        /// Exclusive end byte.
        end: u32,
    },
    /// Two non-identical edits overlap or insert at the same boundary.
    #[error(
        "edits `{first_rule}` and `{second_rule}` conflict at byte ranges \
         {first_start}..{first_end} and {second_start}..{second_end}"
    )]
    Conflict {
        /// First rule in stable order.
        first_rule: String,
        /// Second rule in stable order.
        second_rule: String,
        /// First inclusive start.
        first_start: u32,
        /// First exclusive end.
        first_end: u32,
        /// Second inclusive start.
        second_start: u32,
        /// Second exclusive end.
        second_end: u32,
    },
}

/// Validates and applies a complete edit batch to an immutable source.
///
/// Exact duplicate edits are collapsed. Every other conflict rejects the
/// whole batch; no best-effort subset is ever emitted.
///
/// # Errors
///
/// Returns [`EditError`] for invalid UTF-8 ranges or conflicting edits.
pub fn apply_source_edits(source: &str, edits: &[SourceEdit]) -> Result<String, EditError> {
    let mut ordered = edits.to_vec();
    ordered.sort();
    ordered.dedup();
    for edit in &ordered {
        let start = edit.range.start().get();
        let end = edit.range.end().get();
        let start_index =
            usize::try_from(start).map_err(|_| EditError::InvalidRange { start, end })?;
        let end_index = usize::try_from(end).map_err(|_| EditError::InvalidRange { start, end })?;
        if start_index > source.len()
            || end_index > source.len()
            || !source.is_char_boundary(start_index)
            || !source.is_char_boundary(end_index)
        {
            return Err(EditError::InvalidRange { start, end });
        }
    }
    for pair in ordered.windows(2) {
        let first = &pair[0];
        let second = &pair[1];
        let first_start = first.range.start().get();
        let first_end = first.range.end().get();
        let second_start = second.range.start().get();
        let second_end = second.range.end().get();
        let first_empty = first_start == first_end;
        let second_empty = second_start == second_end;
        let conflicts = if first_empty && second_empty {
            first_start == second_start
        } else if first_empty {
            second_start <= first_start && first_start < second_end
        } else if second_empty {
            first_start <= second_start && second_start < first_end
        } else {
            first_start.max(second_start) < first_end.min(second_end)
        };
        if conflicts {
            return Err(EditError::Conflict {
                first_rule: first.rule_id.clone(),
                second_rule: second.rule_id.clone(),
                first_start,
                first_end,
                second_start,
                second_end,
            });
        }
    }
    let mut result = source.to_owned();
    for edit in ordered.iter().rev() {
        let start =
            usize::try_from(edit.range.start().get()).map_err(|_| EditError::InvalidRange {
                start: edit.range.start().get(),
                end: edit.range.end().get(),
            })?;
        let end = usize::try_from(edit.range.end().get()).map_err(|_| EditError::InvalidRange {
            start: edit.range.start().get(),
            end: edit.range.end().get(),
        })?;
        result.replace_range(start..end, &edit.replacement);
    }
    Ok(result)
}

/// A diagnostic's text rendered in the reader's language.
///
/// This never reaches JSON and never participates in equality or ordering. The
/// English text is a diagnostic's identity: translating one must not change
/// which diagnostics count as duplicates, or the order they are reported in.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Localized {
    /// Translated summary.
    pub message: String,
    /// Translated context, in the same order as the English notes.
    pub notes: Vec<String>,
    /// Translated next step.
    pub help: Option<String>,
}

/// Backend-neutral, deterministically ordered diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Project-relative UTF-8 path.
    pub path: Utf8PathBuf,
    /// Primary source range.
    pub range: TextRange,
    /// Severity.
    pub severity: Severity,
    /// Human-readable summary.
    pub message: String,
    /// Diagnostic producer.
    pub source: DiagnosticSource,
    /// Additional context in stable order.
    pub notes: Vec<String>,
    /// Concrete next step.
    pub help: Option<String>,
    /// The same text in the reader's language, when it is not English.
    ///
    /// Absent for a message produced by the official checker or the C
    /// compiler: that text is those tools' own output, not this project's.
    #[serde(skip)]
    pub localized: Option<Localized>,
}

// Equality deliberately ignores `localized`. `dedup` relies on it, and two
// diagnostics that differ only by the reader's language are the same finding.
impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        self.rule_id == other.rule_id
            && self.path == other.path
            && self.range == other.range
            && self.severity == other.severity
            && self.message == other.message
            && self.source == other.source
            && self.notes == other.notes
            && self.help == other.help
    }
}

impl Ord for Diagnostic {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            &self.path,
            self.range,
            self.severity,
            &self.rule_id,
            &self.message,
            &self.source,
            &self.notes,
            &self.help,
        )
            .cmp(&(
                &other.path,
                other.range,
                other.severity,
                &other.rule_id,
                &other.message,
                &other.source,
                &other.notes,
                &other.help,
            ))
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}..{} [{}] {}",
            self.path,
            self.range.start().get(),
            self.range.end().get(),
            self.rule_id,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use camino::Utf8PathBuf;

    use super::{
        Applicability, Diagnostic, DiagnosticSource, EditError, FileId, LineColumn, LineIndex,
        Severity, SnapshotError, SourceEdit, SourceSnapshot, TextRange, TextSize,
        apply_source_edits, visual_width,
    };

    #[test]
    fn text_ranges_reject_reversed_bounds_and_are_half_open() {
        assert!(TextRange::new(TextSize::new(4), TextSize::new(3)).is_none());
        let range = TextRange::new(TextSize::new(3), TextSize::new(7)).expect("valid range");
        assert!(range.contains(TextSize::new(3)));
        assert!(!range.contains(TextSize::new(7)));
        assert_eq!(range.len(), TextSize::new(4));
    }

    #[test]
    fn line_index_handles_crlf_unicode_tabs_and_eof() {
        let source: Arc<str> = Arc::from("a\r\n\tλ\n");
        let index = LineIndex::new(source).expect("small source");

        assert_eq!(index.line_count(), 3);
        assert_eq!(
            index.line_column(TextSize::new(6)),
            Some(LineColumn {
                line: 2,
                byte_column: 4,
                visual_column: 6,
            })
        );
        assert_eq!(
            index.line_column(TextSize::new(7)),
            Some(LineColumn {
                line: 3,
                byte_column: 1,
                visual_column: 1,
            })
        );
        assert!(index.line_column(TextSize::new(5)).is_none());
    }

    #[test]
    fn tab_width_uses_one_based_four_column_stops() {
        assert_eq!(visual_width("\t", 1), 5);
        assert_eq!(visual_width("\t ", 1), 6);
        assert_eq!(visual_width("abc\t", 1), 5);
    }

    #[test]
    fn unicode_width_matches_terminal_display_columns() {
        assert_eq!(visual_width("界", 1), 3);
        assert_eq!(visual_width("e\u{301}", 1), 2);
    }

    #[test]
    fn snapshots_are_content_addressed_and_project_relative() {
        let text: Arc<str> = Arc::from("int main(void);\n");
        let first = SourceSnapshot::new(
            FileId::new(7),
            Utf8PathBuf::from("src/main.c"),
            Arc::clone(&text),
        )
        .expect("valid snapshot");
        let second = SourceSnapshot::new(
            FileId::new(8),
            Utf8PathBuf::from("other.c"),
            Arc::clone(&text),
        )
        .expect("valid snapshot");

        assert_eq!(first.content_hash(), second.content_hash());
        assert_eq!(first.text().as_ref(), text.as_ref());
        assert!(matches!(
            SourceSnapshot::new(
                FileId::new(9),
                Utf8PathBuf::from("../escape.c"),
                Arc::from("")
            ),
            Err(SnapshotError::InvalidRelativePath(_))
        ));
    }

    #[test]
    fn diagnostics_have_a_total_stable_order() {
        let make = |path: &str, offset: u32, rule: &str| Diagnostic {
            rule_id: rule.to_owned(),
            path: Utf8PathBuf::from(path),
            range: TextRange::empty(TextSize::new(offset)),
            severity: Severity::Warning,
            message: "message".to_owned(),
            source: DiagnosticSource::Parser,
            notes: Vec::new(),
            help: None,
            localized: None,
        };
        let mut diagnostics = [
            make("b.c", 1, "z"),
            make("a.c", 8, "b"),
            make("a.c", 8, "a"),
            make("a.c", 2, "z"),
        ];
        diagnostics.sort();
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (
                    diagnostic.path.as_str(),
                    diagnostic.range.start().get(),
                    diagnostic.rule_id.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("a.c", 2, "z"),
                ("a.c", 8, "a"),
                ("a.c", 8, "b"),
                ("b.c", 1, "z"),
            ]
        );
    }

    #[test]
    fn source_edits_are_atomic_deduplicated_and_utf8_safe() {
        let edit = |start, end, replacement: &str, rule: &str| SourceEdit {
            range: TextRange::new(TextSize::new(start), TextSize::new(end)).expect("ordered range"),
            replacement: replacement.to_owned(),
            rule_id: rule.to_owned(),
            description: "test edit".to_owned(),
            applicability: Applicability::SafeLayout,
        };
        let source = "a λ c";
        let edits = [
            edit(0, 1, "A", "FIRST"),
            edit(5, 6, "C", "SECOND"),
            edit(0, 1, "A", "FIRST"),
        ];
        assert_eq!(
            apply_source_edits(source, &edits).expect("valid batch"),
            "A λ C"
        );

        let invalid = edit(3, 4, "x", "SPLIT_UTF8");
        assert!(matches!(
            apply_source_edits(source, &[invalid]),
            Err(EditError::InvalidRange { .. })
        ));

        let conflict = [edit(0, 2, "x", "RANGE"), edit(0, 0, "y", "INSERT")];
        assert!(matches!(
            apply_source_edits(source, &conflict),
            Err(EditError::Conflict { .. })
        ));
        assert_eq!(source, "a λ c");
    }
}
