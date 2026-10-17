//! Regression boundaries discovered after the numeric-builtin matrix.
use antimony::{
    DiagnosticCode, ExpressionKind, Limits, SourceFile, SourceId, ValidatedModel, analyze,
    format_antimony, parse,
};
fn compile(source: &str) -> Result<ValidatedModel, antimony::Diagnostic> {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(341), "rational", source, &limits).unwrap();
    analyze(&parse(&source, &limits)?)
}
fn root(model: &ValidatedModel) -> &ExpressionKind {
    model
        .expression(model.initial_assignments()[0].expression())
        .unwrap()
        .kind()
}
#[test]
fn rational_is_unreduced_signed_atom_not_division() {
    for (expr, numerator, denominator) in [
        ("(1/2)", 1, 2),
        ("(2/4)", 2, 4),
        ("(1/-2)", 1, -2),
        ("(-1/-2)", -1, -2),
        ("(0/-2)", 0, -2),
        ("(1.0/2e0)", 1, 2),
        ("(01/002)", 1, 2),
    ] {
        let model = compile(&format!("a={expr};")).unwrap();
        assert!(
            matches!(root(&model), ExpressionKind::Rational { numerator:n,denominator:d } if (*n,*d)==(numerator,denominator)),
            "{expr}"
        );
        assert_eq!(
            model.parameters()[0].value(),
            Some(numerator as f64 / denominator as f64)
        );
    }
}
#[test]
fn lexical_call_parentheses_are_not_division_parentheses() {
    for function in [
        "sin", "cos", "tan", "exp", "ln", "log", "log10", "sqrt", "abs", "floor", "ceil",
        "ceiling", "min", "max",
    ] {
        for arg in [
            "1/2",
            "-1/2",
            "1/-2",
            "1.0/2",
            "1e0/2",
            "1e14/2",
            "1/*comment*/ /2",
        ] {
            let source = format!("a={function}({arg});");
            assert_eq!(
                compile(&source).unwrap_err().code(),
                DiagnosticCode::Syntax,
                "{source}"
            );
        }
    }
}
#[test]
fn nearby_division_and_numeric_notation_remain_supported() {
    for expr in [
        "sin((1)/2)",
        "sin(1/(2))",
        "sin(+1/2)",
        "sin(1/+2)",
        "sin(-+1/2)",
        "sin(1/-+2)",
        "sin(0.5/2)",
        "sin(1/2+0)",
        "sin(1e15/2)",
        "pow(1/2,3)",
        "log(2,1/2)",
        "min(1/2,3)",
        "((2147483648)/2)",
        "(2147483648/(2))",
    ] {
        compile(&format!("a={expr};")).unwrap_or_else(|e| panic!("{expr}: {e:?}"));
    }
}
#[test]
fn signed_i32_bounds_apply_only_after_all_unary_normalization() {
    for expr in [
        "(2147483647/2)",
        "(-2147483648/2)",
        "(1/2147483647)",
        "(1/-2147483648)",
        "-(-(-2147483648/2))",
        "-(2147483648/2)",
        "- -(1/2)",
    ] {
        compile(&format!("a={expr};")).unwrap_or_else(|e| panic!("{expr}: {e:?}"));
    }
    for expr in [
        "(2147483648/2)",
        "(-2147483649/2)",
        "(1/2147483648)",
        "(1/-2147483649)",
        "-(-2147483648/2)",
    ] {
        assert_eq!(
            compile(&format!("a={expr};")).unwrap_err().code(),
            DiagnosticCode::Syntax,
            "{expr}"
        );
    }
}
#[test]
fn nonfinite_rational_literals_reject_even_in_rates_but_divide_is_preserved() {
    for expr in ["(1/0)", "(0/0)", "(-1/0)", "(1/-0)", "sin((1/0))"] {
        for source in [format!("a={expr};"), format!("S=0;J:=>S;{expr};")] {
            assert_eq!(
                compile(&source).unwrap_err().code(),
                DiagnosticCode::UnsupportedFeature,
                "{source}"
            );
        }
    }
    compile("S=0;J:=>S;1/0;").unwrap();
    assert_eq!(
        compile("a=1/0;").unwrap_err().code(),
        DiagnosticCode::DeclarationConflict
    );
}
#[test]
fn rational_arithmetic_keeps_unknowns_and_dependencies() {
    let model = compile("a=(1/2)*u;b=sin((2/4))+a;").unwrap();
    assert!(model.parameters().iter().all(|p| p.value().is_none()));
    assert_eq!(
        compile("a=(1/2)*b;b=a;").unwrap_err().code(),
        DiagnosticCode::InitializationCycle
    );
    let model = compile("species S=1,T=2;J:S=>;(1/2)*T;").unwrap();
    assert_eq!(model.reactions()[0].modifiers(), &["T"]);
}
#[test]
fn rational_units_remain_conservative() {
    compile("x=2;x has second;a=(1/2)*x;a has second;").unwrap();
    let model = compile("a=(1/2);").unwrap();
    assert_eq!(
        model.expression_dimensions(model.initial_assignments()[0].expression()),
        Some(antimony::DimensionalStatus::Unspecified)
    );
}
#[test]
fn formatter_retains_lexical_grouping_and_numeric_spelling() {
    let limits = Limits::default();
    let source = SourceFile::new(
        SourceId::new(341),
        "format",
        "a=sin((1.0/*rational*/ /2e0));b=sin((1)/2);",
        &limits,
    )
    .unwrap();
    let parsed = parse(&source, &limits).unwrap();
    let formatted = format_antimony(&parsed).unwrap();
    assert_eq!(formatted, "a = sin((1.0 / 2e0));\nb = sin((1) / 2);\n");
    assert!(parsed.source().text().contains("/*rational*/"));
    let source2 = SourceFile::new(SourceId::new(341), "again", &formatted, &limits).unwrap();
    assert_eq!(
        formatted,
        format_antimony(&parse(&source2, &limits).unwrap()).unwrap()
    );
    let first = analyze(&parsed).unwrap();
    let second = analyze(&parse(&source2, &limits).unwrap()).unwrap();
    assert_eq!(
        first.parameters()[0].value(),
        second.parameters()[0].value()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&first).unwrap().xml(),
        antimony::write_sbml(&second).unwrap().xml()
    );
}
#[test]
#[cfg(feature = "sbml")]
fn mathml_preserves_signed_unreduced_atom_and_function_child() {
    for (expr, expected) in [
        ("(2/4)", "<cn type=\"rational\">2<sep/>4</cn>"),
        ("-(1/-2)", "<cn type=\"rational\">-1<sep/>-2</cn>"),
        (
            "sin((1/2))",
            "<apply><sin/><cn type=\"rational\">1<sep/>2</cn></apply>",
        ),
    ] {
        let model = compile(&format!("a={expr};")).unwrap();
        assert!(
            antimony::write_sbml(&model)
                .unwrap()
                .xml()
                .contains(expected),
            "{expr}"
        );
    }
}
