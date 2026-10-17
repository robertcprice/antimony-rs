//! Independently authored builtin/time regressions; no corpus-ID branches.
use antimony::{
    Builtin, Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionKind, LimitKind, Limits,
    NumericConstant, SourceFile, SourceId, ValidatedModel, analyze, format_antimony, parse,
};

fn compile(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(SourceId::new(731), "independent builtins", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}
fn model(text: &str) -> ValidatedModel {
    compile(text, &Limits::default()).unwrap()
}
fn value(text: &str, name: &str) -> Option<f64> {
    model(text)
        .parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
}
fn initial<'a>(m: &'a ValidatedModel, name: &str) -> &'a antimony::Expression {
    let root = m
        .initial_assignments()
        .iter()
        .find(|a| a.symbol() == name)
        .unwrap()
        .expression();
    m.expression(root).unwrap()
}
fn roundtrip(text: &str) -> ValidatedModel {
    let limits = Limits::default();
    let source = SourceFile::new(
        SourceId::new(731),
        "independent builtin layout",
        text,
        &limits,
    )
    .unwrap();
    let parsed = parse(&source, &limits).unwrap();
    let before = analyze(&parsed).unwrap();
    let output = format_antimony(&parsed).unwrap();
    let after_source = SourceFile::new(SourceId::new(732), "formatted", &output, &limits).unwrap();
    let after_parsed = parse(&after_source, &limits).unwrap();
    assert_eq!(output, format_antimony(&after_parsed).unwrap());
    let after = analyze(&after_parsed).unwrap();
    assert_eq!(
        before
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value(), p.constant()))
            .collect::<Vec<_>>(),
        after
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value(), p.constant()))
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
fn aliases_resolve_to_typed_shared_builtins_without_fabricated_parameters() {
    for (text, expected, arity) in [
        ("x=log(2);", Builtin::Ln, 1),
        ("x=ln(2);", Builtin::Ln, 1),
        ("x=log(2,8);", Builtin::Log, 2),
        ("x=log10(100);", Builtin::Log10, 1),
        ("x=ceil(1.2);", Builtin::Ceiling, 1),
        ("x=ceiling(1.2);", Builtin::Ceiling, 1),
        ("x=pow(2,3);", Builtin::Power, 2),
        ("x=power(2,3);", Builtin::Power, 2),
    ] {
        let m = roundtrip(text);
        assert_eq!(m.parameters().len(), 1);
        let ExpressionKind::Call { function, args } = initial(&m, "x").kind() else {
            panic!("{text}")
        };
        assert_eq!(*function, expected);
        assert_eq!(args.len(), arity);
    }
}

#[test]
fn elementary_numeric_functions_evaluate_without_losing_initial_assignments() {
    for (formula, expected) in [
        ("sin(0)", 0.0),
        ("cos(0)", 1.0),
        ("tan(0)", 0.0),
        ("exp(0)", 1.0),
        ("ln(1)", 0.0),
        ("log(2,8)", 3.0),
        ("log10(100)", 2.0),
        ("sqrt(9)", 3.0),
        ("abs(-2)", 2.0),
        ("floor(-1.2)", -2.0),
        ("ceil(-1.2)", -1.0),
        ("pow(-2,3)", -8.0),
        ("min(4,2,-1,3)", -1.0),
        ("max(-4,-2,-1,-3)", -1.0),
        ("min(4)", 4.0),
        ("max(-4)", -4.0),
    ] {
        let m = roundtrip(&format!("x={formula};"));
        assert_eq!(m.parameters()[0].value(), Some(expected), "{formula}");
        assert!(m.parameters()[0].initial_expression().is_some());
    }
}

#[test]
fn constants_are_typed_values_not_free_symbols() {
    let m = roundtrip("x=pi;y=exponentiale;");
    assert_eq!(m.parameters().len(), 2);
    assert!(matches!(
        initial(&m, "x").kind(),
        ExpressionKind::Constant(NumericConstant::Pi)
    ));
    assert!(matches!(
        initial(&m, "y").kind(),
        ExpressionKind::Constant(NumericConstant::ExponentialE)
    ));
    assert_eq!(m.parameters()[0].value(), Some(std::f64::consts::PI));
    assert_eq!(m.parameters()[1].value(), Some(std::f64::consts::E));
}

#[test]
fn time_alias_is_symbolic_not_guessed_initial_zero() {
    for spelling in ["time", "time()"] {
        let m = roundtrip(&format!("x={spelling};y=x+1;"));
        assert!(matches!(initial(&m, "x").kind(), ExpressionKind::Time));
        assert!(
            m.parameters()
                .iter()
                .all(|p| p.value().is_none() && p.constant())
        );
        assert_eq!(m.initial_assignments().len(), 2);
        assert_eq!(m.parameters().len(), 2);
    }
}

#[test]
fn builtin_arguments_use_contextual_arithmetic_normalization() {
    assert_eq!(value("x=abs((1e16+(-1e16+1))+1);", "x"), Some(1.0));
    assert_eq!(value("x=abs(1e16+(-1e16+1));", "x"), Some(1.0));
    assert_eq!(value("x=max((1e16+(-1e16+1))+1,0);", "x"), Some(1.0));
    let answer = value("x=abs(1e200*(1e-200*1e-200));", "x").unwrap();
    assert!(answer > 0.0 && (answer / 1e-200 - 1.0).abs() < 1e-14);
}

#[test]
fn nested_calls_preserve_expression_boundaries() {
    assert_eq!(
        value("x=log(2,pow(2,abs(-3)))+sqrt(max(0,4));", "x"),
        Some(5.0)
    );
    assert_eq!(value("x=pow(2,3)^2;", "x"), Some(64.0));
    assert_eq!(value("x=pow(2,3^2);", "x"), Some(512.0));
    assert_eq!(value("x=-pow(2,2);", "x"), Some(-4.0));
    assert_eq!(value("x=pow(-2,2);", "x"), Some(4.0));
}

#[test]
fn unknown_call_arguments_remain_unknown_even_with_apparent_shortcuts() {
    for formula in [
        "sin(u)", "min(u,0)", "max(0,u)", "pow(u,0)", "pow(0,u)", "log(u,2)", "sqrt(u)",
    ] {
        let m = roundtrip(&format!("x={formula};"));
        assert_eq!(m.parameters().len(), 2);
        assert!(
            m.parameters().iter().all(|p| p.value().is_none()),
            "{formula}"
        );
    }
}

#[test]
fn known_invalid_subtrees_are_checked_despite_unknown_siblings() {
    for formula in [
        "min(u,sqrt(-1))",
        "max(1/0,u)",
        "max(u,exp(1000))",
        "sin(u+1/0)",
        "log(1,u)",
        "log(u,-1)",
        "pow(0,-1)",
    ] {
        let error = compile(&format!("x={formula};"), &Limits::default()).unwrap_err();
        assert_eq!(
            error.code(),
            DiagnosticCode::DeclarationConflict,
            "{formula}"
        );
        assert!(!error.labels().is_empty());
    }
}

#[test]
fn finite_real_initialization_domain_is_preserved() {
    for formula in [
        "log(0)",
        "ln(-1)",
        "log10(0)",
        "log(1,2)",
        "log(-2,2)",
        "sqrt(-1)",
        "pow(-2,0.5)",
        "pow(0,0)",
        "pow(0,-2)",
        "exp(1000)",
    ] {
        assert_eq!(
            compile(&format!("x={formula};"), &Limits::default())
                .unwrap_err()
                .code(),
            DiagnosticCode::DeclarationConflict,
            "{formula}"
        );
    }
    assert_eq!(value("x=log(0.5,4);", "x"), Some(-2.0));
}

#[test]
fn supported_call_cycles_remain_initialization_cycles() {
    for text in [
        "a=sin(a);",
        "a=min(b,1);b=max(a,2);",
        "a=sin(u+b);b=cos(a);",
    ] {
        assert_eq!(
            compile(text, &Limits::default()).unwrap_err().code(),
            DiagnosticCode::InitializationCycle
        );
    }
}

#[test]
fn call_arity_and_argument_syntax_are_not_silently_repaired() {
    for formula in [
        "sin()",
        "sin(1,2)",
        "log()",
        "log(2,3,4)",
        "ln(2,8)",
        "log10(2,8)",
        "sqrt(2,4)",
        "pow(2)",
        "pow(2,3,4)",
        "min(1,,2)",
        "max(1,2,)",
        "sin(1;2)",
        "min(1,\n2)",
    ] {
        assert_eq!(
            compile(&format!("x={formula};"), &Limits::default())
                .unwrap_err()
                .code(),
            DiagnosticCode::Syntax,
            "{formula}"
        );
    }
    for formula in ["min()", "max()"] {
        assert_eq!(
            compile(&format!("x={formula};"), &Limits::default())
                .unwrap_err()
                .code(),
            DiagnosticCode::UnsupportedFeature
        );
    }
    assert!(compile("x=time(2);", &Limits::default()).is_err()); // Reference loads but produces missing MathML.
}

#[test]
fn call_references_infer_symbols_and_reaction_modifiers() {
    let m = roundtrip("species A=1,B=0,E=2; A=>B;max(A,sin(E))*exp(-time);");
    assert_eq!(m.reactions()[0].modifiers(), ["E"]);
    assert!(m.parameters().is_empty());
    let m = roundtrip("A=>B;sin(_J0);");
    assert_eq!(m.reactions()[0].id(), "_J1");
    assert_eq!(m.parameters()[0].id(), "_J0");
    assert_eq!(m.parameters()[0].value(), None);
}

#[test]
fn builtin_dependencies_propagate_symbol_mutability() {
    for qualifier in ["", "const "] {
        let m = model(&format!(
            "{qualifier}species S=2;formula x=abs(S),y=max(x,1);"
        ));
        assert!(
            m.parameters()
                .iter()
                .all(|p| p.constant() != qualifier.is_empty())
        );
    }
}

#[test]
fn builtin_names_cannot_be_redeclared_or_shadowed_as_symbols() {
    for name in [
        "sin",
        "log",
        "sqrt",
        "ceil",
        "pow",
        "min",
        "time",
        "pi",
        "exponentiale",
    ] {
        for source in [format!("formula {name}=2;"), format!("species {name}=2;")] {
            assert!(compile(&source, &Limits::default()).is_err(), "{source}");
        }
    }
    // Conditional-core now retains this time-dependent initializer symbolically.
    assert!(compile("x=piecewise(1,time>0,0);", &Limits::default()).is_ok());
    for text in ["formula f=2;x=f(3);", "x=foo(1);"] {
        assert!(compile(text, &Limits::default()).is_err());
    }
}

fn dims(model: &ValidatedModel, name: &str) -> DimensionalStatus {
    let root = model
        .initial_assignments()
        .iter()
        .find(|a| a.symbol() == name)
        .unwrap()
        .expression();
    model.expression_dimensions(root).unwrap()
}
#[test]
fn builtin_unit_information_is_conservative_not_guessed_dimensionless() {
    let m = model("x=sin(u);y=pi;z=time;");
    assert_eq!(dims(&m, "x"), DimensionalStatus::Indeterminate);
    assert_eq!(dims(&m, "z"), DimensionalStatus::Unspecified);
    let DimensionalStatus::Known(d) = dims(&m, "y") else {
        panic!("pi should be dimensionless")
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 0));
    let m = model("unit time_unit=second;x=time;");
    let DimensionalStatus::Known(d) = dims(&m, "x") else {
        panic!("explicit model time")
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 1, 0));
    let m = model("unit time_unit=dimensionless;x=time;");
    let DimensionalStatus::Known(d) = dims(&m, "x") else {
        panic!("dimensionless model time")
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 0));
}

#[test]
fn dimensioned_calls_reject_known_mismatches_even_with_unknown_arguments() {
    for text in [
        "x=1;x has second;y=sin(x);",
        "x=1;x has mole;y=exp(x);",
        "x=1;x has second;y=2;y has mole;z=min(u,x,y);",
        "x=1;x has second;y=log(x,2);",
    ] {
        assert!(compile(text, &Limits::default()).is_err(), "{text}");
    }
}

#[test]
fn square_roots_and_shape_preserving_calls_retain_known_dimensions() {
    let m = model("unit square=metre^2;x=4;x has square;y=sqrt(x);z=abs(x);w=floor(x);v=ceil(x);");
    let DimensionalStatus::Known(d) = dims(&m, "y") else {
        panic!("square root")
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 1));
    for name in ["z", "w", "v"] {
        let DimensionalStatus::Known(d) = dims(&m, name) else {
            panic!("shape preservation {name}")
        };
        assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 2));
    }
    let m = model("x=2;x has metre;y=sqrt(x);");
    assert_eq!(dims(&m, "y"), DimensionalStatus::Indeterminate);
}

#[test]
fn nested_calls_and_variadic_width_obey_existing_resource_limits() {
    let nested = format!("x={}1{};", "sin(".repeat(20), ")".repeat(20));
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 10);
    assert_eq!(
        compile(&nested, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let wide = format!(
        "x=min({});",
        std::iter::repeat_n("u", 200).collect::<Vec<_>>().join(",")
    );
    let limits = Limits::default().with_limit(LimitKind::Tokens, 50);
    assert_eq!(
        compile(&wide, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 2);
    assert!(
        compile(&wide, &limits).is_ok(),
        "width is not recursive depth"
    );
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 2);
    assert_eq!(
        compile("x=min(a,b);", &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[cfg(feature = "sbml")]
#[test]
fn sbml_distinguishes_logs_roots_constants_and_time_structurally() {
    let m = model("a=log(2,8);b=log10(100);c=sqrt(4);d=pi;e=exponentiale;f=time;");
    let xml = antimony::write_sbml(&m).unwrap();
    let xml = xml.xml();
    for required in [
        "<log/>",
        "<logbase>",
        "<root/>",
        "<degree>",
        "<pi/>",
        "<exponentiale/>",
        "http://www.sbml.org/sbml/symbols/time",
    ] {
        assert!(xml.contains(required), "missing {required}");
    }
    assert!(!xml.contains("id=\"time\""));
    assert!(!xml.contains("id=\"pi\""));
    assert!(!xml.contains("<assignmentRule"));
}

#[test]
fn raised_limits_do_not_remove_hard_stack_safety() {
    const CHILD: &str = "ANTIMONY_BUILTIN_DEPTH_REVIEW_CHILD";
    if let Ok(case) = std::env::var(CHILD) {
        let n = 3_000;
        let expression = match case.as_str() {
            "calls" => format!("{}1{}", "abs(".repeat(n), ")".repeat(n)),
            "groups" => format!("{}1{}", "(".repeat(n), ")".repeat(n)),
            "unary" => format!("{}1", "- ".repeat(n)),
            "power" => std::iter::repeat_n("1", n).collect::<Vec<_>>().join("^"),
            "mixed" => format!("{}1{}", "abs(-(1+".repeat(n), ")))".repeat(n)),
            "below" => format!("{}1{}", "abs(".repeat(96), ")".repeat(96)),
            _ => panic!("unknown child case"),
        };
        let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 10_000);
        let result = compile(&format!("x={expression};"), &limits);
        if case == "below" {
            assert_eq!(result.unwrap().parameters()[0].value(), Some(1.0));
        } else {
            assert_eq!(result.unwrap_err().code(), DiagnosticCode::ResourceLimit);
        }
        return;
    }
    // Test-thread stacks are deliberately not enlarged. Each adversarial source
    // gets its own process, so a regression is reported instead of killing the
    // whole integration-test process with an uncaught stack-overflow abort.
    for case in ["calls", "groups", "unary", "power", "mixed", "below"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "raised_limits_do_not_remove_hard_stack_safety",
                "--nocapture",
            ])
            .env(CHILD, case)
            .output()
            .unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
    }
}

#[test]
fn malformed_nested_calls_return_located_diagnostics_without_panics() {
    for function in ["sin", "min", "log", "pow"] {
        for arguments in [
            "(", ")", ",1", "1,", "1+", "1*/2", "(1,2", "1;2", "1,,2", "abs(", "abs())", "((1)",
        ] {
            let text = format!("x={function}({arguments});");
            let result = std::panic::catch_unwind(|| compile(&text, &Limits::default()));
            let error = result
                .expect("malformed source must not panic")
                .expect_err(&text);
            assert!(!error.labels().is_empty(), "{text}");
            for label in error.labels() {
                assert_eq!(label.span().source_id(), SourceId::new(731));
                assert!(text.get(label.span().start()..label.span().end()).is_some());
            }
        }
    }
}
