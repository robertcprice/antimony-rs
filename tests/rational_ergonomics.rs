//! Ordinary authoring integration tests for parenthesized rational atoms `(p/q)`,
//! unparenthesized direct-call rational-token syntax rejection `f(p/q)`,
//! ordinary division neighbors, unary sign folding, partial/unset models,
//! reaction rates, and canonical formatting idempotence.
use antimony::{
    BinaryOp, Builtin, Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionId, ExpressionKind,
    Limits, ParsedDocument, SourceFile, SourceId, ValidatedModel, analyze, format_antimony, parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(401), "rational_ergonomics", text, &limits).unwrap();
    parse(&source, &limits)
}

fn assert_approx(actual: Option<f64>, expected: f64) {
    let value = actual.expect("expected evaluated finite f64 value");
    assert!(
        (value - expected).abs() <= 1e-12,
        "expected {expected:.15}, got {value:.15}"
    );
}

fn check_roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let doc = parsed(text).expect("source should parse");
    let model = analyze(&doc).expect("source should analyze");
    let formatted = format_antimony(&doc).expect("source should format");

    let reparsed_source = SourceFile::new(
        SourceId::new(402),
        "rational_ergonomics_formatted",
        &formatted,
        &limits,
    )
    .unwrap();
    let reparsed = parse(&reparsed_source, &limits).expect("formatted output should parse");
    let reanalyzed = analyze(&reparsed).expect("formatted output should analyze");
    let reformatted = format_antimony(&reparsed).expect("formatted output should reformat");

    assert_eq!(reformatted, formatted, "formatter must be byte-idempotent");
    assert_eq!(model.id(), reanalyzed.id());

    let snap_species = |m: &ValidatedModel| {
        m.species()
            .iter()
            .map(|s| {
                (
                    s.id().to_owned(),
                    s.compartment().to_owned(),
                    s.initial_concentration(),
                    s.initial_expression().is_some(),
                    s.boundary_condition(),
                )
            })
            .collect::<Vec<_>>()
    };
    let snap_params = |m: &ValidatedModel| {
        m.parameters()
            .iter()
            .map(|p| {
                (
                    p.id().to_owned(),
                    p.value(),
                    p.initial_expression().is_some(),
                    p.constant(),
                )
            })
            .collect::<Vec<_>>()
    };
    let snap_rxns = |m: &ValidatedModel| {
        m.reactions()
            .iter()
            .map(|r| (r.id().to_owned(), r.reversible(), r.modifiers().to_vec()))
            .collect::<Vec<_>>()
    };
    let snap_inits = |m: &ValidatedModel| {
        m.initial_assignments()
            .iter()
            .map(|ia| {
                (
                    ia.symbol().to_owned(),
                    format!("{:?}", m.expression(ia.expression()).unwrap().kind()),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(snap_species(&model), snap_species(&reanalyzed));
    assert_eq!(snap_params(&model), snap_params(&reanalyzed));
    assert_eq!(snap_rxns(&model), snap_rxns(&reanalyzed));
    assert_eq!(snap_inits(&model), snap_inits(&reanalyzed));

    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&model).unwrap().xml(),
        antimony::write_sbml(&reanalyzed).unwrap().xml()
    );

    (model, formatted)
}

fn param_val(model: &ValidatedModel, id: &str) -> Option<f64> {
    model
        .parameters()
        .iter()
        .find(|p| p.id() == id)
        .unwrap()
        .value()
}

fn param_expr(model: &ValidatedModel, id: &str) -> ExpressionId {
    model
        .parameters()
        .iter()
        .find(|p| p.id() == id)
        .unwrap()
        .initial_expression()
        .expect("parameter should have an initial expression")
}

fn assert_rational_atom(model: &ValidatedModel, expr_id: ExpressionId) {
    let kind = model.expression(expr_id).unwrap().kind();
    assert!(
        matches!(kind, ExpressionKind::Rational { .. }),
        "expected exact rational atom rather than BinaryOp::Divide, got {kind:?}"
    );
    assert_eq!(
        model.expression_dimensions(expr_id),
        Some(DimensionalStatus::Unspecified)
    );
}

#[test]
fn parenthesized_rational_atom_vs_unparenthesized_ordinary_division_in_initializers() {
    let source = "model rational_vs_divide\n  a = (1/2);\n  b = 1/2;\n  species S_rat = (3/4), S_div = 3/4;\n  S_rat => S_div; 0.1 * S_rat;\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "a"), 0.5);
    assert_approx(param_val(&model, "b"), 0.5);

    let a_expr = param_expr(&model, "a");
    let b_expr = param_expr(&model, "b");
    assert_rational_atom(&model, a_expr);
    assert!(matches!(
        model.expression(b_expr).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Divide,
            ..
        }
    ));

    let s_rat = model.species().iter().find(|s| s.id() == "S_rat").unwrap();
    let s_div = model.species().iter().find(|s| s.id() == "S_div").unwrap();
    assert_approx(s_rat.initial_concentration(), 0.75);
    assert_approx(s_div.initial_concentration(), 0.75);
    assert_rational_atom(&model, s_rat.initial_expression().unwrap());
    assert!(matches!(
        model
            .expression(s_div.initial_expression().unwrap())
            .unwrap()
            .kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Divide,
            ..
        }
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">3<sep/>4</cn>"));
        assert!(xml.contains(
            "<apply><divide/><cn type=\"integer\">1</cn><cn type=\"integer\">2</cn></apply>"
        ));
        assert!(xml.contains(
            "<apply><divide/><cn type=\"integer\">3</cn><cn type=\"integer\">4</cn></apply>"
        ));
    }
}

#[test]
fn unreduced_rational_atoms_preserve_exact_numerator_and_denominator_without_reduction() {
    let source =
        "model unreduced_rationals\n  r_even = (2/4);\n  r_ten = (10/20);\n  r_six = (6/9);\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_approx(param_val(&model, "r_even"), 0.5);
    assert_approx(param_val(&model, "r_ten"), 0.5);
    assert_approx(param_val(&model, "r_six"), 2.0 / 3.0);

    for id in ["r_even", "r_ten", "r_six"] {
        assert_rational_atom(&model, param_expr(&model, id));
    }
    assert!(formatted.contains("r_even = (2 / 4);\n") || formatted.contains("r_even = (2/4);\n"));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<cn type=\"rational\">2<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">10<sep/>20</cn>"));
        assert!(xml.contains("<cn type=\"rational\">6<sep/>9</cn>"));
        assert!(!xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
        assert!(!xml.contains("<cn type=\"rational\">2<sep/>3</cn>"));
    }
}

#[test]
fn canonical_integral_decimal_and_scientific_spellings_form_rational_atoms() {
    let source = "model canonical_integral_rationals\n  dec_num = (1.0/2);\n  sci_num = (1e0/2);\n  both = (2.0/4e0);\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_approx(param_val(&model, "dec_num"), 0.5);
    assert_approx(param_val(&model, "sci_num"), 0.5);
    assert_approx(param_val(&model, "both"), 0.5);

    for id in ["dec_num", "sci_num", "both"] {
        assert_rational_atom(&model, param_expr(&model, id));
    }
    assert!(formatted.contains("1.0"));
    assert!(formatted.contains("1e0"));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">2<sep/>4</cn>"));
        assert!(!xml.contains("<divide/>"));
    }
}

#[test]
fn unary_minus_folds_into_rational_numerator_and_never_normalizes_denominator_sign() {
    let source = "model signed_rationals\n  neg_outer = -(1/2);\n  neg_num = (-1/2);\n  neg_den = (1/-2);\n  neg_outer_neg_den = -(1/-2);\n  neg_both = (-1/-2);\n  double_neg = - -(1/2);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "neg_outer"), -0.5);
    assert_approx(param_val(&model, "neg_num"), -0.5);
    assert_approx(param_val(&model, "neg_den"), -0.5);
    assert_approx(param_val(&model, "neg_outer_neg_den"), 0.5);
    assert_approx(param_val(&model, "neg_both"), 0.5);
    assert_approx(param_val(&model, "double_neg"), 0.5);

    for id in [
        "neg_outer",
        "neg_num",
        "neg_den",
        "neg_outer_neg_den",
        "neg_both",
        "double_neg",
    ] {
        let expr_id = param_expr(&model, id);
        assert_rational_atom(&model, expr_id);
        assert!(
            !matches!(
                model.expression(expr_id).unwrap().kind(),
                ExpressionKind::Unary { .. }
            ),
            "{id} should fold outer unary minus into rational numerator"
        );
    }

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<cn type=\"rational\">-1<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>-2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">-1<sep/>-2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
        assert!(!xml.contains("<divide/>"));
    }
}

#[test]
fn direct_single_argument_builtin_calls_with_rational_token_shape_reject_as_syntax() {
    for bad_source in [
        "a = sin(1/2);",
        "a = cos(1 / 2);",
        "a = tan(1.0/2);",
        "a = exp(1e0/2);",
        "a = ln(1/2);",
        "a = log(1/2);",
        "a = log10(1/2);",
        "a = sqrt(1/2);",
        "a = abs(-1/2);",
        "a = floor(1/-2);",
        "a = ceil(-1/-2);",
        "a = ceiling(2/4);",
        "a = min(1/2);",
        "a = max(1.0/2);",
    ] {
        let err = parsed(bad_source).expect_err(bad_source);
        assert_eq!(
            err.code(),
            DiagnosticCode::Syntax,
            "expected DiagnosticCode::Syntax for {bad_source}, got {:?}",
            err.code()
        );
    }
}

#[test]
fn parenthesized_rational_argument_inside_builtin_calls_succeeds_as_rational_atom() {
    let source = "model parenthesized_call_rationals\n  s = sin((1/2));\n  c = cos(-(1/2));\n  m = min((2/4));\n  p = pow((1/2), (3/2));\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "s"), 0.5_f64.sin());
    assert_approx(param_val(&model, "c"), (-0.5_f64).cos());
    assert_approx(param_val(&model, "m"), 0.5);
    assert_approx(param_val(&model, "p"), 0.5_f64.powf(1.5));

    let ExpressionKind::Call {
        function: Builtin::Sin,
        args: s_args,
    } = model.expression(param_expr(&model, "s")).unwrap().kind()
    else {
        panic!("expected sin call");
    };
    assert_eq!(s_args.len(), 1);
    assert_rational_atom(&model, s_args[0]);

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<apply><sin/><cn type=\"rational\">1<sep/>2</cn></apply>"));
        assert!(xml.contains("<apply><cos/><cn type=\"rational\">-1<sep/>2</cn></apply>"));
        assert!(xml.contains("<cn type=\"rational\">2<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">3<sep/>2</cn>"));
        assert!(!xml.contains("<divide/>"));
    }
}

#[test]
fn non_rational_division_neighbors_in_builtin_calls_succeed_as_ordinary_divide() {
    let source = "model valid_division_neighbors\n  g_num = sin((1)/2);\n  g_den = sin(1/(2));\n  u_plus_num = sin(+1/2);\n  u_plus_den = sin(1/+2);\n  frac_num = sin(0.5/2);\n  frac_den = sin(1/2.5);\n  add_tail = sin(1/2 + 0);\nend";
    let (model, _) = check_roundtrip(source);

    for id in ["g_num", "g_den", "u_plus_num", "u_plus_den", "add_tail"] {
        assert_approx(param_val(&model, id), 0.5_f64.sin());
    }
    assert_approx(param_val(&model, "frac_num"), 0.25_f64.sin());
    assert_approx(param_val(&model, "frac_den"), 0.4_f64.sin());

    for id in [
        "g_num",
        "g_den",
        "u_plus_num",
        "u_plus_den",
        "frac_num",
        "frac_den",
    ] {
        let ExpressionKind::Call {
            function: Builtin::Sin,
            args,
        } = model.expression(param_expr(&model, id)).unwrap().kind()
        else {
            panic!("expected sin call for {id}");
        };
        assert!(
            matches!(
                model.expression(args[0]).unwrap().kind(),
                ExpressionKind::Binary {
                    op: BinaryOp::Divide,
                    ..
                }
            ),
            "expected ordinary divide argument for {id}"
        );
    }

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<divide/>"));
        assert!(!xml.contains("type=\"rational\""));
    }
}

#[test]
fn multi_argument_builtins_accept_unparenthesized_division_arguments_as_ordinary_divide() {
    let source = "model multi_arg_divide_calls\n  p = pow(1/2, 3);\n  l = log(2, 1/2);\n  mn = min(1/2, 3/4, 5/8);\n  mx = max(1/4, 1.0/2);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "p"), 0.125);
    assert_approx(param_val(&model, "l"), -1.0);
    assert_approx(param_val(&model, "mn"), 0.5);
    assert_approx(param_val(&model, "mx"), 0.5);

    for id in ["p", "l", "mn", "mx"] {
        let ExpressionKind::Call { args, .. } =
            model.expression(param_expr(&model, id)).unwrap().kind()
        else {
            panic!("expected call expression for {id}");
        };
        assert!(args.iter().any(|&arg| matches!(
            model.expression(arg).unwrap().kind(),
            ExpressionKind::Binary {
                op: BinaryOp::Divide,
                ..
            }
        )));
    }

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<divide/>"));
        assert!(!xml.contains("type=\"rational\""));
    }
}

#[test]
fn nested_builtins_and_arithmetic_combine_rational_atoms_and_ordinary_division() {
    let source = "model nested_rational_arithmetic\n  a = sin((1/2) + 0) + cos((1)/2);\n  b = sqrt((9/4)) * pow((1/2), 2) + log(2, (1/8));\n  c = max(min((3/4), 1/2), exp(ln((1/4))));\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "a"), 0.5_f64.sin() + 0.5_f64.cos());
    assert_approx(param_val(&model, "b"), 1.5 * 0.25 - 3.0);
    assert_approx(param_val(&model, "c"), 0.5);

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">9<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>8</cn>"));
        assert!(xml.contains("<cn type=\"rational\">3<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>4</cn>"));
        assert!(xml.contains(
            "<apply><divide/><cn type=\"integer\">1</cn><cn type=\"integer\">2</cn></apply>"
        ));
    }
}

#[test]
fn partial_unset_models_preserve_none_while_evaluating_known_rational_initializers() {
    let source = "model partial_rational_initials\n  formula k_unset;\n  k_rat = (3/8);\n  k_dep = k_unset + (1/2) + sin((1/4));\n  species S_unset, S_known = (5/2), S_symbolic = S_unset * (2/4);\n  S_known => S_unset; k_rat * S_known;\nend";
    let (model, _) = check_roundtrip(source);

    assert_eq!(param_val(&model, "k_unset"), None);
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "k_unset")
            .unwrap()
            .initial_expression(),
        None
    );
    assert_approx(param_val(&model, "k_rat"), 0.375);
    assert_eq!(param_val(&model, "k_dep"), None);
    assert!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "k_dep")
            .unwrap()
            .initial_expression()
            .is_some()
    );

    let s_unset = model
        .species()
        .iter()
        .find(|s| s.id() == "S_unset")
        .unwrap();
    assert_eq!(s_unset.initial_concentration(), None);
    assert_eq!(s_unset.initial_expression(), None);

    let s_known = model
        .species()
        .iter()
        .find(|s| s.id() == "S_known")
        .unwrap();
    assert_approx(s_known.initial_concentration(), 2.5);
    assert_rational_atom(&model, s_known.initial_expression().unwrap());

    let s_symbolic = model
        .species()
        .iter()
        .find(|s| s.id() == "S_symbolic")
        .unwrap();
    assert_eq!(s_symbolic.initial_concentration(), None);
    assert!(s_symbolic.initial_expression().is_some());
}

#[test]
fn named_and_unnamed_reaction_rates_with_actual_species_and_modifiers() {
    let source = "model reaction_rational_kinetics\n  compartment cell = 1.5;\n  species Substrate in cell = (4/2), Product in cell = 0, Modulator in cell = (1/2);\n  k_cat = (3/4);\n  forward: Substrate => Product; k_cat * (1/2) * Substrate * (1 + sin((1/4)) * Modulator) / ((1)/2 + Substrate);\n  Product =>; (2/4) * Product * cos((1)/2);\nend";
    let (model, _) = check_roundtrip(source);

    assert_eq!(model.compartments()[0].id(), "cell");
    assert_eq!(model.compartments()[0].size(), Some(1.5));

    let forward = model
        .reactions()
        .iter()
        .find(|r| r.id() == "forward")
        .unwrap();
    assert_eq!(forward.reactants()[0].species(), "Substrate");
    assert_eq!(forward.products()[0].species(), "Product");
    assert_eq!(forward.modifiers(), ["Modulator"]);

    let decay = model.reactions().iter().find(|r| r.id() == "_J0").unwrap();
    assert_eq!(decay.reactants()[0].species(), "Product");
    assert!(decay.products().is_empty());
    assert!(decay.modifiers().is_empty());

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<modifierSpeciesReference species=\"Modulator\"/>"));
        assert!(xml.contains("<cn type=\"rational\">4<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">3<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>4</cn>"));
        assert!(xml.contains("<cn type=\"rational\">2<sep/>4</cn>"));
        assert!(xml.contains(
            "<apply><divide/><cn type=\"integer\">1</cn><cn type=\"integer\">2</cn></apply>"
        ));
    }
}

#[test]
fn source_formatter_idempotence_retains_grouping_and_semantic_distinctions() {
    let source = "model formatter_rational_preservation\n  a=(1/2);\n  b=(2/4);\n  c=(1.0/2);\n  d=-(1/-2);\n  e=1/2;\n  f=sin((1/2))+sin((1)/2)+pow(1/2,3);\n  species S=(3/2);\n  r1: S =>; (1/3) * S;\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_approx(param_val(&model, "a"), 0.5);
    assert_approx(param_val(&model, "b"), 0.5);
    assert_approx(param_val(&model, "c"), 0.5);
    assert_approx(param_val(&model, "d"), 0.5);
    assert_approx(param_val(&model, "e"), 0.5);
    assert!(
        !formatted.contains("sin(1 / 2)") && !formatted.contains("sin(1/2)"),
        "formatter must never strip grouping to produce upstream-rejected sin(1/2): {formatted}"
    );
    assert_rational_atom(&model, param_expr(&model, "a"));
    assert_rational_atom(&model, param_expr(&model, "b"));
    assert_rational_atom(&model, param_expr(&model, "c"));
    assert_rational_atom(&model, param_expr(&model, "d"));
    assert!(matches!(
        model.expression(param_expr(&model, "e")).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Divide,
            ..
        }
    ));
}
