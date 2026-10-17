//! Numeric builtins are typed, bounded and share arithmetic normalization.
use antimony::{
    Builtin, DiagnosticCode, DimensionalStatus, ExpressionKind, LimitKind, Limits, NumericConstant,
    SourceFile, SourceId, ValidatedModel, analyze, format_antimony, parse,
};
fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, antimony::Diagnostic> {
    let source = SourceFile::new(SourceId::new(330), "numeric builtins", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}
fn compile(text: &str) -> Result<ValidatedModel, antimony::Diagnostic> {
    compile_with(text, &Limits::default())
}
fn value(model: &ValidatedModel, name: &str) -> Option<f64> {
    model
        .parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
}
#[test]
fn numeric_builtin_table_has_verified_values_and_call_identity() {
    for (expression, function, expected) in [
        ("sin(0)", Builtin::Sin, 0.0),
        ("cos(0)", Builtin::Cos, 1.0),
        ("tan(0)", Builtin::Tan, 0.0),
        ("exp(0)", Builtin::Exp, 1.0),
        ("ln(exponentiale)", Builtin::Ln, 1.0),
        ("log(exponentiale)", Builtin::Ln, 1.0),
        ("log10(100)", Builtin::Log10, 2.0),
        ("log(2,8)", Builtin::Log, 3.0),
        ("sqrt(9)", Builtin::Sqrt, 3.0),
        ("abs(-4)", Builtin::Abs, 4.0),
        ("floor(-1.5)", Builtin::Floor, -2.0),
        ("ceil(-1.5)", Builtin::Ceiling, -1.0),
        ("ceiling(1.5)", Builtin::Ceiling, 2.0),
        ("pow(2,3)", Builtin::Power, 8.0),
        ("power(2,3)", Builtin::Power, 8.0),
        ("min(3,-1,2)", Builtin::Min, -1.0),
        ("max(3,-1,2)", Builtin::Max, 3.0),
    ] {
        let model = compile(&format!("answer={expression};")).unwrap();
        assert_eq!(value(&model, "answer"), Some(expected), "{expression}");
        let root = model.initial_assignments()[0].expression();
        assert!(
            matches!(model.expression(root).unwrap().kind(), ExpressionKind::Call { function: f, .. } if *f == function)
        );
    }
}
#[test]
fn constants_are_typed_not_inferred_parameters() {
    let model = compile("a=pi;b=exponentiale;e=2;").unwrap();
    assert_eq!(model.parameters().len(), 3);
    for (id, constant) in [
        ("a", NumericConstant::Pi),
        ("b", NumericConstant::ExponentialE),
    ] {
        let parameter = model.parameters().iter().find(|p| p.id() == id).unwrap();
        assert_eq!(parameter.value(), Some(constant.value()));
        assert!(
            matches!(model.expression(parameter.initial_expression().unwrap()).unwrap().kind(), ExpressionKind::Constant(c) if *c == constant)
        );
    }
}
#[test]
fn time_and_dependencies_are_symbolic_not_guessed_zero() {
    let model = compile("a=time;b=time();c=sin(a);d=max(c,2);").unwrap();
    for p in model.parameters() {
        assert_eq!(p.value(), None);
        assert!(p.constant());
    }
    assert!(matches!(
        model
            .expression(model.parameters()[0].initial_expression().unwrap())
            .unwrap()
            .kind(),
        ExpressionKind::Time
    ));
}
#[test]
fn unknown_argument_preserves_none_but_known_invalid_siblings_reject() {
    let model = compile("a=min(u,2);b=log(u,2);c=pow(u,2);").unwrap();
    for p in model.parameters() {
        assert_eq!(p.value(), None);
    }
    for source in [
        "a=min(u,sqrt(-1));",
        "a=log(1,u);",
        "a=log(u,-1);",
        "a=exp(1000)+u;",
        "a=sin(u)+1/0;",
    ] {
        assert_eq!(
            compile(source).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict,
            "{source}"
        );
    }
}
#[test]
fn finite_real_initial_domain_errors_are_deterministic() {
    for expression in [
        "sqrt(-1)",
        "ln(0)",
        "log(-1)",
        "log10(0)",
        "log(-2,1)",
        "log(1,2)",
        "pow(-2,0.5)",
        "power(0,0)",
        "power(0,-1)",
        "exp(1000)",
        "pow(1e200,2)",
    ] {
        let source = format!("a={expression};");
        let first = compile(&source).unwrap_err();
        let second = compile(&source).unwrap_err();
        assert_eq!(
            first.code(),
            DiagnosticCode::DeclarationConflict,
            "{source}"
        );
        assert_eq!(first.message(), second.message());
        assert_eq!(first.labels()[0].span(), second.labels()[0].span());
    }
}
#[test]
fn runtime_rate_singularities_are_not_claimed_statically_eliminated() {
    let model = compile("J:=>S;ln(time)+sqrt(-1)+pow(0,-1);").unwrap();
    assert_eq!(model.reactions().len(), 1);
}
#[test]
fn arity_errors_are_distinct_from_bounded_unsupported_calls() {
    // Previously deferred inputs are explicit newly-supported checks, not removed coverage.
    compile("a=piecewise(1,true,2);").unwrap();
    compile("a=true;").unwrap();
    for expression in [
        "sin()",
        "cos(1,2)",
        "log()",
        "log(1,2,3)",
        "ln(2,3)",
        "log10(2,3)",
        "sqrt(1,2)",
        "pow(2)",
        "power(2,3,4)",
        "time(1)",
        "pi()",
        "exponentiale()",
        "sin(1,)",
        "sin",
    ] {
        assert_eq!(
            compile(&format!("a={expression};")).unwrap_err().code(),
            DiagnosticCode::Syntax,
            "{expression}"
        );
    }
    for expression in [
        "min()",
        "max()",
        "root(3,8)",
        "delay(1,2)",
        "custom(2)",
        "Sin(2)",
    ] {
        assert_eq!(
            compile(&format!("a={expression};")).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{expression}"
        );
    }
}
#[test]
fn all_builtin_names_remain_reserved_in_declarations() {
    for name in [
        "sin",
        "cos",
        "tan",
        "exp",
        "ln",
        "log",
        "log10",
        "sqrt",
        "abs",
        "floor",
        "ceil",
        "ceiling",
        "pow",
        "power",
        "min",
        "max",
        "pi",
        "exponentiale",
        "time",
    ] {
        for source in [
            format!("{name}=1;"),
            format!("species {name};"),
            format!("model {name}()\na=1;end"),
        ] {
            assert_eq!(
                compile(&source).unwrap_err().code(),
                DiagnosticCode::UnsupportedFeature,
                "{source}"
            );
        }
    }
}
#[test]
fn calls_preserve_argument_order_and_contextual_arithmetic_groups() {
    let model =
        compile("a=abs((1e16+(-1e16+1))+1);b=abs(1e16+(-1e16+1));c=max(a,min(b,4));").unwrap();
    assert_eq!(value(&model, "a"), Some(1.0));
    assert_eq!(value(&model, "b"), Some(1.0));
    assert_eq!(value(&model, "c"), Some(1.0));
    let model = compile("a=log(2,8);").unwrap();
    let ExpressionKind::Call { args, .. } = model
        .expression(model.initial_assignments()[0].expression())
        .unwrap()
        .kind()
    else {
        panic!()
    };
    assert!(matches!(
        model.expression(args[0]).unwrap().kind(),
        ExpressionKind::Number { value: 2.0, .. }
    ));
    assert!(matches!(
        model.expression(args[1]).unwrap().kind(),
        ExpressionKind::Number { value: 8.0, .. }
    ));
}
#[test]
fn nested_calls_participate_in_dependency_cycles_and_species_modifiers() {
    assert_eq!(
        compile("a=sin(b);b=cos(a);").unwrap_err().code(),
        DiagnosticCode::InitializationCycle
    );
    let model = compile("species E;E=2;J:S=>P;sin(E)*S;a=cos(E);").unwrap();
    assert_eq!(model.reactions()[0].modifiers(), &["E"]);
    assert!(
        !model
            .parameters()
            .iter()
            .find(|p| p.id() == "a")
            .unwrap()
            .constant()
    );
}
#[test]
fn source_formatter_preserves_spelling_and_is_idempotent() {
    let limits = Limits::default();
    let text = "a= log ( 2, max(3,4) ) ; b=ceil(- -2);c=time();";
    let source = SourceFile::new(SourceId::new(330), "format", text, &limits).unwrap();
    let once = format_antimony(&parse(&source, &limits).unwrap()).unwrap();
    assert_eq!(
        once,
        "a = log(2, max(3, 4));\nb = ceil(- -2);\nc = time();\n"
    );
    let source = SourceFile::new(SourceId::new(330), "format", &once, &limits).unwrap();
    assert_eq!(
        format_antimony(&parse(&source, &limits).unwrap()).unwrap(),
        once
    );
}
#[test]
fn known_dimensional_inputs_cannot_enter_transcendental_functions() {
    for expression in [
        "sin(x)",
        "cos(x)",
        "tan(x)",
        "exp(x)",
        "ln(x)",
        "log10(x)",
        "log(x,u)",
        "pow(u,x)",
        "min(x,y,u)",
    ] {
        let source = format!("x=2;x has second;y=3;y has metre;a={expression};");
        assert_eq!(
            compile(&source).unwrap_err().code(),
            DiagnosticCode::UnitsInconsistency,
            "{expression}"
        );
    }
}
#[test]
fn dimensions_preserve_unknowns_and_explicit_constant_or_time_knowledge() {
    for (source, expected_known) in [
        ("a=sin(2);", false),
        ("a=sin(u);", false),
        ("a=sin(pi);", true),
        ("a=time;", false),
        ("unit time_unit=second;a=time;", true),
        ("x=2;x has second;a=abs(x);", true),
        ("unit square=metre^2;x=4;x has square;a=sqrt(x);", true),
        ("x=4;x has metre;a=sqrt(x);", false),
        ("x=2;x has second;a=min(x,u);", false),
    ] {
        let model = compile(source).unwrap();
        let p = model.parameters().iter().find(|p| p.id() == "a").unwrap();
        assert_eq!(
            matches!(
                model.expression_dimensions(p.initial_expression().unwrap()),
                Some(DimensionalStatus::Known(_))
            ),
            expected_known,
            "{source}"
        );
    }
}
#[test]
fn deep_calls_fail_closed_and_wide_calls_are_not_recursive() {
    let text = format!("a={}1{};", "sin(".repeat(2000), ")".repeat(2000));
    assert_eq!(
        compile(&text).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 10000);
    assert_eq!(
        compile_with(&text, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let text = format!("a=max({});", vec!["1"; 10000].join(","));
    assert_eq!(value(&compile(&text).unwrap(), "a"), Some(1.0));
    let limits = Limits::default().with_limit(LimitKind::Tokens, 100);
    assert_eq!(
        compile_with(&text, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}
#[cfg(feature = "sbml")]
#[test]
fn sbml_encodes_constants_time_root_degree_log_base_and_aliases() {
    let model = compile(
        "a=pi;b=exponentiale;c=time();d=log(2,8);f=log10(100);g=sqrt(4);h=ceil(2);i=pow(2,3);",
    )
    .unwrap();
    let xml = antimony::write_sbml(&model).unwrap();
    for fragment in [
        "<pi/>",
        "<exponentiale/>",
        "definitionURL=\"http://www.sbml.org/sbml/symbols/time\"",
        "<logbase><cn type=\"integer\">2</cn></logbase>",
        "<logbase><cn type=\"integer\">10</cn></logbase>",
        "<degree><cn type=\"integer\">2</cn></degree>",
        "<ceiling/>",
        "<power/>",
    ] {
        assert!(xml.xml().contains(fragment), "{fragment}");
    }
    assert!(!xml.xml().contains("<listOfRules"));
    assert!(!xml.xml().contains("id=\"time\""));
}

#[test]
fn time_uses_actual_model_default_dimensions_including_dimensionless() {
    let model = compile("unit time_unit=dimensionless;a=sin(time);").unwrap();
    let root = model.parameters()[0].initial_expression().unwrap();
    let Some(DimensionalStatus::Known(d)) = model.expression_dimensions(root) else {
        panic!("explicit dimensionless time must stay dimensionless")
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 0));
    assert_eq!(value(&model, "a"), None);
    assert_eq!(
        compile("unit time_unit=second;a=sin(time);")
            .unwrap_err()
            .code(),
        DiagnosticCode::UnitsInconsistency
    );
}
#[test]
fn iterative_calls_retain_inclusive_configured_and_hard_depth_bounds() {
    for depth in [2, 32, 128, 256] {
        let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, depth);
        let inside = format!("a={}1{};", "abs(".repeat(depth - 1), ")".repeat(depth - 1));
        assert_eq!(
            value(&compile_with(&inside, &limits).unwrap(), "a"),
            Some(1.0)
        );
        let outside = format!("a={}1{};", "abs(".repeat(depth), ")".repeat(depth));
        assert_eq!(
            compile_with(&outside, &limits).unwrap_err().code(),
            DiagnosticCode::ResourceLimit
        );
    }
}
