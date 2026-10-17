//! Ordinary authoring integration tests for numeric builtins, constants, `time`,
//! partial/symbolic initialization, reaction modifiers, and canonical formatting.
use antimony::{
    Builtin, Diagnostic, DiagnosticCode, ExpressionKind, IMPLEMENTED_PROFILE, Limits,
    ParsedDocument, SourceFile, SourceId, SymbolKind, ValidatedModel, analyze, format_antimony,
    parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(301), "builtin_ergonomics", text, &limits).unwrap();
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
        SourceId::new(302),
        "builtin_ergonomics_formatted",
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
    assert_eq!(snap_species(&model), snap_species(&reanalyzed));
    assert_eq!(snap_params(&model), snap_params(&reanalyzed));
    assert_eq!(snap_rxns(&model), snap_rxns(&reanalyzed));
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

#[test]
fn nested_calls_and_known_initial_values_evaluate_accurately() {
    let source = "model trig_rounding_calibration\n  a = sin(pi / 6) + cos(0) + tan(pi / 4);\n  b = sqrt(pow(3, 2) + power(4, 2));\n  c = abs(-3.75) + floor(2.9) + ceil(2.1) + ceiling(1.05);\n  d = exp(ln(2.5));\nend";
    let (model, _) = check_roundtrip(source);

    assert_eq!(model.profile_id(), IMPLEMENTED_PROFILE);
    assert_eq!(IMPLEMENTED_PROFILE, "modular-core-v0.8-slice");
    assert_approx(param_val(&model, "a"), 2.5);
    assert_approx(param_val(&model, "b"), 5.0);
    assert_approx(param_val(&model, "c"), 3.75 + 2.0 + 3.0 + 2.0);
    assert_approx(param_val(&model, "d"), 2.5);
}

#[test]
fn logarithm_aliases_ln_unary_log_and_log10_resolve_and_evaluate() {
    let source = "model log_variants\n  x = exponentiale ^ 3;\n  n1 = ln(x);\n  n2 = log(x);\n  d10 = log10(1000);\n  species S = log(exponentiale ^ 2);\n  S =>; log10(10) * S;\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "n1"), 3.0);
    assert_approx(param_val(&model, "n2"), 3.0);
    assert_approx(param_val(&model, "d10"), 3.0);
    assert_approx(model.species()[0].initial_concentration(), 2.0);

    let n2_expr = model
        .parameters()
        .iter()
        .find(|p| p.id() == "n2")
        .unwrap()
        .initial_expression()
        .unwrap();
    assert!(matches!(
        model.expression(n2_expr).unwrap().kind(),
        ExpressionKind::Call { function: Builtin::Ln, args } if args.len() == 1
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<ln/>"));
        assert!(xml.contains("<log/><logbase><cn type=\"integer\">10</cn></logbase>"));
    }
}

#[test]
fn two_argument_log_evaluates_base_value_and_exports_logbase() {
    let source = "model custom_log_bases\n  b2 = log(2, 8);\n  b_half = log(0.5, 0.25);\n  nested = log(3, pow(3, 4));\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "b2"), 3.0);
    assert_approx(param_val(&model, "b_half"), 2.0);
    assert_approx(param_val(&model, "nested"), 4.0);

    let b2_expr = model
        .parameters()
        .iter()
        .find(|p| p.id() == "b2")
        .unwrap()
        .initial_expression()
        .unwrap();
    assert!(matches!(
        model.expression(b2_expr).unwrap().kind(),
        ExpressionKind::Call { function: Builtin::Log, args } if args.len() == 2
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains(
            "<log/><logbase><cn type=\"integer\">2</cn></logbase><cn type=\"integer\">8</cn>"
        ));
    }
}

#[test]
fn power_aliases_pow_and_power_match_caret_and_preserve_call_syntax() {
    let source = "model power_aliases\n  p_short = pow(2, 5);\n  p_long = power(2, 5);\n  p_infix = 2 ^ 5;\n  p_frac = power(9, 0.5) + pow(8, -1);\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_approx(param_val(&model, "p_short"), 32.0);
    assert_approx(param_val(&model, "p_long"), 32.0);
    assert_approx(param_val(&model, "p_infix"), 32.0);
    assert_approx(param_val(&model, "p_frac"), 3.125);
    assert!(formatted.contains("p_short = pow(2, 5);\n"));
    assert!(formatted.contains("p_long = power(2, 5);\n"));

    for id in ["p_short", "p_long"] {
        let expr_id = model
            .parameters()
            .iter()
            .find(|p| p.id() == id)
            .unwrap()
            .initial_expression()
            .unwrap();
        assert!(matches!(
            model.expression(expr_id).unwrap().kind(),
            ExpressionKind::Call { function: Builtin::Power, args } if args.len() == 2
        ));
    }
}

#[test]
fn variadic_min_and_max_support_single_and_multiple_arguments() {
    let source = "model variadic_extrema\n  single_min = min(4.5);\n  single_max = max(4.5);\n  multi_min = min(7, 2.5, 9, 1.25, 4);\n  multi_max = max(-3, 0.5, 12.75, 8);\n  composed = max(min(10, 3), min(5, 6, 4));\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "single_min"), 4.5);
    assert_approx(param_val(&model, "single_max"), 4.5);
    assert_approx(param_val(&model, "multi_min"), 1.25);
    assert_approx(param_val(&model, "multi_max"), 12.75);
    assert_approx(param_val(&model, "composed"), 4.0);
}

#[test]
fn constants_pi_and_exponentiale_in_initializers_and_rates() {
    let source = "model harmonic_constants\n  omega = 2 * pi;\n  damping = exponentiale ^ (-0.5);\n  species X = sin(pi / 2) * exponentiale;\n  X =>; omega * damping * X / pi;\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "omega"), 2.0 * std::f64::consts::PI);
    assert_approx(param_val(&model, "damping"), (-0.5_f64).exp());
    assert_approx(
        model.species()[0].initial_concentration(),
        std::f64::consts::E,
    );
    let omega_expr = model
        .parameters()
        .iter()
        .find(|p| p.id() == "omega")
        .unwrap()
        .initial_expression()
        .unwrap();
    assert!(matches!(
        model.expression(omega_expr).unwrap().kind(),
        ExpressionKind::Binary { right, .. }
            if matches!(
                model.expression(*right).unwrap().kind(),
                ExpressionKind::Constant(
                    antimony::NumericConstant::Pi | antimony::NumericConstant::ExponentialE
                )
            )
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<pi/>"));
        assert!(xml.contains("<exponentiale/>"));
    }
}

#[test]
fn identifier_e_is_ordinary_symbol_distinct_from_exponentiale() {
    let source = "model symbol_e_distinction\n  e = 0.5;\n  combined = e + exponentiale;\n  formula unset_e_dep;\n  derived = unset_e_dep + e;\nend";
    let (model, _) = check_roundtrip(source);

    let e_param = model.parameters().iter().find(|p| p.id() == "e").unwrap();
    assert_eq!(e_param.value(), Some(0.5));
    assert_approx(param_val(&model, "combined"), 0.5 + std::f64::consts::E);
    assert_eq!(param_val(&model, "derived"), None);

    let (uninitialized_e, _) = check_roundtrip("formula e; y = e * 2;");
    assert_eq!(param_val(&uninitialized_e, "e"), None);
    assert_eq!(param_val(&uninitialized_e, "y"), None);
}

#[test]
fn incomplete_parameters_propagate_none_through_builtin_initializers() {
    let source = "model partial_builtin_initials\n  formula k_unset;\n  k_known = log10(100);\n  k_partial = exp(k_unset) + min(1, k_unset, k_known);\n  species S = sqrt(k_unset + 4);\n  S =>; k_known * S;\nend";
    let (model, _) = check_roundtrip(source);

    let k_unset = model
        .parameters()
        .iter()
        .find(|p| p.id() == "k_unset")
        .unwrap();
    assert_eq!(k_unset.value(), None);
    assert_eq!(k_unset.initial_expression(), None);

    let k_known = model
        .parameters()
        .iter()
        .find(|p| p.id() == "k_known")
        .unwrap();
    assert_approx(k_known.value(), 2.0);
    assert!(k_known.initial_expression().is_some());

    let k_partial = model
        .parameters()
        .iter()
        .find(|p| p.id() == "k_partial")
        .unwrap();
    assert_eq!(k_partial.value(), None);
    assert!(k_partial.initial_expression().is_some());

    let s = &model.species()[0];
    assert_eq!(s.initial_concentration(), None);
    assert!(s.initial_expression().is_some());
}

#[test]
fn time_and_time_call_in_initializers_remain_symbolic_none_not_zero() {
    let source = "model symbolic_time_initials\n  p_bare = cos(time);\n  p_call = time() + 2.5;\n  species S = exp(-time());\n  S =>; 0.1 * S;\nend";
    let (model, _) = check_roundtrip(source);

    for id in ["p_bare", "p_call"] {
        let p = model.parameters().iter().find(|p| p.id() == id).unwrap();
        assert_eq!(
            p.value(),
            None,
            "{id} must remain symbolic None, never 0-substituted"
        );
        assert!(p.initial_expression().is_some());
    }
    let s = &model.species()[0];
    assert_eq!(s.initial_concentration(), None);
    assert!(s.initial_expression().is_some());

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(
            xml.contains("definitionURL=\"http://www.sbml.org/sbml/symbols/time\">time</csymbol>")
        );
        assert!(!xml.contains("initialConcentration="));
    }
}

#[test]
fn time_forcing_in_named_and_unnamed_reactions() {
    let source = "model forced_oscillator\n  species X = 1;\n  k_in = 0.8; k_out = 0.2; tau = 5;\n  drive: => X; k_in * (1 + sin(2 * pi * time));\n  X =>; k_out * X * exp(-time() / tau);\nend";
    let (model, formatted) = check_roundtrip(source);

    assert!(formatted.contains("drive: => X; k_in * (1 + sin(2 * pi * time));\n"));
    assert!(formatted.contains("X =>; k_out * X * exp(-time() / tau);\n"));
    assert_eq!(
        model.reactions().iter().map(|r| r.id()).collect::<Vec<_>>(),
        ["_J0", "drive"]
    );
}

#[test]
fn nested_function_argument_species_included_as_reaction_modifiers() {
    let source = "model nested_modifier_extraction\n  species A = 4, B = 0, M1 = 1, M2 = 3, M3 = 2, M4 = 0.5;\n  k = 0.25;\n  catalyze: A => B; k * A * max(1, min(M1, sqrt(M2 + ln(1 + M3)))) / (1 + exp(-M4));\nend";
    let (model, _) = check_roundtrip(source);

    let rxn = &model.reactions()[0];
    assert_eq!(rxn.id(), "catalyze");
    assert_eq!(rxn.reactants()[0].species(), "A");
    assert_eq!(rxn.products()[0].species(), "B");
    assert_eq!(rxn.modifiers(), ["M1", "M2", "M3", "M4"]);

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        for m in ["M1", "M2", "M3", "M4"] {
            assert!(xml.contains(&format!("<modifierSpeciesReference species=\"{m}\"/>")));
        }
    }
}

#[test]
fn source_format_idempotence_retains_alias_spellings_and_semantics() {
    let source = "model alias_preservation\n  a=ln(2)+log(2)+log(2,8)+log10(10);\n  b=ceil(1.2)+ceiling(1.2)+pow(2,3)+power(2,3);\n  species S=1;\n  S=>;S*(1+sin(time)+cos(time())+pi/exponentiale);\nend";
    let (_, formatted) = check_roundtrip(source);

    assert_eq!(
        formatted,
        "model alias_preservation\n  a = ln(2) + log(2) + log(2, 8) + log10(10);\n  b = ceil(1.2) + ceiling(1.2) + pow(2, 3) + power(2, 3);\n  species S = 1;\n  S =>; S * (1 + sin(time) + cos(time()) + pi / exponentiale);\nend\n"
    );
}

#[test]
fn strict_rejection_of_unknown_functions_user_definitions_and_conditions() {
    // Finite numeric conditions and user-defined functions are now supported.
    analyze(&parsed("k = piecewise(1, 2);").unwrap()).unwrap();
    analyze(&parsed("function f(x) x + 1 end").unwrap()).unwrap();
    for (bad_source, expected_code) in [
        ("k = hill(2, 3);", DiagnosticCode::UnsupportedFeature),
        ("k = asin(0.5);", DiagnosticCode::UnsupportedFeature),
        ("k = min();", DiagnosticCode::UnsupportedFeature),
        ("k = sin(1, 2);", DiagnosticCode::Syntax),
        ("k = pow(2);", DiagnosticCode::Syntax),
        ("k = pi(2);", DiagnosticCode::Syntax),
        ("k = time(1);", DiagnosticCode::Syntax),
    ] {
        let err = parsed(bad_source).expect_err(bad_source);
        assert_eq!(
            err.code(),
            expected_code,
            "unexpected code for {bad_source}"
        );
    }
}

#[test]
fn freeform_named_circadian_bioreactor_model_combines_declarations_and_builtins() {
    let source = "model circadian_bioreactor_v3()\n  compartment vessel = 2.5;\n  species $Feed in vessel, Nutrient in vessel, Biomass in vessel, Regulator in vessel;\n  formula k_feed = 0.4, period = 24, basal = log(exponentiale ^ 0.5), damp;\n  Feed = 10 * exp(-0.1);\n  Nutrient = max(1, min(5, sqrt(pow(3, 2) + power(4, 2)) / 2));\n  Biomass = ceil(0.2) + floor(1.8);\n  Regulator = log(2, 8) / log10(1000);\n  inflow: $Feed => Nutrient; k_feed * Feed * (1 + 0.5 * sin(2 * pi * time / period));\n  Nutrient => Biomass; 0.15 * Nutrient * Biomass * (1 + cos(time())) / (1 + exp(-Regulator));\n  Biomass =>; 0.05 * Biomass;\nend";
    let (model, _) = check_roundtrip(source);

    assert_eq!(model.id(), "circadian_bioreactor_v3");
    assert_eq!(model.compartments()[0].size(), Some(2.5));
    assert_approx(param_val(&model, "basal"), 0.5);
    assert_eq!(param_val(&model, "damp"), None);

    let nutrient = model
        .species()
        .iter()
        .find(|s| s.id() == "Nutrient")
        .unwrap();
    assert_approx(nutrient.initial_concentration(), 2.5);
    let growth = model.reactions().iter().find(|r| r.id() == "_J0").unwrap();
    assert_eq!(growth.modifiers(), ["Regulator"]);
    assert!(matches!(
        model.expression(growth.rate()).unwrap().kind(),
        ExpressionKind::Binary { .. }
            | ExpressionKind::Call { .. }
            | ExpressionKind::Symbol {
                kind: SymbolKind::Species,
                ..
            }
    ));
}
