//! Native arithmetic representation, numeric categories and source fidelity regressions.
use antimony::{ExpressionKind, Limits, SourceFile, SourceId, analyze, format_antimony, parse};
fn model(text: &str) -> antimony::ValidatedModel {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(919), "arithmetic", text, &limits).unwrap();
    analyze(&parse(&source, &limits).unwrap()).unwrap()
}
#[test]
fn scientific_initializer_value_uses_exported_mantissa_interpretation() {
    let model = model("answer=(9.999e-5);");
    let parameter = &model.parameters()[0];
    let expected = 9.999_f64 * 10.0_f64.powf(-5.0);
    assert_eq!(
        parameter
            .value()
            .expect("explicit initialized test model")
            .to_bits(),
        expected.to_bits()
    );
    match model
        .expression(parameter.initial_expression().unwrap())
        .unwrap()
        .kind()
    {
        ExpressionKind::Number { lexeme, value } => {
            assert_eq!(lexeme, "9.999e-5");
            assert_eq!(value.to_bits(), expected.to_bits());
        }
        _ => panic!("expected normalized numeric leaf"),
    }
}
#[test]
fn source_grouping_survives_semantic_normalization_and_formatting() {
    let limits = Limits::default();
    let source = SourceFile::new(
        SourceId::new(919),
        "source",
        "a=2+(3+4);b=(2+(3+4))+5;",
        &limits,
    )
    .unwrap();
    let formatted = format_antimony(&parse(&source, &limits).unwrap()).unwrap();
    assert_eq!(formatted, "a = 2 + (3 + 4);\nb = (2 + (3 + 4)) + 5;\n");
    let again = SourceFile::new(SourceId::new(919), "again", &formatted, &limits).unwrap();
    assert_eq!(
        format_antimony(&parse(&again, &limits).unwrap()).unwrap(),
        formatted
    );
}
#[test]
#[cfg(feature = "sbml")]
fn numeric_ast_categories_and_source_digits_are_preserved() {
    for (literal, expected) in [
        ("2147483647", "<cn type=\"integer\">2147483647</cn>"),
        ("2147483648", "<cn type=\"real\">2147483648</cn>"),
        ("-2147483648", "<cn type=\"real\">-2147483648</cn>"),
        ("1e-4", "<cn type=\"real\">0.0001</cn>"),
        ("1e-5", "<cn type=\"e-notation\">1<sep/>-5</cn>"),
        ("1e15", "<cn type=\"e-notation\">1<sep/>15</cn>"),
        (
            "1.23456789012345e-200",
            "<cn type=\"e-notation\">1.23456789012345<sep/>-200</cn>",
        ),
        (
            "0.000000000000009999",
            "<cn type=\"e-notation\">9.999<sep/>-15</cn>",
        ),
        ("-1e308", "<cn type=\"e-notation\">-1<sep/>308</cn>"),
    ] {
        let model = model(&format!("S=2;J:S=>;{literal}*S;"));
        assert!(
            antimony::write_sbml(&model)
                .unwrap()
                .xml()
                .contains(expected),
            "{literal}"
        );
    }
}
#[test]
fn repeated_analysis_cannot_mutate_the_source_tree() {
    let limits = Limits::default();
    let source = SourceFile::new(
        SourceId::new(919),
        "stable",
        "answer=(1e16+(-1e16+1))+1;",
        &limits,
    )
    .unwrap();
    let parsed = parse(&source, &limits).unwrap();
    for _ in 0..5 {
        assert_eq!(
            analyze(&parsed).unwrap().parameters()[0]
                .value()
                .expect("explicit initialized test model"),
            1.0
        );
    }
}
