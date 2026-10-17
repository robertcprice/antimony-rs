//! Independently authored Phase 1 regressions; these are not corpus-ID checks.
use antimony::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, SourceId, ValidatedModel, analyze,
    format_antimony, parse,
};

fn compile(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source =
        SourceFile::new(SourceId::new(914), "independent arithmetic", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}

fn value(text: &str) -> f64 {
    compile(text, &Limits::default()).unwrap().parameters()[0]
        .value()
        .expect("explicit initialized test model")
}

#[test]
fn cancellation_uses_documented_reference_normalized_order() {
    // Reference flattens associative groups. This is intentionally not the
    // IEEE result of evaluating the source's right-hand parentheses first.
    assert_eq!(value("answer=1e16+(-1e16+1);"), 1.0);
    assert_eq!(value("answer=(1e16+-1e16)+1;"), 1.0);
}

#[test]
fn enclosing_same_operator_preserves_reference_inner_group() {
    // Upstream's grouping is contextual: an enclosing plus/multiply appends to
    // the original left group without first normalizing that group's children.
    assert_eq!(value("answer=(1e16+(-1e16+1))+1;"), 1.0);
    assert_eq!(value("answer=(1e200*(1e-200*1e-200))*1;"), 0.0);
}

#[test]
fn signed_zero_initializers_follow_exported_literal_meaning() {
    for expression in ["(-0)", "0*-0", "-(-0)", "-(0*2)"] {
        assert_eq!(
            value(&format!("answer={expression};")).to_bits(),
            0.0f64.to_bits()
        );
    }
}

#[test]
fn long_exponents_are_bounded_inputs_not_internal_panics() {
    assert_eq!(value(&format!("answer=(0e{});", "9".repeat(2000))), 0.0);
    for sign in ["", "-"] {
        let text = format!("answer=(1e{sign}{});", "9".repeat(2000));
        assert!(compile(&text, &Limits::default()).is_err());
    }
    let text = format!("answer=(1e{}1);", "0".repeat(2000));
    assert_eq!(value(&text), 10.0);
}

#[test]
fn normalized_intermediate_overflow_does_not_hide_behind_finite_source_group() {
    let error = compile("answer=1e308+(1e308+-1e308);", &Limits::default()).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::DeclarationConflict);
    assert!(!error.labels().is_empty());
}

#[test]
fn product_evaluation_does_not_underflow_the_right_group_first() {
    let actual = value("answer=1e200*(1e-200*1e-200);");
    assert!(actual > 0.0);
    assert!((actual / 1e-200 - 1.0).abs() < 1e-14);
}

#[test]
fn signs_and_powers_preserve_non_associative_boundaries() {
    for (expression, expected) in [
        ("2^-3^2", 1.0 / 512.0),
        ("(-2)^2", 4.0),
        ("-2^2", -4.0),
        ("12/(3/2)", 8.0),
        ("12/3/2", 2.0),
        ("12-(3-2)", 11.0),
        ("12-3-2", 7.0),
        ("+-2", -2.0),
        ("-+2", -2.0),
        ("++2", 2.0),
        ("- -2", 2.0),
        ("-/* comment */-2", 2.0),
        ("2-/* comment */-1", 3.0),
        ("-(-(-2))", -2.0),
    ] {
        assert_eq!(
            value(&format!("answer={expression};")),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn malformed_adjacent_decrement_does_not_become_two_unary_operators() {
    for text in ["answer=--2;", "answer=2--1;", "answer=- // comment\n-2;"] {
        assert!(compile(text, &Limits::default()).is_err(), "{text}");
    }
}

#[test]
fn accepted_literal_extremes_do_not_round_nonzero_inputs_to_zero() {
    for literal in ["1e-307", "1e-100", "1e-9", "1e15", "1e100", "1e308"] {
        assert_eq!(
            value(&format!("answer=({literal});")),
            literal.parse::<f64>().unwrap()
        );
    }
    for literal in ["1e-999", "1e309", "1.1234567890123456", "1e-308"] {
        assert!(compile(&format!("answer=({literal});"), &Limits::default()).is_err());
    }
}

#[test]
fn normalization_cannot_bypass_the_expression_depth_budget() {
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 3);
    let text = "answer=((1+2)+(3+4))+((5+6)+(7+8));";
    let error = compile(text, &limits).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    for label in error.labels() {
        assert!(text.get(label.span().range()).is_some());
    }
}

#[test]
fn comment_stripping_cannot_fuse_separate_negative_signs() {
    let limits = Limits::default();
    for text in ["a=- -2;", "a=-/* comment */-2;", "a=2-/* comment */-1;"] {
        let source = SourceFile::new(SourceId::new(914), "signs", text, &limits).unwrap();
        let formatted = format_antimony(&parse(&source, &limits).unwrap()).unwrap();
        assert!(!formatted.contains("--"));
        assert_eq!(value(text), value(&formatted));
        let source = SourceFile::new(SourceId::new(914), "signs", &formatted, &limits).unwrap();
        assert_eq!(
            format_antimony(&parse(&source, &limits).unwrap()).unwrap(),
            formatted
        );
    }
}

#[test]
fn deeply_nested_signs_cannot_bypass_limits_by_simplification() {
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 8);
    let text = format!("answer={}2{};", "-(".repeat(20), ")".repeat(20));
    assert_eq!(
        compile(&text, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn arbitrary_source_names_and_layout_retain_semantics() {
    let limits = Limits::default();
    let text = "model independent_lab()\n formula Zeta=2, tiny=1e-9;species Rho,Tau;Rho=3;Tau=0;\n turnover:Rho=>Tau;tiny*(Zeta*(Rho+(-(-Tau))));\nend";
    let source = SourceFile::new(SourceId::new(914), "unseen model", text, &limits).unwrap();
    let parsed = parse(&source, &limits).unwrap();
    let original = analyze(&parsed).unwrap();
    let formatted = format_antimony(&parsed).unwrap();
    assert!(formatted.contains("1e-9"));
    let source = SourceFile::new(SourceId::new(914), "formatted", &formatted, &limits).unwrap();
    let parsed = parse(&source, &limits).unwrap();
    assert_eq!(format_antimony(&parsed).unwrap(), formatted);
    let roundtrip = analyze(&parsed).unwrap();
    assert_eq!(
        original.parameters()[1]
            .value()
            .expect("explicit initialized test model"),
        roundtrip.parameters()[1]
            .value()
            .expect("explicit initialized test model")
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&original).unwrap().xml(),
        antimony::write_sbml(&roundtrip).unwrap().xml()
    );
}
