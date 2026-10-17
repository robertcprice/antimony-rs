//! Owned UTF-8 sources and validated, half-open byte ranges.
use crate::{LimitExceeded, LimitKind, Limits};
use std::{error::Error, fmt, ops::Range};

/// Caller-assigned source identity, unique within one compilation/report.
///
/// IDs are not global. Do not combine unrelated reports without remapping IDs,
/// or reuse an ID for changed text while its old spans remain in use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceId(u32);
impl SourceId {
    /// Assign an ID; the caller owns uniqueness within its compilation.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Read the caller-assigned numeric value.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Half-open UTF-8 byte range, constructed only through [`SourceFile::span`].
/// An empty span is valid, including at end-of-file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Span {
    source_id: SourceId,
    start: usize,
    end: usize,
}
impl Span {
    /// Source identity assigned by the caller.
    pub fn source_id(self) -> SourceId {
        self.source_id
    }
    /// Inclusive start byte offset.
    pub fn start(self) -> usize {
        self.start
    }
    /// Exclusive end byte offset.
    pub fn end(self) -> usize {
        self.end
    }
    /// Byte range suitable for slicing after verifying source identity.
    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
    /// Number of bytes, not Unicode characters.
    pub fn len(self) -> usize {
        self.end - self.start
    }
    /// Whether the span is empty.
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Immutable owned source with a precomputed line-start index.
#[derive(Clone, Debug)]
pub struct SourceFile {
    id: SourceId,
    name: String,
    text: String,
    line_starts: Vec<usize>,
}
impl SourceFile {
    /// Copy UTF-8 text after checking its byte budget. The name is a display
    /// label only: it never triggers filesystem or network access.
    ///
    /// LF, CRLF and lone CR are line endings; CRLF counts as one newline.
    pub fn new(
        id: SourceId,
        name: impl Into<String>,
        text: &str,
        limits: &Limits,
    ) -> Result<Self, LimitExceeded> {
        limits.check(LimitKind::SourceBytes, text.len())?;
        let bytes = text.as_bytes();
        let mut line_starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' => {
                    i += 1;
                    if bytes.get(i) == Some(&b'\n') {
                        i += 1;
                    }
                    line_starts.push(i);
                }
                b'\n' => {
                    i += 1;
                    line_starts.push(i);
                }
                _ => i += 1,
            }
        }
        Ok(Self {
            id,
            name: name.into(),
            text: text.to_owned(),
            line_starts,
        })
    }
    /// Caller-assigned identity.
    pub fn id(&self) -> SourceId {
        self.id
    }
    /// Display-only name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Original immutable UTF-8 source text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Number of lines, including an empty final line after a trailing newline.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
    /// Validate a half-open byte range against this source.
    pub fn span(&self, start: usize, end: usize) -> Result<Span, SpanError> {
        if start > end {
            return Err(SpanError::Reversed { start, end });
        }
        self.check_offset(start)?;
        self.check_offset(end)?;
        Ok(Span {
            source_id: self.id,
            start,
            end,
        })
    }
    /// Safely slice a span, rejecting another source ID and rechecking bounds.
    pub fn slice(&self, span: Span) -> Result<&str, SpanError> {
        if span.source_id != self.id {
            return Err(SpanError::WrongSource {
                expected: self.id,
                actual: span.source_id,
            });
        }
        self.span(span.start, span.end)?;
        Ok(&self.text[span.range()])
    }
    /// Locate a UTF-8 boundary, including EOF. Lines and both columns are
    /// one-based. Character columns count Unicode scalar values, not graphemes,
    /// tab expansion or terminal cells. Newline bytes belong to the prior line.
    pub fn location(&self, byte_offset: usize) -> Result<SourceLocation, SpanError> {
        self.check_offset(byte_offset)?;
        let line_index = self
            .line_starts
            .partition_point(|&start| start <= byte_offset)
            - 1;
        let line_start = self.line_starts[line_index];
        Ok(SourceLocation {
            line: line_index + 1,
            column: self.text[line_start..byte_offset].chars().count() + 1,
            byte_column: byte_offset - line_start + 1,
        })
    }
    fn check_offset(&self, offset: usize) -> Result<(), SpanError> {
        if offset > self.text.len() {
            Err(SpanError::OutOfBounds {
                offset,
                source_len: self.text.len(),
            })
        } else if !self.text.is_char_boundary(offset) {
            Err(SpanError::NotCharBoundary { offset })
        } else {
            Ok(())
        }
    }
}

/// One-based human-facing position for a validated byte offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    line: usize,
    column: usize,
    byte_column: usize,
}
impl SourceLocation {
    /// One-based line number.
    pub fn line(self) -> usize {
        self.line
    }
    /// One-based Unicode scalar column.
    pub fn column(self) -> usize {
        self.column
    }
    /// One-based UTF-8 byte column.
    pub fn byte_column(self) -> usize {
        self.byte_column
    }
}

/// A source offset/range cannot be used safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SpanError {
    /// Range starts after its end.
    Reversed {
        /// Requested start.
        start: usize,
        /// Requested end.
        end: usize,
    },
    /// Byte offset exceeds EOF.
    OutOfBounds {
        /// Requested offset.
        offset: usize,
        /// Source byte length.
        source_len: usize,
    },
    /// Offset divides a UTF-8 code point.
    NotCharBoundary {
        /// Requested offset.
        offset: usize,
    },
    /// Span belongs to a different caller-assigned source ID.
    WrongSource {
        /// Receiving source ID.
        expected: SourceId,
        /// Span source ID.
        actual: SourceId,
    },
}
impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reversed { start, end } => write!(f, "span start {start} exceeds end {end}"),
            Self::OutOfBounds { offset, source_len } => {
                write!(f, "offset {offset} exceeds source length {source_len}")
            }
            Self::NotCharBoundary { offset } => {
                write!(f, "offset {offset} splits a UTF-8 character")
            }
            Self::WrongSource { expected, actual } => write!(
                f,
                "span source {} does not match source {}",
                actual.get(),
                expected.get()
            ),
        }
    }
}
impl Error for SpanError {}
