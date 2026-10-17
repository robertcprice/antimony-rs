//! Configurable, inclusive resource ceilings and checked arithmetic.
use std::{error::Error, fmt};

/// Resource measured by an inclusive ceiling. Units are explicit in each variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum LimitKind {
    /// UTF-8 bytes in a single source file.
    SourceBytes,
    /// Tokens produced for one compilation.
    Tokens,
    /// Simultaneously nested expression levels (not cumulative nodes).
    ExpressionDepth,
    /// Model entities in one compilation.
    ModelEntities,
    /// Accumulated diagnostics in one report.
    Diagnostics,
}
impl LimitKind {
    const fn index(self) -> usize {
        match self {
            Self::SourceBytes => 0,
            Self::Tokens => 1,
            Self::ExpressionDepth => 2,
            Self::ModelEntities => 3,
            Self::Diagnostics => 4,
        }
    }
}

/// Configurable resource ceilings. Zero is valid and rejects any positive usage.
///
/// These are safety policy values, not a performance or memory-allocation guarantee.
/// Consumers must call the guards before growing their own parser/model storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Limits {
    ceilings: [usize; 5],
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            ceilings: [4 * 1024 * 1024, 1_000_000, 128, 100_000, 100],
        }
    }
}
impl Limits {
    /// Replace one ceiling; all other limits remain unchanged.
    #[must_use]
    pub fn with_limit(mut self, kind: LimitKind, maximum: usize) -> Self {
        self.ceilings[kind.index()] = maximum;
        self
    }
    /// Return the inclusive ceiling for a resource.
    pub fn limit(&self, kind: LimitKind) -> usize {
        self.ceilings[kind.index()]
    }
    /// Check an already-computed usage without modifying state.
    pub fn check(&self, kind: LimitKind, attempted: usize) -> Result<(), LimitExceeded> {
        let maximum = self.limit(kind);
        if attempted > maximum {
            Err(LimitExceeded {
                kind,
                maximum,
                attempted: Some(attempted),
            })
        } else {
            Ok(())
        }
    }
    /// Check a prospective increment, rejecting integer overflow even with a
    /// `usize::MAX` ceiling. Only commit the returned total after success.
    pub fn checked_add(
        &self,
        kind: LimitKind,
        current: usize,
        additional: usize,
    ) -> Result<usize, LimitExceeded> {
        let attempted = current.checked_add(additional).ok_or(LimitExceeded {
            kind,
            maximum: self.limit(kind),
            attempted: None,
        })?;
        self.check(kind, attempted)?;
        Ok(attempted)
    }
}

/// A resource ceiling was exceeded, or the proposed count overflowed `usize`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitExceeded {
    kind: LimitKind,
    maximum: usize,
    attempted: Option<usize>,
}
impl LimitExceeded {
    /// The resource whose budget was exceeded.
    pub fn kind(&self) -> LimitKind {
        self.kind
    }
    /// Its configured inclusive ceiling.
    pub fn maximum(&self) -> usize {
        self.maximum
    }
    /// Prospective usage, or `None` if addition overflowed `usize`.
    pub fn attempted(&self) -> Option<usize> {
        self.attempted
    }
}
impl fmt::Display for LimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.attempted {
            Some(n) => write!(
                f,
                "{:?} limit exceeded: attempted {n}, maximum {}",
                self.kind, self.maximum
            ),
            None => write!(
                f,
                "{:?} count overflowed (maximum {})",
                self.kind, self.maximum
            ),
        }
    }
}
impl Error for LimitExceeded {}
