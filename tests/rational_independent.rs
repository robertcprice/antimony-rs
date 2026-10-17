//! Independent lexical, semantic and resource checks for rational MathML leaves.
use antimony::{
    BinaryOp, Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionId, ExpressionKind,
    LimitKind, Limits, SourceFile, SourceId, ValidatedModel, analyze, format_antimony, parse,
};
use std::collections::HashSet;

fn compile(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(SourceId::new(842), "independent rational", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}
fn model(text: &str) -> ValidatedModel {
    compile(text, &Limits::default()).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}
fn root(m: &ValidatedModel, name: &str) -> ExpressionId {
    m.initial_assignments()
        .iter()
        .find(|x| x.symbol() == name)
        .unwrap()
        .expression()
}
fn rational(m: &ValidatedModel, id: ExpressionId) -> (i64, i64) {
    match m.expression(id).unwrap().kind() {
        ExpressionKind::Rational {
            numerator,
            denominator,
        } => (*numerator, *denominator),
        other => panic!("expected rational, found {other:?}"),
    }
}
fn value(m: &ValidatedModel, name: &str) -> Option<f64> {
    m.parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
}
fn rejected(text: &str, code: DiagnosticCode) {
    let e = compile(text, &Limits::default()).expect_err(text);
    assert_eq!(e.code(), code, "{text}: {e:?}");
    assert!(!e.labels().is_empty(), "{text}: unlocated diagnostic");
}
fn roundtrip(text: &str) -> ValidatedModel {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(842), "rational layout", text, &limits).unwrap();
    let p = parse(&source, &limits).unwrap();
    let before = analyze(&p).unwrap();
    let formatted = format_antimony(&p).unwrap();
    let second = SourceFile::new(SourceId::new(843), "formatted", &formatted, &limits).unwrap();
    let q = parse(&second, &limits).unwrap();
    assert_eq!(formatted, format_antimony(&q).unwrap());
    let after = analyze(&q).unwrap();
    assert_eq!(
        before
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>(),
        after
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&before).unwrap().xml(),
        antimony::write_sbml(&after).unwrap().xml()
    );
    before
}

#[test]
fn rational_components_are_unreduced_and_signed_not_divide_nodes() {
    for (expr, pair) in [
        ("(2/4)", (2, 4)),
        ("(-1/-2)", (-1, -2)),
        ("(1/-2)", (1, -2)),
        ("(-0/-2)", (0, -2)),
        ("((1/2))", (1, 2)),
        ("(1.0/2e0)", (1, 2)),
        ("(1.00000000000000/2)", (1, 2)),
        ("(0001/0002)", (1, 2)),
    ] {
        let m = roundtrip(&format!("a={expr};"));
        assert_eq!(rational(&m, root(&m, "a")), pair, "{expr}");
        assert_eq!(value(&m, "a"), Some(pair.0 as f64 / pair.1 as f64));
    }
}

#[test]
fn rational_negation_folds_only_final_retained_numerator() {
    for (expr, pair) in [
        ("-(1/2)", (-1, 2)),
        ("-(-(1/2))", (1, 2)),
        ("+(1/2)", (1, 2)),
        ("-(1/-2)", (-1, -2)),
        ("(-2147483648/2)", (-2147483648, 2)),
        ("(1/-2147483648)", (1, -2147483648)),
        ("-(-2147483647/2)", (2147483647, 2)),
        ("-(-(-2147483648/2))", (-2147483648, 2)),
    ] {
        let m = model(&format!("a={expr};"));
        assert_eq!(rational(&m, root(&m, "a")), pair, "{expr}");
    }
}

#[test]
fn rational_overflow_is_rejected_instead_of_truncating_or_reinterpreting() {
    for expr in [
        "(2147483648/2)",
        "(-2147483649/2)",
        "(1/2147483648)",
        "(1/-2147483649)",
        "-(-2147483648/2)",
        "(999999999999999/2)",
        "(2.147483648e9/2)",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::Syntax);
    }
}

#[test]
fn ordinary_division_escape_shapes_do_not_become_rational() {
    for expr in [
        "1/2",
        "(+1/2)",
        "(1/+2)",
        "((1)/2)",
        "(1/(2))",
        "((1)/(2))",
        "(0.5/2)",
        "(1.1/2)",
        "(1/2.1)",
        "(1e15/2)",
        "(1/1e15)",
        "((2147483648)/2)",
        "(2147483648/(2))",
        "(2147483648.1/2)",
    ] {
        let m = roundtrip(&format!("a={expr};"));
        assert!(
            matches!(
                m.expression(root(&m, "a")).unwrap().kind(),
                ExpressionKind::Binary {
                    op: BinaryOp::Divide,
                    ..
                }
            ),
            "{expr}"
        );
    }
}

#[test]
fn call_open_consumption_is_syntax_not_a_false_successful_builtin() {
    for f in [
        "sin", "cos", "tan", "exp", "ln", "log", "log10", "sqrt", "abs", "floor", "ceil",
        "ceiling", "min", "max",
    ] {
        for ratio in [
            "1/2",
            "1 / 2",
            "1.0/2",
            "1/2.0",
            "1e0/2",
            "-1/-2",
            "1/*comment*//2",
        ] {
            rejected(&format!("a={f}({ratio});"), DiagnosticCode::Syntax);
        }
    }
}

#[test]
fn builtin_arguments_preserve_extra_groups_and_real_division() {
    for f in [
        "sin", "cos", "tan", "exp", "ln", "log", "log10", "sqrt", "abs", "floor", "ceil",
        "ceiling", "min", "max",
    ] {
        let m = model(&format!("a={f}((1/2));"));
        let ExpressionKind::Call { args, .. } = m.expression(root(&m, "a")).unwrap().kind() else {
            panic!("{f}")
        };
        assert_eq!(rational(&m, args[0]), (1, 2));
        let n = model(&format!("a={f}((1)/2);"));
        let ExpressionKind::Call { args, .. } = n.expression(root(&n, "a")).unwrap().kind() else {
            panic!("{f}")
        };
        assert!(matches!(
            n.expression(args[0]).unwrap().kind(),
            ExpressionKind::Binary {
                op: BinaryOp::Divide,
                ..
            }
        ));
    }
    for source in [
        "a=pow(1/2,3);",
        "a=log(2,1/2);",
        "a=min(1/2,3);",
        "a=max(1/2,3);",
        "a=sin(1/2+0);",
        "a=sin(+1/2);",
        "a=sin(1e15/2);",
    ] {
        roundtrip(source);
    }
}

#[test]
fn nested_ratios_keep_leaf_identity_without_outer_reclassification() {
    let m = model("a=((1/2)/(3/4));b=(1/(2/3));c=((1/2)/3);");
    let ExpressionKind::Binary {
        op: BinaryOp::Divide,
        left,
        right,
    } = m.expression(root(&m, "a")).unwrap().kind()
    else {
        panic!()
    };
    assert_eq!(rational(&m, *left), (1, 2));
    assert_eq!(rational(&m, *right), (3, 4));
    assert_eq!(value(&m, "a"), Some(2.0 / 3.0));
    assert_eq!(value(&m, "b"), Some(1.5));
    assert_eq!(value(&m, "c"), Some(1.0 / 6.0));
}

#[test]
fn rational_zero_denominators_never_export_nonfinite_literals() {
    for expr in [
        "(1/0)",
        "(0/0)",
        "(-1/-0)",
        "(1/0.0)",
        "(1/0e0)",
        "(1/0e999)",
    ] {
        for source in [
            format!("a={expr};"),
            format!("species S=0;J:=>S;{expr};"),
            format!("a=u+{expr};"),
        ] {
            rejected(&source, DiagnosticCode::UnsupportedFeature);
        }
    }
    // Unparenthesized division in a runtime rate remains structural, not evaluated.
    model("species S=0;J:=>S;1/0;");
    rejected("a=1/0;", DiagnosticCode::DeclarationConflict);
}

#[test]
fn rational_initialization_retains_unknowns_and_dependency_validation() {
    let m = model("a=u+(1/2);b=(1/2)*0;c=u*(0/2);");
    assert_eq!(value(&m, "a"), None);
    assert_eq!(value(&m, "b"), Some(0.0));
    assert_eq!(value(&m, "c"), None);
    rejected("a=(1/2)+b;b=a;", DiagnosticCode::InitializationCycle);
    rejected("a=sqrt((-1/2));", DiagnosticCode::DeclarationConflict);
}

#[test]
fn rational_dimensions_remain_unspecified_like_bare_numeric_literals() {
    let m = model("a=(1/2);x=2;x has second;b=(1/2)*x;");
    assert_eq!(
        m.expression_dimensions(root(&m, "a")),
        Some(DimensionalStatus::Unspecified)
    );
    // A numeric leaf does not gain a fabricated dimensionless annotation.
    assert_ne!(m.expression_dimensions(root(&m, "b")), None);
    rejected(
        "x=2;x has second;y=3;y has metre;a=(1/2)*(x+y);",
        DiagnosticCode::UnitsInconsistency,
    );
}

#[test]
fn rational_provenance_and_utf8_formatting_remain_located() {
    let source = "/* β source */ a = (1.0 /* numerator */ / -2e0);";
    let m = roundtrip(source);
    let e = m.expression(root(&m, "a")).unwrap();
    assert_eq!(rational(&m, root(&m, "a")), (1, -2));
    assert!(e.span().start() < e.span().end());
    let text = &source[e.span().start()..e.span().end()];
    assert!(text.contains('/') && text.contains("1.0"));
}

#[test]
fn rational_nodes_do_not_expand_normalized_arena_or_lose_child_ids() {
    let n = 80usize;
    let source = format!(
        "a={};",
        std::iter::repeat_n("(1/2)", n)
            .collect::<Vec<_>>()
            .join("+")
    );
    let m = model(&source);
    let mut pending = vec![root(&m, "a")];
    let mut seen = HashSet::new();
    let mut leaves = 0;
    while let Some(id) = pending.pop() {
        assert!(seen.insert(id), "unexpected shared/cyclic expression id");
        match m.expression(id).expect("model-owned child id").kind() {
            ExpressionKind::Rational { .. } => leaves += 1,
            ExpressionKind::Binary { left, right, .. } => {
                pending.push(*left);
                pending.push(*right);
            }
            other => panic!("unexpected node {other:?}"),
        }
    }
    assert_eq!(leaves, n);
    assert_eq!(seen.len(), 2 * n - 1);
    assert_eq!(value(&m, "a"), Some(40.0));
}

#[test]
fn rational_fast_recognition_cannot_bypass_depth_or_token_limits() {
    let source = format!("a={}1/2{};", "(".repeat(40), ")".repeat(40));
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 8);
    assert_eq!(
        compile(&source, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::Tokens, 5);
    assert_eq!(
        compile("a=(1/2);", &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let source = format!(
        "a={};",
        std::iter::repeat_n("(1/2)", 300)
            .collect::<Vec<_>>()
            .join("+")
    );
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 10_000);
    assert_eq!(
        compile(&source, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[cfg(feature = "sbml")]
#[test]
fn sbml_emits_exact_rational_components_without_divide_or_reduction() {
    let m = model("a=(2/4);b=(-1/-2);c=sin((1/2));");
    let xml = antimony::write_sbml(&m).unwrap();
    assert!(xml.xml().contains("<cn type=\"rational\">2<sep/>4</cn>"));
    assert!(xml.xml().contains("<cn type=\"rational\">-1<sep/>-2</cn>"));
    assert!(xml.xml().contains("<cn type=\"rational\">1<sep/>2</cn>"));
    assert!(!xml.xml().contains("<divide/>"));
}
