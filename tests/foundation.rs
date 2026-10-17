//! Native foundation invariants and boundary-condition regression tests.
use antimony::{
    Diagnostic, DiagnosticCode, Diagnostics, Label, LabelStyle, LimitKind, Limits, Severity,
    SourceFile, SourceId, SpanError,
};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId::new(7), "test", text, &Limits::default()).unwrap()
}

fn diagnostic(severity: Severity) -> Diagnostic {
    Diagnostic::new(severity, DiagnosticCode::Syntax, "expected a value")
}

#[test]
fn owns_source_without_interpreting_the_name() {
    let mut text = String::from("A = 1;");
    let file = SourceFile::new(
        SourceId::new(42),
        "$(touch /not-a-command)",
        &text,
        &Limits::default(),
    )
    .unwrap();
    text.clear();
    assert_eq!(file.text(), "A = 1;");
    assert_eq!(file.name(), "$(touch /not-a-command)");
    assert_eq!(file.id().get(), 42);
}

#[test]
fn spans_are_half_open_and_count_bytes() {
    let file = source("aβ😀");
    let span = file.span(1, 7).unwrap();
    assert_eq!(span.source_id(), file.id());
    assert_eq!(span.start(), 1);
    assert_eq!(span.end(), 7);
    assert_eq!(span.range(), 1..7);
    assert_eq!(span.len(), 6);
    assert!(!span.is_empty());
    assert_eq!(file.slice(span).unwrap(), "β😀");
}

#[test]
fn empty_and_eof_spans_are_valid() {
    for text in ["", "abc", "😀"] {
        let file = source(text);
        for offset in [0, text.len()] {
            let span = file.span(offset, offset).unwrap();
            assert!(span.is_empty());
            assert_eq!(span.len(), 0);
            assert_eq!(file.slice(span).unwrap(), "");
        }
    }
}

#[test]
fn reversed_spans_are_rejected_without_subtraction_overflow() {
    assert_eq!(
        source("abc").span(usize::MAX, 0),
        Err(SpanError::Reversed {
            start: usize::MAX,
            end: 0
        })
    );
}

#[test]
fn oversized_offsets_are_rejected() {
    let file = source("abc");
    for offset in [4, usize::MAX] {
        assert_eq!(
            file.span(0, offset),
            Err(SpanError::OutOfBounds {
                offset,
                source_len: 3
            })
        );
        assert_eq!(
            file.location(offset),
            Err(SpanError::OutOfBounds {
                offset,
                source_len: 3
            })
        );
    }
}

#[test]
fn utf8_interior_offsets_are_rejected_at_either_endpoint() {
    let file = source("β😀");
    for offset in [1, 3, 4, 5] {
        let error = SpanError::NotCharBoundary { offset };
        assert_eq!(file.span(offset, 6), Err(error));
        assert_eq!(file.span(0, offset), Err(error));
        assert_eq!(file.location(offset), Err(error));
    }
}

#[test]
fn slicing_rejects_other_source_id() {
    let first = source("abc");
    let second = SourceFile::new(SourceId::new(8), "second", "abc", &Limits::default()).unwrap();
    let span = first.span(0, 3).unwrap();
    assert_eq!(
        second.slice(span),
        Err(SpanError::WrongSource {
            expected: second.id(),
            actual: first.id()
        })
    );
}

#[test]
fn slicing_defensively_rechecks_even_reused_source_ids() {
    // Reusing IDs violates the caller's identity contract, but must not panic.
    let first = source("abc");
    let shorter = source("a");
    assert!(shorter.slice(first.span(0, 3).unwrap()).is_err());
    let different_boundaries = source("βa");
    assert!(
        different_boundaries
            .slice(first.span(1, 3).unwrap())
            .is_err()
    );
}

#[test]
fn positions_distinguish_scalar_and_byte_columns() {
    let file = source("aβ😀\tz");
    for (offset, scalar, byte) in [
        (0, 1, 1),
        (1, 2, 2),
        (3, 3, 4),
        (7, 4, 8),
        (8, 5, 9),
        (9, 6, 10),
    ] {
        let location = file.location(offset).unwrap();
        assert_eq!(
            (location.line(), location.column(), location.byte_column()),
            (1, scalar, byte)
        );
    }
}

#[test]
fn line_mapping_handles_lf_crlf_and_lone_cr() {
    let file = source("a\r\nb\rc\n");
    assert_eq!(file.line_count(), 4);
    for (offset, line, column) in [
        (0, 1, 1),
        (1, 1, 2),
        (2, 1, 3),
        (3, 2, 1),
        (4, 2, 2),
        (5, 3, 1),
        (6, 3, 2),
        (7, 4, 1),
    ] {
        let location = file.location(offset).unwrap();
        assert_eq!((location.line(), location.column()), (line, column));
    }
}

#[test]
fn empty_source_has_one_line_and_consecutive_newlines_have_empty_lines() {
    let file = source("");
    assert_eq!(file.line_count(), 1);
    assert_eq!(file.location(0).unwrap().line(), 1);
    let file = source("\n\n");
    assert_eq!(file.line_count(), 3);
    for offset in 0..=2 {
        let location = file.location(offset).unwrap();
        assert_eq!((location.line(), location.column()), (offset + 1, 1));
    }
}

#[test]
fn source_limit_counts_utf8_bytes_and_is_inclusive() {
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, 2);
    assert!(SourceFile::new(SourceId::new(0), "", "β", &limits).is_ok());
    let error = SourceFile::new(SourceId::new(0), "", "βa", &limits).unwrap_err();
    assert_eq!(error.kind(), LimitKind::SourceBytes);
    assert_eq!(error.maximum(), 2);
    assert_eq!(error.attempted(), Some(3));
    let limits = limits.with_limit(LimitKind::SourceBytes, 0);
    assert!(SourceFile::new(SourceId::new(0), "", "", &limits).is_ok());
    assert!(SourceFile::new(SourceId::new(0), "", "a", &limits).is_err());
}

#[test]
fn limits_match_documented_defaults() {
    let limits = Limits::default();
    for (kind, expected) in [
        (LimitKind::SourceBytes, 4 * 1024 * 1024),
        (LimitKind::Tokens, 1_000_000),
        (LimitKind::ExpressionDepth, 128),
        (LimitKind::ModelEntities, 100_000),
        (LimitKind::Diagnostics, 100),
    ] {
        assert_eq!(limits.limit(kind), expected);
        assert!(limits.check(kind, expected).is_ok());
        assert!(limits.check(kind, expected + 1).is_err());
    }
}

#[test]
fn limits_are_independent_and_zero_is_respected() {
    for kind in [
        LimitKind::SourceBytes,
        LimitKind::Tokens,
        LimitKind::ExpressionDepth,
        LimitKind::ModelEntities,
        LimitKind::Diagnostics,
    ] {
        let limits = Limits::default().with_limit(kind, 0);
        assert!(limits.check(kind, 0).is_ok());
        assert!(limits.check(kind, 1).is_err());
        assert_eq!(limits.checked_add(kind, 0, 0).unwrap(), 0);
        assert!(limits.checked_add(kind, 0, 1).is_err());
    }
    let defaults = Limits::default();
    let changed = defaults.clone().with_limit(LimitKind::Tokens, 3);
    assert_eq!(
        changed.limit(LimitKind::SourceBytes),
        defaults.limit(LimitKind::SourceBytes)
    );
    assert_eq!(defaults.limit(LimitKind::Tokens), 1_000_000);
}

#[test]
fn checked_add_rejects_overflow_and_existing_over_budget_counts() {
    let kind = LimitKind::Tokens;
    let limits = Limits::default().with_limit(kind, usize::MAX);
    assert_eq!(
        limits.checked_add(kind, usize::MAX - 1, 1).unwrap(),
        usize::MAX
    );
    let error = limits.checked_add(kind, usize::MAX, 1).unwrap_err();
    assert_eq!(error.attempted(), None);
    assert_eq!(error.maximum(), usize::MAX);
    assert_eq!(error.kind(), kind);
    assert!(error.to_string().contains("overflowed"));
    let limits = limits.with_limit(kind, 2);
    assert!(limits.checked_add(kind, 3, 0).is_err());
    let error = limits.checked_add(kind, 2, 1).unwrap_err();
    assert_eq!(error.attempted(), Some(3));
    assert!(error.to_string().contains("attempted 3"));
}

#[test]
fn diagnostic_keeps_labels_notes_and_machine_code() {
    let file = source("x x");
    let report = Diagnostic::new(
        Severity::Error,
        DiagnosticCode::DeclarationConflict,
        "duplicate declaration",
    )
    .with_label(Label::primary(file.span(2, 3).unwrap(), "second"))
    .with_label(Label::secondary(file.span(0, 1).unwrap(), "first"))
    .with_note("rename one declaration");
    assert_eq!(report.severity(), Severity::Error);
    assert_eq!(report.code().as_str(), "declaration_conflict");
    assert_eq!(report.message(), "duplicate declaration");
    assert_eq!(report.labels().len(), 2);
    assert_eq!(report.labels()[0].style(), LabelStyle::Primary);
    assert_eq!(report.labels()[1].style(), LabelStyle::Secondary);
    assert_eq!(report.labels()[0].message(), "second");
    assert_eq!(file.slice(report.labels()[1].span()).unwrap(), "x");
    assert_eq!(report.notes(), &["rename one declaration"]);
}

#[test]
fn diagnostic_codes_are_distinct_and_stable() {
    let pairs = [
        (DiagnosticCode::Syntax, "syntax"),
        (DiagnosticCode::UnresolvedSymbol, "unresolved_symbol"),
        (DiagnosticCode::DeclarationConflict, "declaration_conflict"),
        (DiagnosticCode::UnsupportedFeature, "unsupported_feature"),
        (DiagnosticCode::UnitsInconsistency, "units_inconsistency"),
        (DiagnosticCode::InitializationCycle, "initialization_cycle"),
        (DiagnosticCode::ResourceLimit, "resource_limit"),
        (DiagnosticCode::Export, "export"),
    ];
    for (code, expected) in pairs {
        assert_eq!(code.as_str(), expected);
    }
}

#[test]
fn diagnostic_collection_rejects_excess_without_mutation() {
    let limits = Limits::default().with_limit(LimitKind::Diagnostics, 2);
    let mut report = Diagnostics::new(&limits);
    assert!(report.is_empty());
    report.push(diagnostic(Severity::Warning)).unwrap();
    report.push(diagnostic(Severity::Note)).unwrap();
    assert!(!report.has_errors());
    let error = report.push(diagnostic(Severity::Error)).unwrap_err();
    assert_eq!(error.kind(), LimitKind::Diagnostics);
    assert_eq!(error.maximum(), 2);
    assert_eq!(error.attempted(), Some(3));
    assert_eq!(report.len(), 2);
    assert!(!report.has_errors());
    assert_eq!(
        report.iter().map(|d| d.severity()).collect::<Vec<_>>(),
        vec![Severity::Warning, Severity::Note]
    );
}

#[test]
fn zero_diagnostic_budget_is_explicit_not_a_silent_drop() {
    let limits = Limits::default().with_limit(LimitKind::Diagnostics, 0);
    let mut report = Diagnostics::new(&limits);
    assert!(report.push(diagnostic(Severity::Error)).is_err());
    assert!(report.is_empty());
}

#[test]
fn accepted_error_marks_report_and_clones_have_independent_storage() {
    let mut report = Diagnostics::new(&Limits::default());
    report.push(diagnostic(Severity::Warning)).unwrap();
    let mut copy = report.clone();
    copy.push(diagnostic(Severity::Error)).unwrap();
    assert!(copy.has_errors());
    assert!(!report.has_errors());
    assert_eq!(report.len(), 1);
    assert_eq!(copy.len(), 2);
}

#[test]
fn exhaustive_small_ranges_agree_with_safe_string_slicing() {
    for text in ["", "ASCII", "β😀e\u{301}\r\n終\r"] {
        let file = source(text);
        for start in 0..=text.len() + 1 {
            for end in 0..=text.len() + 1 {
                let expected = text.get(start..end);
                let actual = file
                    .span(start, end)
                    .ok()
                    .and_then(|span| file.slice(span).ok());
                assert_eq!(actual, expected, "{text:?} at {start}..{end}");
            }
        }
    }
}

#[test]
fn foundation_types_are_send_sync_and_thread_owned() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SourceFile>();
    send_sync::<antimony::Span>();
    send_sync::<Diagnostics>();
    send_sync::<Limits>();
    let handles = (0..4)
        .map(|id| {
            std::thread::spawn(move || {
                let file =
                    SourceFile::new(SourceId::new(id), "thread", "β = 1;", &Limits::default())
                        .unwrap();
                assert_eq!(file.slice(file.span(0, 2).unwrap()).unwrap(), "β");
                assert_eq!(file.location(2).unwrap().column(), 2);
                file.id().get()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}
