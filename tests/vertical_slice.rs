//! Native numeric-slice tests; reference success remains a separate external gate.
use antimony::{
    BinaryOp, Diagnostic, DiagnosticCode, ExpressionKind, LimitKind, Limits, SourceFile, SourceId,
    UnaryOp, ValidatedModel, analyze, parse,
};

fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    // Source is admitted under defaults to test parse's own smaller source guard.
    let source = SourceFile::new(SourceId::new(9), "test", text, &Limits::default()).unwrap();
    analyze(&parse(&source, limits)?)
}
const ACCEPTED: [&str; 7] = [
    include_str!("../fixtures/accepted/first_conversion.ant"),
    include_str!("../fixtures/accepted/arrow_irreversible.ant"),
    include_str!("../fixtures/accepted/arrow_reversible.ant"),
    include_str!("../fixtures/accepted/boundary_const.ant"),
    include_str!("../fixtures/accepted/boundary_dollar.ant"),
    include_str!("../fixtures/accepted/nonunit_volume.ant"),
    include_str!("../fixtures/accepted/precedence.ant"),
];

#[test]
fn seven_curated_inputs_compile_natively() {
    for text in ACCEPTED {
        let model = compile(text).unwrap();
        assert_eq!(model.profile_id(), "modular-core-v0.8-slice");
        assert_eq!(model.sbml_target(), (3, 2));
        assert_eq!(model.reactions().len(), 1);
        assert_eq!(model.species().len(), 2);
        assert!(
            model
                .warnings()
                .iter()
                .any(|d| d.code() == DiagnosticCode::UnspecifiedUnits)
        );
    }
}
#[test]
fn first_conversion_preserves_explicit_values_and_compartment() {
    let model = compile(ACCEPTED[0]).unwrap();
    assert_eq!(model.id(), "first_conversion");
    assert_eq!(model.compartments()[0].id(), "cell");
    assert_eq!(
        model.compartments()[0]
            .size()
            .expect("explicit initialized test model"),
        1.0
    );
    assert!(!model.compartments()[0].is_implicit());
    assert_eq!(model.species()[0].id(), "A");
    assert_eq!(
        model.species()[0]
            .initial_concentration()
            .expect("explicit initialized test model"),
        12.0
    );
    assert_eq!(
        model.parameters()[0]
            .value()
            .expect("explicit initialized test model"),
        0.2
    );
    assert_eq!(model.reactions()[0].id(), "J0");
    assert_eq!(model.source().id(), SourceId::new(9));
}
#[test]
fn arrows_have_upstream_not_conventional_meaning() {
    assert!(!compile(ACCEPTED[1]).unwrap().reactions()[0].reversible());
    assert!(compile(ACCEPTED[2]).unwrap().reactions()[0].reversible());
}
#[test]
fn default_compartment_is_reported_not_silently_guessed() {
    let model = compile(ACCEPTED[1]).unwrap();
    assert_eq!(model.id(), "__main");
    let c = &model.compartments()[0];
    assert_eq!(c.id(), "default_compartment");
    assert_eq!(c.size().expect("explicit initialized test model"), 1.0);
    assert!(c.is_implicit());
    assert!(
        model
            .warnings()
            .iter()
            .any(|d| d.code() == DiagnosticCode::DefaultCompartment)
    );
}
#[test]
fn const_and_dollar_species_are_boundary_not_constant() {
    for text in [ACCEPTED[3], ACCEPTED[4]] {
        let model = compile(text).unwrap();
        let s = model.species().iter().find(|s| s.id() == "S").unwrap();
        assert!(s.boundary_condition());
        assert!(!s.constant());
    }
}
#[test]
fn nonunit_volume_does_not_change_concentration_or_rate_tree() {
    let model = compile(ACCEPTED[5]).unwrap();
    assert_eq!(
        model.compartments()[0]
            .size()
            .expect("explicit initialized test model"),
        3.0
    );
    assert_eq!(
        model
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .initial_concentration()
            .expect("explicit initialized test model"),
        2.0
    );
    let root = model.expression(model.reactions()[0].rate()).unwrap();
    assert!(matches!(
        root.kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Multiply,
            ..
        }
    ));
}
#[test]
fn power_is_right_associative_and_above_unary_minus() {
    let model = compile("S=1; P=0; J:S=>P; -S^2+2^3^2;").unwrap();
    let ExpressionKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = model
        .expression(model.reactions()[0].rate())
        .unwrap()
        .kind()
    else {
        panic!("addition root");
    };
    let ExpressionKind::Unary {
        op: UnaryOp::Minus,
        child,
    } = model.expression(*left).unwrap().kind()
    else {
        panic!("minus after power");
    };
    assert!(matches!(
        model.expression(*child).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Power,
            ..
        }
    ));
    let ExpressionKind::Binary {
        op: BinaryOp::Power,
        right,
        ..
    } = model.expression(*right).unwrap().kind()
    else {
        panic!("outer power");
    };
    assert!(matches!(
        model.expression(*right).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Power,
            ..
        }
    ));
}
#[test]
fn ordinary_multiplication_and_subtraction_are_left_associative() {
    let model = compile("S=1; P=0; J:S=>P; 8/2/2-3-1;").unwrap();
    let ExpressionKind::Binary {
        op: BinaryOp::Subtract,
        left,
        ..
    } = model
        .expression(model.reactions()[0].rate())
        .unwrap()
        .kind()
    else {
        panic!();
    };
    let ExpressionKind::Binary {
        op: BinaryOp::Subtract,
        left,
        ..
    } = model.expression(*left).unwrap().kind()
    else {
        panic!();
    };
    let ExpressionKind::Binary {
        op: BinaryOp::Divide,
        left,
        ..
    } = model.expression(*left).unwrap().kind()
    else {
        panic!();
    };
    assert!(matches!(
        model.expression(*left).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Divide,
            ..
        }
    ));
}
#[test]
fn forward_declarations_resolve_before_inference() {
    let model =
        compile("J:S=>P; k*S*c; P=0; S=1; k=2; species S in c, P in c; compartment c=3;").unwrap();
    assert_eq!(model.parameters().len(), 1);
    assert_eq!(model.species().len(), 2);
    assert!(!model.compartments()[0].is_implicit());
}
#[test]
fn modifier_species_do_not_become_reactants() {
    let model = compile("species E; E=2; S=1; P=0; J:S=>P; E*S;").unwrap();
    assert_eq!(model.reactions()[0].modifiers(), &["E"]);
    assert_eq!(model.reactions()[0].reactants().len(), 1);
}
#[test]
fn source_sink_and_fractional_stoichiometry_are_explicit() {
    let model = compile("S=1; J: => 0.5 S; 2; K:2 S => ; S;").unwrap();
    assert!(model.reactions()[0].reactants().is_empty());
    assert_eq!(model.reactions()[0].products()[0].stoichiometry(), 0.5);
    assert!(model.reactions()[1].products().is_empty());
    assert_eq!(model.reactions()[1].reactants()[0].stoichiometry(), 2.0);
}
#[test]
fn comments_separators_scientific_numbers_and_utf8_comment_spans() {
    let model = compile("// β comment\r\nS=1e-2\nP=0; /*😀*/ k=-2.5E+1; J:S=>P; k*S;").unwrap();
    assert_eq!(
        model.parameters()[0]
            .value()
            .expect("explicit initialized test model"),
        -25.0
    );
    assert_eq!(
        model
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .initial_concentration()
            .expect("explicit initialized test model"),
        0.01
    );
}
#[test]
fn absent_values_are_preserved_never_zero() {
    for text in [
        include_str!("../fixtures/rejected/absent_values.ant"),
        "S=1; P=0; J:S=>P; missing*S;",
    ] {
        let model = compile(text).unwrap();
        assert!(
            model.parameters().iter().any(|p| p.value().is_none())
                || model
                    .species()
                    .iter()
                    .any(|s| s.initial_concentration().is_none())
        );
    }
}
#[test]
fn valid_unsupported_corpus_is_not_called_malformed() {
    // Historical sin, assignment_rule, and event fixtures are deliberately newly supported.
    assert!(compile(include_str!("../fixtures/rejected/function_call.ant")).is_ok());
    assert!(compile(include_str!("../fixtures/rejected/assignment_rule.ant")).is_ok());
    assert!(compile(include_str!("../fixtures/rejected/event.ant")).is_ok());
    assert_eq!(
        compile("0 = S + 1;").unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );
}
#[test]
fn malformed_corpus_is_a_syntax_error() {
    assert_eq!(
        compile(include_str!("../fixtures/rejected/malformed.ant"))
            .unwrap_err()
            .code(),
        DiagnosticCode::Syntax
    );
}
#[test]
fn unsupported_language_is_not_silently_dropped() {
    assert!(compile("model a()\nS=1;\nend\nmodel b()\nP=1;\nend").is_ok());
    for text in [
        "import \"/etc/passwd\";",
        "S=1; S'=;",
        "S=1; S is \"label\";",
        "S=1; S.sboTerm=1;",
        "S=1; P=0; J:S<=>P; S;",
    ] {
        assert!(compile(text).is_err(), "must reject: {text}");
    }
}
#[test]
fn unsupported_initialization_forms_are_explicit() {
    assert!(compile("compartment c=2+3;").is_ok());
    assert!(compile("compartment c=(2);").is_ok());
    assert_eq!(
        compile("k=delay(2,1);").unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );
}
#[test]
fn repeated_initialization_is_last_wins_and_conflicting_roles_reject() {
    assert_eq!(
        compile("k=1;k=2;").unwrap().parameters()[0].value(),
        Some(2.0)
    );
    for text in [
        "species S; compartment S=1;",
        "compartment S=1; P=0; J:S=>P; S;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict
        );
    }
}
#[test]
fn numeric_domains_are_checked() {
    for text in [
        "compartment c=0;",
        "compartment c=-1;",
        "k=1e999;",
        "S=-1; P=0; J:S=>P; S;",
        "S=1; J:0 S=>; S;",
        "S=1; J:S=>; 1e999;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict,
            "{text}"
        );
    }
}
#[test]
fn inferred_explicit_compartment_preserves_unset_size() {
    let model = compile("species S in missing;S=1;").unwrap();
    assert_eq!(model.compartments()[0].size(), None);
    assert!(!model.compartments()[0].is_implicit());
}
#[test]
fn reserved_constants_and_functions_are_not_ordinary_parameters() {
    for name in [
        "pi",
        "exponentiale",
        "true",
        "False",
        "TRUE",
        "NAN",
        "inf",
        "infinity",
        "avogadro",
        "time",
        "sin",
        "pow",
        "floor",
        "sqrt",
        "log",
        "ln",
        "delay",
        "rateOf",
        "piecewise",
        "and",
        "or",
        "not",
        "xor",
        "truncatedNormal",
    ] {
        for text in [
            format!("{name}=3;"),
            format!("species {name};"),
            format!("model {name}()\nk=1;\nend"),
        ] {
            assert_eq!(
                compile(&text).unwrap_err().code(),
                DiagnosticCode::UnsupportedFeature,
                "{text}"
            );
        }
    }
    for name in ["pi", "exponentiale", "time"] {
        assert!(compile(&format!("S=1; J:S=>; {name}*S;")).is_ok());
    }
    for name in ["sin", "pow", "floor", "sqrt", "log", "ln"] {
        assert_eq!(
            compile(&format!("a={name};")).unwrap_err().code(),
            DiagnosticCode::Syntax
        );
    }
    assert_eq!(compile("e=2;").unwrap().parameters()[0].id(), "e");
}
#[test]
fn all_resource_guards_are_live() {
    for (kind, maximum) in [
        (LimitKind::SourceBytes, 1),
        (LimitKind::Tokens, 2),
        (LimitKind::ModelEntities, 1),
        (LimitKind::Diagnostics, 0),
        (LimitKind::ExpressionDepth, 1),
    ] {
        let limits = Limits::default().with_limit(kind, maximum);
        assert_eq!(
            compile_with(ACCEPTED[0], &limits).unwrap_err().code(),
            DiagnosticCode::ResourceLimit,
            "{kind:?}"
        );
    }
}
#[test]
fn deep_nesting_and_long_left_chains_fail_without_recursive_drop() {
    for rate in [
        format!("{}S{}", "(".repeat(5000), ")".repeat(5000)),
        std::iter::repeat_n("S", 5000).collect::<Vec<_>>().join("+"),
    ] {
        let text = format!("S=1; J:S=>; {rate};");
        assert_eq!(
            compile(&text).unwrap_err().code(),
            DiagnosticCode::ResourceLimit
        );
    }
    let text = format!("S=1; J:S=>; {}S{};", "(".repeat(300), ")".repeat(300));
    assert_eq!(
        compile_with(
            &text,
            &Limits::default().with_limit(LimitKind::ExpressionDepth, usize::MAX)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
}
#[test]
fn malformed_utf8_boundary_context_returns_located_errors() {
    for text in [
        "S=1; β=2;",
        "S=1; /* unterminated 😀",
        "S=1e+;",
        "model m()\nS=1;",
    ] {
        let d = compile(text).unwrap_err();
        assert_eq!(d.code(), DiagnosticCode::Syntax);
        for label in d.labels() {
            assert!(text.get(label.span().range()).is_some());
        }
    }
}
#[test]
fn adversarial_short_inputs_do_not_panic() {
    let alphabet = ['A', '0', ':', ';', '=', '(', ')', '$', '+', '^', '\n', 'β'];
    for a in alphabet {
        for b in alphabet {
            for c in alphabet {
                let _ = compile(&format!("{a}{b}{c}"));
            }
        }
    }
}
#[test]
fn deterministic_and_thread_independent_compilation() {
    let handles = (0..4)
        .map(|_| {
            std::thread::spawn(|| {
                let model = compile(ACCEPTED[0]).unwrap();
                (
                    model.id().to_owned(),
                    model.parameters()[0]
                        .value()
                        .expect("explicit initialized test model"),
                    model.species()[0]
                        .initial_concentration()
                        .expect("explicit initialized test model"),
                )
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        assert_eq!(
            handle.join().unwrap(),
            ("first_conversion".into(), 0.2, 12.0)
        );
    }
}
#[cfg(feature = "sbml")]
#[test]
fn structured_export_preserves_notices_and_is_deterministic() {
    let model = compile(ACCEPTED[0]).unwrap();
    let one = antimony::write_sbml(&model).unwrap();
    let two = antimony::write_sbml(&model).unwrap();
    assert_eq!(one.xml(), two.xml());
    assert_eq!(one.profile_id(), model.profile_id());
    assert_eq!(one.warnings(), model.warnings());
    assert!(one.xml().contains("level=\"3\" version=\"2\""));
    assert!(one.xml().contains("initialConcentration=\"12\""));
    assert!(!one.xml().contains("initialAmount"));
    assert!(!one.xml().contains("<initialAssignment"));
}
#[cfg(feature = "sbml")]
#[test]
fn mathml_preserves_numeric_values_and_binary_grouping() {
    let model = compile("S=1; J:S=>; 2.5e-3*S+2^3^2;").unwrap();
    let doc = antimony::write_sbml(&model).unwrap();
    assert!(doc.xml().contains("<cn type=\"real\">0.0025</cn>"));
    assert!(
        doc.xml()
            .contains("<power/><cn type=\"integer\">2</cn><apply><power/>")
    );
}

#[test]
fn remaining_unsupported_forms_fail_closed() {
    for text in [
        "default_compartment=3;",
        "S=1; J:S=>; 9223372036854775808*S;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{text}"
        );
    }
}
#[test]
fn reaction_requires_semicolon_and_unspaced_source_arrow_works() {
    assert_eq!(
        compile("S=1; P=0; J:S=>P\nS;").unwrap_err().code(),
        DiagnosticCode::Syntax
    );
    assert!(compile("S=1;J:=>S;2;").is_ok());
}
#[cfg(feature = "sbml")]
#[test]
fn reference_numeric_serialization_mapping_is_local_and_explicit() {
    for (rate, expected) in [
        ("1.*S", "<cn type=\"integer\">1</cn>"),
        ("1e3*S", "<cn type=\"integer\">1000</cn>"),
        ("1e-3*S", "<cn type=\"real\">0.001</cn>"),
        ("S^-2", "<cn type=\"integer\">-2</cn>"),
        ("S+(-2)", "<cn type=\"integer\">-2</cn>"),
    ] {
        let model = compile(&format!("S=1; J:S=>; {rate};")).unwrap();
        assert!(
            antimony::write_sbml(&model)
                .unwrap()
                .xml()
                .contains(expected),
            "{rate}"
        );
    }
}

#[test]
fn historical_nineteen_probes_with_explicit_arithmetic_increment() {
    let probes = [
        ("S", true),
        ("-S", true),
        ("+S", true),
        ("--S", false),
        ("S^-2", true),
        ("S^2^3", true),
        ("(-S)^2", true),
        ("S-(P-S)", true),
        ("S/(P/S)", true),
        ("1e-3*S", true),
        ("1E+3*S", true),
        (".5*S", true),
        ("1.*S", true),
        ("0002*S", true),
        ("9223372036854775808*S", false),
        ("S+P+S", true),
        ("S*(P*S)", true),
        ("S+(-2)", true),
        ("2^-2^3", true),
    ];
    for (rate, accepted) in probes {
        let result = compile(&format!("S=2;P=1;J:S=>P;{rate};"));
        assert_eq!(result.is_ok(), accepted, "{rate}");
        if let Err(d) = result {
            assert_eq!(
                d.code(),
                if rate == "--S" {
                    DiagnosticCode::Syntax
                } else {
                    DiagnosticCode::UnsupportedFeature
                }
            );
        }
    }
}
#[test]
fn historical_twenty_models_with_explicit_arithmetic_increment() {
    let probes = [
        ("a=1;", true),
        ("compartment c=2;", true),
        ("species S;S=1;", true),
        ("species S,P,M;S=1;P=0;M=2;J:S=>P;M*S;", true),
        ("S=1;J:=>S;2;", true),
        ("S=1;J:S=>;S;", true),
        ("S=1;P=0;J:0.5 S=>2 P;S;", true),
        ("S=1;P=0;J:S=>$P;S;", true),
        ("S=1;P=0;k=-2;J:S=>P;k*S;", true),
        ("default_compartment=3;", false),
        ("S=1;/* β😀 */ P=0; //x\nJ:S=>P;S;", true),
        ("S=1/*\n*/P=0;J:S=>P;S;", false),
        ("compartment c=3;species S in c,P;S=1;P=0;J:S=>P;S;", true),
        ("species S in c;S=1;compartment c=2;", true),
        ("J:=>;1;", false),
        ("sboTerm=2;", true),
        ("model pi()\na=1;\nend", false),
        ("S/*x*/=1;P=0;J:S=>P;S;", true),
        ("S=1;P=0;J:S=>P\nS;", false),
        ("S=1;P=0;J:S=>P;++S;", true),
    ];
    for (text, accepted) in probes {
        assert_eq!(compile(text).is_ok(), accepted, "{text}");
    }
}

#[test]
fn source_order_provenance_and_reaction_replacement_are_retained() {
    let text = "formula A; A=2; A=>B;k*A; _J0:B=>A;q*B;";
    let model = compile(text).unwrap();
    assert_eq!(
        model
            .source()
            .slice(model.species()[0].source_span())
            .unwrap(),
        "A"
    );
    assert_eq!(model.species()[0].source_span().start(), 8);
    assert_eq!(model.reactions().len(), 1);
    assert_eq!(model.reactions()[0].source_span().start(), 16);
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| p.id())
            .collect::<Vec<_>>(),
        ["k", "q"]
    );
}

#[test]
fn unset_inference_reactions_and_synthetic_law_depth_obey_limits() {
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 4);
    let text = "A=>B;k*A;"; // A, B, k, reaction, implicit compartment = five.
    let source = SourceFile::new(SourceId::new(99), "bounded", text, &limits).unwrap();
    assert_eq!(
        analyze(&parse(&source, &limits).unwrap())
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 1);
    let source = SourceFile::new(SourceId::new(99), "bounded", "A=>B;1;_J0=-2;", &limits).unwrap();
    assert_eq!(
        analyze(&parse(&source, &limits).unwrap())
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
}
