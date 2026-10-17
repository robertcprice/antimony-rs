//! Initialization-time values and retained formulas are tested independently.
use antimony::{
    BinaryOp, Diagnostic, DiagnosticCode, ExpressionKind, LimitKind, Limits, SourceFile, SourceId,
    ValidatedModel, analyze, parse,
};

fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(
        SourceId::new(17),
        "initialization",
        text,
        &Limits::default(),
    )
    .unwrap();
    analyze(&parse(&source, limits)?)
}
fn parameter(model: &ValidatedModel, name: &str) -> f64 {
    model
        .parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
        .expect("explicit initialized test model")
}

#[test]
fn arithmetic_evaluates_but_retains_original_tree() {
    let model = compile("a=2+3*4;").unwrap();
    assert_eq!(parameter(&model, "a"), 14.0);
    let assignment = &model.initial_assignments()[0];
    assert_eq!(assignment.symbol(), "a");
    assert_eq!(
        model.parameters()[0].initial_expression(),
        Some(assignment.expression())
    );
    assert!(matches!(
        model.expression(assignment.expression()).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn forward_references_and_declarations_resolve_before_evaluation() {
    for text in ["c=b+1;b=a*2;formula a=3;", "formula a=3;b=a*2;c=b+1;"] {
        let model = compile(text).unwrap();
        assert_eq!(parameter(&model, "c"), 7.0);
        assert_eq!(model.initial_assignments().len(), 2);
    }
}

#[test]
fn wrapped_and_plus_literals_are_assignments_but_negative_literal_is_scalar() {
    let model = compile("a=-2;b=+2;c=(-2);d=(3);e=2;").unwrap();
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|a| a.symbol())
            .collect::<Vec<_>>(),
        ["b", "c", "d"]
    );
    assert_eq!(parameter(&model, "a"), -2.0);
    assert_eq!(parameter(&model, "c"), -2.0);
}

#[test]
fn species_initialization_uses_concentration_not_amount() {
    let model = compile("compartment c=3;species S in c,P in c;S=p*2;P=S/c;p=4;").unwrap();
    assert_eq!(
        model
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .initial_concentration()
            .expect("explicit initialized test model"),
        8.0
    );
    assert_eq!(
        model
            .species()
            .iter()
            .find(|s| s.id() == "P")
            .unwrap()
            .initial_concentration()
            .expect("explicit initialized test model"),
        8.0 / 3.0
    );
    assert!(
        model
            .species()
            .iter()
            .all(|s| s.initial_expression().is_some())
    );
}

#[test]
fn parameter_constant_flags_follow_transitive_species_dependencies() {
    for (declaration, expected) in [
        ("species S", false),
        ("const species S", true),
        ("species $S", true),
    ] {
        let model = compile(&format!("{declaration};S=2;a=S*3;b=a*2;")).unwrap();
        assert!(model.parameters().iter().all(|p| p.constant() == expected));
        assert_eq!(parameter(&model, "b"), 12.0);
    }
    let model = compile("compartment c=3;a=c*2;b=a+1;").unwrap();
    assert!(model.parameters().iter().all(|p| p.constant()));
}

#[test]
fn assignment_order_is_species_then_parameters_and_first_occurrence_within_type() {
    for (text, expected) in [
        ("a=2+3;species S;S=a*2;", vec!["S", "a"]),
        ("species B,A;A=p+1;B=p+2;p=3;", vec!["B", "A"]),
        ("J:S=>P;k*S;P=a+1;S=a+2;k=a/2;a=2;", vec!["S", "P", "k"]),
        ("z=a*2;a=b+1;b=3;", vec!["z", "a"]),
    ] {
        assert_eq!(
            compile(text)
                .unwrap()
                .initial_assignments()
                .iter()
                .map(|a| a.symbol())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn repeated_references_are_one_dependency_not_false_cycle() {
    let model = compile("a=b+b-b;b=2;").unwrap();
    assert_eq!(parameter(&model, "a"), 2.0);
}

#[test]
fn unknown_and_uninitialized_references_never_default_to_zero() {
    for text in ["a=unknown*2;", "formula b;a=b+1;", "species S;a=S*2;"] {
        let model = compile(text).unwrap();
        assert!(model.parameters().iter().all(|p| p.value().is_none()));
        assert_eq!(model.initial_assignments().len(), 1);
    }
}

#[test]
fn cycles_report_located_initialization_diagnostic() {
    for text in [
        "a=a+1;",
        "a=b+1;b=a+1;",
        "species S;S=a*2;a=S+1;",
        "z=a+1;a=b+1;b=a+1;",
    ] {
        let error = compile(text).unwrap_err();
        assert_eq!(error.code(), DiagnosticCode::InitializationCycle);
        assert!(!error.labels().is_empty());
        for label in error.labels() {
            assert!(text.get(label.span().range()).is_some());
        }
    }
}

#[test]
fn cyclic_reaction_network_is_not_an_initialization_cycle() {
    let model = compile("S=p*2;P=p+1;p=1;J:S=>P;S;K:P=>S;P;").unwrap();
    assert_eq!(model.reactions().len(), 2);
    assert_eq!(model.initial_assignments().len(), 2);
}

#[test]
fn finite_real_domain_is_checked_at_each_operation() {
    for text in [
        "a=1/0;",
        "a=0/0;",
        "a=(-2)^0.5;",
        "a=0^-1;",
        "a=0^0;",
        "a=10^400;",
        "a=(10^400)/10;",
        "b=0;a=1/b;",
    ] {
        let error = compile(text).unwrap_err();
        assert_eq!(error.code(), DiagnosticCode::DeclarationConflict, "{text}");
        assert!(!error.labels().is_empty());
    }
    for (text, expected) in [
        ("a=(-2)^3;", -8.0),
        ("a=2^-2;", 0.25),
        ("a=-2^2;", -4.0),
        ("a=2^3^2;", 512.0),
    ] {
        assert_eq!(parameter(&compile(text).unwrap(), "a"), expected);
    }
}

#[test]
fn negative_species_results_are_rejected_but_negative_parameters_are_legal() {
    assert_eq!(
        compile("species S;S=1-2;").unwrap_err().code(),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(parameter(&compile("a=1-2;").unwrap(), "a"), -1.0);
}

#[test]
fn compartment_expressions_remain_explicitly_deferred() {
    for (text, expected) in [
        ("compartment c=2+3;", 5.0),
        ("compartment c=p*2;p=1;", 2.0),
        ("compartment c=(2);", 2.0),
        ("compartment c=+2;", 2.0),
    ] {
        let model = compile(text).unwrap();
        let comp = model.compartments().iter().find(|c| c.id() == "c").unwrap();
        assert_eq!(comp.size(), Some(expected), "{text}");
        assert!(comp.initial_expression().is_some(), "{text}");
    }
}

#[test]
fn remaining_expression_restrictions_apply_to_initializers() {
    assert_eq!(
        parameter(&compile("a=piecewise(1,true,0);").unwrap(), "a"),
        1.0
    );
    for text in ["a=delay(2,1);", "a=1.1234567890123456+1;"] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{text}"
        );
    }
    // Literal numeric initialization retains its previous wider finite envelope.
    assert_eq!(parameter(&compile("a=1e-200;").unwrap(), "a"), 1e-200);
}

#[test]
fn reaction_references_still_fail_closed() {
    {
        let text = "S=1;J:S=>;S;a=J+1;";
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature
        );
    }
}

#[test]
fn long_chains_and_cycles_use_no_recursive_dependency_traversal() {
    let mut text = String::new();
    for i in 0..10_000 {
        text.push_str(&format!("x{i}=x{}+1;", i + 1));
    }
    text.push_str("x10000=1;");
    assert_eq!(parameter(&compile(&text).unwrap(), "x0"), 10001.0);
    let text = text.replace("x10000=1;", "x10000=x0+1;");
    assert_eq!(
        compile(&text).unwrap_err().code(),
        DiagnosticCode::InitializationCycle
    );
}

#[test]
fn initializer_expression_and_entity_limits_are_enforced() {
    for (text, limits) in [
        (
            "a=2+3;",
            Limits::default().with_limit(LimitKind::ExpressionDepth, 1),
        ),
        (
            "a=b+1;b=2;",
            Limits::default().with_limit(LimitKind::ModelEntities, 1),
        ),
        (
            "a=2+3;",
            Limits::default().with_limit(LimitKind::Diagnostics, 0),
        ),
    ] {
        assert_eq!(
            compile_with(text, &limits).unwrap_err().code(),
            DiagnosticCode::ResourceLimit
        );
    }
    let text = format!("a={}2{};", "(".repeat(5000), ")".repeat(5000));
    assert_eq!(
        compile(&text).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[cfg(feature = "sbml")]
#[test]
fn export_preserves_assignments_without_scalar_attrs_or_rules() {
    let model = compile("compartment c=3;species S in c;S=p*2;p=2+3;a=S+1;").unwrap();
    let xml = antimony::write_sbml(&model).unwrap();
    assert!(!xml.xml().contains("initialConcentration="));
    assert!(!xml.xml().contains("initialAmount="));
    assert!(!xml.xml().contains("value="));
    assert!(!xml.xml().contains("listOfRules"));
    assert!(
        xml.xml()
            .contains("<parameter id=\"a\" constant=\"false\"/>")
    );
    assert!(xml.xml().contains("<initialAssignment symbol=\"S\"><math"));
    assert!(
        xml.xml().contains(
            "<apply><plus/><cn type=\"integer\">2</cn><cn type=\"integer\">3</cn></apply>"
        )
    );
    assert_eq!(xml.xml(), antimony::write_sbml(&model).unwrap().xml());
}
