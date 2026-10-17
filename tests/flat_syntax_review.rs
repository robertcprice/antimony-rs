//! Independent boundary review for ordinary flat headers and formula lists.
use antimony::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, SourceId, ValidatedModel, analyze,
    format_antimony, parse,
};

fn source(text: &str, limits: &Limits) -> SourceFile {
    SourceFile::new(SourceId::new(109), "review α; not a file", text, limits).unwrap()
}
fn roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let original = source(text, &limits);
    let parsed = parse(&original, &limits).unwrap();
    let model = analyze(&parsed).unwrap();
    let formatted = format_antimony(&parsed).unwrap();
    let output_source = source(&formatted, &limits);
    let output_parsed = parse(&output_source, &limits).unwrap();
    let after = analyze(&output_parsed).unwrap();
    assert_eq!(format_antimony(&output_parsed).unwrap(), formatted);
    assert_eq!(after.id(), model.id());
    assert_eq!(after.parameters().len(), model.parameters().len());
    assert_eq!(after.species().len(), model.species().len());
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&model).unwrap().xml(),
        antimony::write_sbml(&after).unwrap().xml(),
        "source: {text}\nformatted: {formatted}"
    );
    (model, formatted)
}
fn parse_error(text: &str, expected: DiagnosticCode) -> Diagnostic {
    let limits = Limits::default();
    let input = source(text, &limits);
    let error = parse(&input, &limits).unwrap_err();
    assert_eq!(error.code(), expected, "source: {text}");
    assert!(!error.labels().is_empty(), "source: {text}");
    for label in error.labels() {
        assert_eq!(label.span().source_id(), input.id());
        input.slice(label.span()).unwrap();
    }
    error
}

#[test]
fn optional_parentheses_preserve_header_spelling_not_just_model_identity() {
    let (bare, output) = roundtrip("model header;formula x=1,y=x+2;end");
    assert_eq!(output, "model header\n  formula x = 1, y = x + 2;\nend\n");
    let (empty, output) = roundtrip("model header();formula x=1,y=x+2;end");
    assert!(output.starts_with("model header()\n"));
    assert_eq!(bare.id(), empty.id());
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&bare).unwrap().xml(),
        antimony::write_sbml(&empty).unwrap().xml()
    );
}
#[test]
fn header_comments_unicode_and_separator_runs_cannot_become_body_tokens() {
    for header in [
        "model/*λ*/m\r\n",
        "model m;;;\n",
        "model m/*comment\nignored*/;",
        "model m(/*💡*/)\n",
    ] {
        let (model, output) = roundtrip(&format!("{header}formula x=1,y=2;end"));
        assert_eq!(model.id(), "m");
        assert_eq!(model.parameters().len(), 2);
        assert_eq!(output.matches("formula").count(), 1);
        assert!(!output.contains("comment"));
    }
}
#[test]
fn list_scalar_plus_parentheses_and_arithmetic_modes_are_preserved() {
    let (model, output) = roundtrip("formula a=-2,b=+2,c=(2),d=a+4,e; e=5;");
    assert_eq!(
        output,
        "formula a = -2, b = +2, c = (2), d = a + 4, e;\ne = 5;\n"
    );
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|a| a.symbol())
            .collect::<Vec<_>>(),
        ["b", "c", "d"]
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value().expect("explicit initialized test model")))
            .collect::<Vec<_>>(),
        [("a", -2.0), ("b", 2.0), ("c", 2.0), ("d", 2.0), ("e", 5.0)]
    );
}
#[test]
fn forward_list_dependencies_keep_original_assignment_appearance_order() {
    let (model, output) = roundtrip("formula z=b+1,a=2,b=a+3;");
    assert_eq!(output, "formula z = b + 1, a = 2, b = a + 3;\n");
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|a| a.symbol())
            .collect::<Vec<_>>(),
        ["z", "b"]
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "z")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        6.0
    );
}
#[test]
fn mixed_uninitialized_entries_resolve_later_without_inventing_values() {
    let (model, output) = roundtrip("formula a,b=2,c; a=3;c=4;");
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| p.value().expect("explicit initialized test model"))
            .collect::<Vec<_>>(),
        [3.0, 2.0, 4.0]
    );
    assert!(output.starts_with("formula a, b = 2, c;\n"));
    let limits = Limits::default();
    let input = source("formula a=1,b;", &limits);
    let parsed = parse(&input, &limits).unwrap();
    assert_eq!(analyze(&parsed).unwrap().parameters()[1].value(), None);
    assert!(format_antimony(&parsed).is_ok());
}
#[test]
fn list_endings_eof_newline_and_named_end_roundtrip() {
    for text in [
        "formula a=1,b=2",
        "formula a=1,b=2\n",
        "model m\nformula a=1,b=2\nend",
        "model m();formula a=1,b=2 end",
    ] {
        assert_eq!(roundtrip(text).0.parameters().len(), 2);
    }
}
#[test]
fn truncated_or_cross_line_lists_are_syntax_errors_not_partial_models() {
    for text in [
        "formula",
        "formula ;",
        "formula ,a=1;",
        "formula a=1,",
        "formula a=1,;",
        "formula a=1,,b=2;",
        "formula a=1,\nb=2;",
        "formula a=1, // next\nb=2;",
        "formula a=1,b=;",
        "model m\nformula a=1,\nend",
    ] {
        parse_error(text, DiagnosticCode::Syntax);
    }
}
#[test]
fn bare_unknown_is_malformed_but_known_unsupported_constructs_remain_distinct() {
    for text in [
        "unknown;",
        "unknown",
        "unknown\n",
        "model m\nunknown\nend",
        "formula a=1,b=2; unknown;",
    ] {
        parse_error(text, DiagnosticCode::Syntax);
    }
    for text in ["a:=1;", "a'=1;"] {
        assert_eq!(roundtrip(text).0.rules().len(), 1);
    }
    for text in [
        "a:=;",
        "a'=;",
        "S in cell;",
        "a is \"label\";",
        "import \"secret\";",
        "a=delay(2,1);",
    ] {
        parse_error(text, DiagnosticCode::UnsupportedFeature);
    }
}
#[test]
fn parameter_is_not_invented_as_a_declaration_keyword() {
    // Pinned upstream rejects this spelling; an ordinary identifier named
    // parameter remains legal when used in actual initialization syntax.
    let limits = Limits::default();
    assert!(parse(&source("parameter a=1,b=2;", &limits), &limits).is_err());
    let (model, output) = roundtrip("parameter=1;formula b=parameter+1;");
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "b")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        2.0
    );
    assert!(output.starts_with("parameter = 1;\n"));
}
#[test]
fn nonempty_ports_are_unsupported_and_malformed_headers_are_syntax_errors() {
    for text in ["model m(a)\na=1;end", "model m(a,$b)\na=1;b=2;end"] {
        parse_error(text, DiagnosticCode::UnsupportedFeature);
    }
    for text in [
        "model m(\na=1;end",
        "model m(1)\na=1;end",
        "model m(a,)\na=1;end",
        "model m();a=1;",
        "model\na=1;end",
    ] {
        parse_error(text, DiagnosticCode::Syntax);
    }
}
#[test]
fn lists_promote_species_and_retain_dependency_constant_inference() {
    let limits = Limits::default();
    let input = source("formula a=1,b=2;species b;", &limits);
    let parsed = parse(&input, &limits).unwrap();
    assert_eq!(analyze(&parsed).unwrap().species()[0].id(), "b");
    for boundary in [false, true] {
        let prefix = if boundary { "const " } else { "" };
        let (model, _) = roundtrip(&format!("{prefix}species S=2;formula b=S+2,z=b+1;"));
        assert!(model.parameters().iter().all(|p| p.constant() == boundary));
    }
}
#[test]
fn grouped_declarations_keep_entity_and_generated_output_budgets() {
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 1);
    let input = source("formula a=1,b=2;", &limits);
    let parsed = parse(&input, &limits).unwrap();
    assert_eq!(
        analyze(&parsed).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    assert_eq!(
        format_antimony(&parsed).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::Tokens, 14);
    let input = source("model m;formula a=1,b=2;end", &limits);
    let parsed = parse(&input, &limits).unwrap();
    let error = format_antimony(&parsed).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    for label in error.labels() {
        input.slice(label.span()).unwrap();
    }
    let text = "model m;formula a=1,b=2;end";
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, text.len());
    let parsed = parse(&source(text, &limits), &limits).unwrap();
    assert_eq!(
        format_antimony(&parsed).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}
#[test]
fn long_lists_and_tail_errors_do_not_truncate_or_recurse_by_entry() {
    let entries = (0..500)
        .map(|n| format!("p{n}={n}"))
        .collect::<Vec<_>>()
        .join(",");
    let (model, output) = roundtrip(&format!("formula {entries};"));
    assert_eq!(model.parameters().len(), 500);
    assert_eq!(output.matches(',').count(), 499);
    parse_error(&format!("formula {entries},;"), DiagnosticCode::Syntax);
}
