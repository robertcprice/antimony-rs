//! Reference-probed flat declarations; not broader module or qualifier support.
use antimony::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, ParsedDocument, SourceFile, SourceId,
    ValidatedModel, analyze, format_antimony, parse,
};

fn parsed(text: &str, limits: &Limits) -> Result<ParsedDocument, Diagnostic> {
    let source = SourceFile::new(SourceId::new(92), "flat syntax", text, &Limits::default())
        .expect("bounded fixture");
    parse(&source, limits)
}
fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    analyze(&parsed(text, &Limits::default())?)
}
fn roundtrip(text: &str) -> String {
    let limits = Limits::default();
    let document = parsed(text, &limits).unwrap();
    let original = analyze(&document).unwrap();
    let output = format_antimony(&document).unwrap();
    let formatted = parsed(&output, &limits).unwrap();
    let again = analyze(&formatted).unwrap();
    assert_eq!(format_antimony(&formatted).unwrap(), output);
    assert_eq!(original.id(), again.id());
    assert_eq!(original.parameters().len(), again.parameters().len());
    assert_eq!(
        original.initial_assignments().len(),
        again.initial_assignments().len()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&original).unwrap().xml(),
        antimony::write_sbml(&again).unwrap().xml()
    );
    output
}

#[test]
fn optional_empty_header_parentheses_preserve_source_form() {
    for (source, expected) in [
        ("model m\na=1;end", "model m\n  a = 1;\nend\n"),
        ("model m;a=1;end", "model m\n  a = 1;\nend\n"),
        ("model m ( );a=1;end", "model m()\n  a = 1;\nend\n"),
        ("model m/*β*/\r\na=1;end", "model m\n  a = 1;\nend\n"),
    ] {
        assert_eq!(roundtrip(source), expected);
        assert_eq!(compile(source).unwrap().id(), "m");
    }
}

#[test]
fn formula_list_preserves_scalar_lexemes_and_negative_values() {
    let source = "formula a=0001.,b=-2,c=0e-999;";
    assert_eq!(
        roundtrip(source),
        "formula a = 0001., b = -2, c = 0e-999;\n"
    );
    let model = compile(source).unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| p.value().expect("explicit initialized test model"))
            .collect::<Vec<_>>(),
        [1.0, -2.0, 0.0]
    );
    assert!(model.parameters().iter().all(|p| p.constant()));
    assert!(model.initial_assignments().is_empty());
}

#[test]
fn forward_list_dependencies_keep_first_appearance_assignment_order() {
    let source = "formula z=b+1,a=2,b=a+3;";
    roundtrip(source);
    let model = compile(source).unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value().expect("explicit initialized test model")))
            .collect::<Vec<_>>(),
        [("a", 2.0), ("b", 5.0), ("z", 6.0)]
    );
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|a| a.symbol())
            .collect::<Vec<_>>(),
        ["z", "b"]
    );
}

#[test]
fn declaration_only_and_mixed_list_entries_preserve_unset_values() {
    for source in ["formula a,b;a=1;b=2;", "formula a,b=a+2;a=1;"] {
        roundtrip(source);
        assert_eq!(compile(source).unwrap().parameters().len(), 2);
    }
    assert_eq!(
        compile("formula a,b=2;").unwrap().parameters()[0].value(),
        None
    );
}

#[test]
fn list_initializers_retain_literal_versus_assignment_mode() {
    let source = "formula a=(2),b=+3,c=-4,d=2+0;";
    roundtrip(source);
    let model = compile(source).unwrap();
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|a| a.symbol())
            .collect::<Vec<_>>(),
        ["a", "b", "d"]
    );
    assert!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "c")
            .unwrap()
            .initial_expression()
            .is_none()
    );
}

#[test]
fn list_dependencies_on_species_preserve_parameter_constant_flags() {
    let source = "species S=2;const species B=3;formula a=S+1,b=B+1,c=a+1;";
    roundtrip(source);
    let model = compile(source).unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.constant()))
            .collect::<Vec<_>>(),
        [("a", false), ("b", true), ("c", false)]
    );
}

#[test]
fn list_annotations_stay_independent_from_numeric_initialization() {
    let source = "unit inv=1/second;formula a=1,b=2;a has inv;b has inv;";
    roundtrip(source);
    let model = compile(source).unwrap();
    assert_eq!(model.units("a"), Some("inv"));
    assert_eq!(model.units("b"), Some("inv"));
    assert_eq!(
        model.parameters()[1]
            .value()
            .expect("explicit initialized test model"),
        2.0
    );
}

#[test]
fn malformed_bare_statements_are_syntax_with_original_identifier_spans() {
    for source in [
        "nonsense;",
        "/* β */ nonsense",
        "nonsense\n",
        "model m\nnonsense end",
    ] {
        let diagnostic = compile(source).unwrap_err();
        assert_eq!(diagnostic.code(), DiagnosticCode::Syntax, "{source}");
        let span = diagnostic.labels()[0].span();
        assert_eq!(span.source_id(), SourceId::new(92));
        assert_eq!(&source[span.range()], "nonsense");
    }
}

#[test]
fn broader_flat_models_still_reject_other_language_families() {
    for source in ["a:=1;", "a'=1;", "a is b;a=1;b=1;"] {
        assert!(compile(source).is_ok(), "{source}");
    }
    for source in [
        "a is \"label\";",
        "compartment c=1;species S;S in c;S=1;",
        "a:=;",
        "a'=;",
        "import \"ignored\";",
        "model m a=1;end",
    ] {
        assert_eq!(
            compile(source).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{source}"
        );
    }
}

#[test]
fn valid_nonempty_ports_remain_unsupported_but_malformed_headers_are_syntax() {
    for source in ["model m(a)\na=1;end", "model m(a,$b)\na=1;b=2;end"] {
        assert_eq!(
            compile(source).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature
        );
    }
    for source in [
        "model m(\na=1;end",
        "model m(1)\na=1;end",
        "model m(a\na=1;end",
        "model m(a,)\na=1;end",
    ] {
        assert_eq!(
            compile(source).unwrap_err().code(),
            DiagnosticCode::Syntax,
            "{source}"
        );
    }
}

#[test]
fn malformed_lists_do_not_expand_into_bare_assignment_or_keyword_alias_syntax() {
    for source in [
        "formula a=1,;",
        "formula a=1,\nb=2;",
        "formula a=1,,b=2;",
        "formula ,a=1;",
        "a=1,b=2;",
    ] {
        assert_eq!(
            compile(source).unwrap_err().code(),
            DiagnosticCode::Syntax,
            "{source}"
        );
    }
    assert!(compile("parameter a=1,b=2;").is_err());
}

#[test]
fn list_semantics_retain_conflict_cycle_and_numeric_domain_guards() {
    for (source, expected) in [
        ("formula a=b,b=a;", DiagnosticCode::InitializationCycle),
        ("formula a=1,b=1/0;", DiagnosticCode::DeclarationConflict),
        ("formula a=1,b=1e-400;", DiagnosticCode::UnsupportedFeature),
        (
            "formula a=1,b=1.234567890123456;",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "species a;compartment a=2;",
            DiagnosticCode::DeclarationConflict,
        ),
    ] {
        assert_eq!(compile(source).unwrap_err().code(), expected, "{source}");
    }
}

#[test]
fn grouped_declarations_obey_retained_resource_limits() {
    let source = "model m\nformula a=1,b=2,c=a+b;end";
    for limits in [
        Limits::default().with_limit(LimitKind::SourceBytes, 10),
        Limits::default().with_limit(LimitKind::Tokens, 10),
        Limits::default().with_limit(LimitKind::ModelEntities, 2),
        Limits::default().with_limit(LimitKind::ExpressionDepth, 1),
    ] {
        let result = parsed(source, &limits).and_then(|document| analyze(&document));
        assert_eq!(result.unwrap_err().code(), DiagnosticCode::ResourceLimit);
    }
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, source.len());
    let document = parsed(source, &limits).unwrap();
    let diagnostic = format_antimony(&document).unwrap_err();
    assert_eq!(diagnostic.code(), DiagnosticCode::ResourceLimit);
    assert_eq!(diagnostic.labels()[0].span().source_id(), SourceId::new(92));
    assert!(source.get(diagnostic.labels()[0].span().range()).is_some());
}

#[test]
fn flat_declarations_expose_the_new_profile_without_changing_public_api() {
    let model = compile("model m\nformula a=1,b=2;end").unwrap();
    assert_eq!(model.profile_id(), "modular-core-v0.8-slice");
    assert_eq!(model.sbml_target(), (3, 2));
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&model).unwrap().profile_id(),
        model.profile_id()
    );
}
