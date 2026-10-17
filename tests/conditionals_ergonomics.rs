//! Ordinary authoring integration tests for `conditional-core-v0.3-slice`:
//! piecewise time stimuli, lazy selected-branch and short-circuit evaluation,
//! unknown condition/operand propagation, Boolean/numeric context coercions,
//! equal-precedence left-associative `&&`/`||`, direct relational/logical calls,
//! static all-branch cycle/unit/literal checks, and formatter idempotence.
use antimony::{
    BinaryOp, ComparisonOp, Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionId,
    ExpressionKind, IMPLEMENTED_PROFILE, Limits, LogicalOp, ParsedDocument, PiecewiseBranch,
    SourceFile, SourceId, SymbolKind, ValidatedModel, analyze, format_antimony, parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source =
        SourceFile::new(SourceId::new(501), "conditionals_ergonomics", text, &limits).unwrap();
    parse(&source, &limits)
}

fn compile_err_code(text: &str) -> DiagnosticCode {
    match parsed(text) {
        Err(diag) => diag.code(),
        Ok(doc) => analyze(&doc)
            .expect_err("expected compilation failure")
            .code(),
    }
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
        SourceId::new(502),
        "conditionals_ergonomics_formatted",
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

#[test]
fn piecewise_time_stimuli_and_ordered_mathml_vs_raw_apply() {
    let source = "model time_stimuli\n  pulse = piecewise(0, time < 1, 10, time <= 5, 1);\n  step_no_else = piecewise(5, time() > 2);\n  only_default = piecewise(4.5);\nend";
    let (model, _) = check_roundtrip(source);

    assert_eq!(IMPLEMENTED_PROFILE, "modular-core-v0.8-slice");
    assert_eq!(model.profile_id(), "modular-core-v0.8-slice");

    assert_eq!(param_val(&model, "pulse"), None);
    assert_eq!(param_val(&model, "step_no_else"), None);
    assert_approx(param_val(&model, "only_default"), 4.5);

    let ExpressionKind::Piecewise {
        branches: pulse_branches,
        otherwise: Some(pulse_else),
    } = model
        .expression(param_expr(&model, "pulse"))
        .unwrap()
        .kind()
    else {
        panic!("expected piecewise with otherwise for pulse");
    };
    assert_eq!(pulse_branches.len(), 2);
    let PiecewiseBranch {
        value: first_val,
        condition: first_cond,
    } = pulse_branches[0];
    assert!(matches!(
        model.expression(first_val).unwrap().kind(),
        ExpressionKind::Number { value, .. } if *value == 0.0
    ));
    assert!(matches!(
        model.expression(first_cond).unwrap().kind(),
        ExpressionKind::Compare {
            op: ComparisonOp::Less,
            args
        } if args.len() == 2 && matches!(model.expression(args[0]).unwrap().kind(), ExpressionKind::Time)
    ));
    assert!(matches!(
        model.expression(*pulse_else).unwrap().kind(),
        ExpressionKind::Number { value, .. } if *value == 1.0
    ));

    let ExpressionKind::Piecewise {
        branches: step_branches,
        otherwise: step_else,
    } = model
        .expression(param_expr(&model, "step_no_else"))
        .unwrap()
        .kind()
    else {
        panic!("expected piecewise for step_no_else");
    };
    assert_eq!(step_branches.len(), 1);
    assert_eq!(*step_else, None);

    let ExpressionKind::Piecewise {
        branches: default_branches,
        otherwise: Some(default_else),
    } = model
        .expression(param_expr(&model, "only_default"))
        .unwrap()
        .kind()
    else {
        panic!("expected otherwise-only piecewise for only_default");
    };
    assert!(default_branches.is_empty());
    assert!(matches!(
        model.expression(*default_else).unwrap().kind(),
        ExpressionKind::Number { value, .. } if *value == 4.5
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains(
            "<piecewise><piece><cn type=\"integer\">0</cn><apply><lt/><csymbol encoding=\"text\" definitionURL=\"http://www.sbml.org/sbml/symbols/time\">time</csymbol><cn type=\"integer\">1</cn></apply></piece><piece><cn type=\"integer\">10</cn><apply><leq/><csymbol encoding=\"text\" definitionURL=\"http://www.sbml.org/sbml/symbols/time\">time</csymbol><cn type=\"integer\">5</cn></apply></piece><otherwise><cn type=\"integer\">1</cn></otherwise></piecewise>"
        ));
        assert!(xml.contains(
            "<piecewise><piece><cn type=\"integer\">5</cn><apply><gt/><csymbol encoding=\"text\" definitionURL=\"http://www.sbml.org/sbml/symbols/time\">time</csymbol><cn type=\"integer\">2</cn></apply></piece></piecewise>"
        ));
        // Real cn may spell its default type explicitly; the typed value above
        // and otherwise-only shape are the semantic contract.
        assert!(xml.contains("<piecewise><otherwise>"));
        assert!(xml.contains("</otherwise></piecewise>"));
        assert!(!xml.contains("<apply><piecewise"));
    }
}

#[test]
fn selected_branch_laziness_skips_inactive_domain_errors() {
    let source = "model lazy_piecewise_branches\n  active_first = piecewise(7, true, sqrt(-1));\n  active_fallback = piecewise(sqrt(-1), false, 9);\n  multi_skip = piecewise(1 / 0, 2 < 1, ln(0), false, (17/4));\n  masked_later_cond = piecewise(11, 1 == 1, 12, sqrt(-1) > 0, 13);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "active_first"), 7.0);
    assert_approx(param_val(&model, "active_fallback"), 9.0);
    assert_approx(param_val(&model, "multi_skip"), 4.25);
    assert_approx(param_val(&model, "masked_later_cond"), 11.0);

    for bad_active in [
        "a = piecewise(sqrt(-1), true, 7);",
        "a = piecewise(7, false, 1 / 0);",
        "a = piecewise(7, sqrt(-1) > 0, 9);",
    ] {
        let doc = parsed(bad_active).expect("active domain error source should parse");
        assert!(
            analyze(&doc).is_err(),
            "expected demanded domain error to reject initialization: {bad_active}"
        );
    }
}

#[test]
fn unknown_conditions_return_none_and_incomplete_declarations_are_retained() {
    let source = "model unknown_condition_preservation\n  formula u_cond;\n  species S_unset;\n  p_unknown_branch = piecewise(10, u_cond > 0, 20);\n  p_identical_alts = piecewise(5, u_cond == 0, 5);\n  p_masks_later_err = piecewise(1, u_cond > 0, sqrt(-1));\n  p_time_cond = piecewise(7, time > 0, 9);\n  species S_cond = piecewise(2.5, S_unset > 1, 0.5);\nend";
    let (model, _) = check_roundtrip(source);

    let u_cond = model
        .parameters()
        .iter()
        .find(|p| p.id() == "u_cond")
        .unwrap();
    assert_eq!(u_cond.value(), None);
    assert_eq!(u_cond.initial_expression(), None);

    let s_unset = model
        .species()
        .iter()
        .find(|s| s.id() == "S_unset")
        .unwrap();
    assert_eq!(s_unset.initial_concentration(), None);
    assert_eq!(s_unset.initial_expression(), None);

    for id in [
        "p_unknown_branch",
        "p_identical_alts",
        "p_masks_later_err",
        "p_time_cond",
    ] {
        assert_eq!(
            param_val(&model, id),
            None,
            "{id} must remain symbolic None"
        );
        assert!(model.expression(param_expr(&model, id)).is_some());
    }

    let s_cond = model.species().iter().find(|s| s.id() == "S_cond").unwrap();
    assert_eq!(s_cond.initial_concentration(), None);
    assert!(s_cond.initial_expression().is_some());
}

#[test]
fn boolean_spellings_and_numeric_coercion_in_initial_values_and_conditions() {
    let source = "model boolean_numeric_coercion\n  b_t1 = true;\n  b_t2 = True;\n  b_t3 = TRUE;\n  b_f1 = false;\n  b_f2 = False;\n  b_f3 = FALSE;\n  TrUe = 42;\n  mixed_case_ref = TrUe + 1;\n  add_true = true + 2;\n  neg_true = -true;\n  pow_true = true ^ 2;\n  sin_true = sin(true);\n  pw_bool_val = piecewise(true, true, 9);\n  pw_zero_cond = piecewise(7, 0, 9);\n  pw_neg_cond = piecewise(7, -2, 9);\n  pw_rat_cond = piecewise(7, (1/2), 9);\n  not_zero = not(0);\n  not_neg = not(-2);\n  not_half = not(0.5);\nend";
    let (model, _) = check_roundtrip(source);

    for id in ["b_t1", "b_t2", "b_t3"] {
        assert_approx(param_val(&model, id), 1.0);
        assert!(matches!(
            model.expression(param_expr(&model, id)).unwrap().kind(),
            ExpressionKind::Boolean(true)
        ));
        let Some(DimensionalStatus::Known(dims)) =
            model.expression_dimensions(param_expr(&model, id))
        else {
            panic!("expected known dimensionless status for {id}");
        };
        assert_eq!((dims.substance(), dims.time(), dims.length()), (0, 0, 0));
    }
    for id in ["b_f1", "b_f2", "b_f3"] {
        assert_approx(param_val(&model, id), 0.0);
        assert!(matches!(
            model.expression(param_expr(&model, id)).unwrap().kind(),
            ExpressionKind::Boolean(false)
        ));
    }

    assert_approx(param_val(&model, "TrUe"), 42.0);
    assert_approx(param_val(&model, "mixed_case_ref"), 43.0);
    let ExpressionKind::Binary {
        op: BinaryOp::Add,
        left,
        ..
    } = model
        .expression(param_expr(&model, "mixed_case_ref"))
        .unwrap()
        .kind()
    else {
        panic!("expected binary add for mixed_case_ref");
    };
    assert!(matches!(
        model.expression(*left).unwrap().kind(),
        ExpressionKind::Symbol {
            name,
            kind: SymbolKind::Parameter
        } if name == "TrUe"
    ));

    assert_approx(param_val(&model, "add_true"), 3.0);
    assert_approx(param_val(&model, "neg_true"), -1.0);
    assert_approx(param_val(&model, "pow_true"), 1.0);
    assert_approx(param_val(&model, "sin_true"), 1.0_f64.sin());
    assert_approx(param_val(&model, "pw_bool_val"), 1.0);
    assert_approx(param_val(&model, "pw_zero_cond"), 9.0);
    assert_approx(param_val(&model, "pw_neg_cond"), 7.0);
    assert_approx(param_val(&model, "pw_rat_cond"), 7.0);
    assert_approx(param_val(&model, "not_zero"), 1.0);
    assert_approx(param_val(&model, "not_neg"), 0.0);
    assert_approx(param_val(&model, "not_half"), 0.0);

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<true/>"));
        assert!(xml.contains("<false/>"));
        assert!(xml.contains("<apply><sin/><true/></apply>"));
    }
}

#[test]
fn symbolic_and_functional_logical_operators_empty_identities_and_not_rational_guard() {
    let source = "model logical_forms_and_not_calls\n  id_and = and();\n  id_or = or();\n  one_and = and(false);\n  one_or = or(2);\n  sym_not = !false;\n  fn_not_spaced = not (false);\n  dbl_sym_not = !(!true);\n  dbl_fn_not = not(not(false));\n  not_rat_atom = not((1/2));\n  not_div_arg = not((1)/2);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "id_and"), 1.0);
    assert_approx(param_val(&model, "id_or"), 0.0);
    assert_approx(param_val(&model, "one_and"), 0.0);
    assert_approx(param_val(&model, "one_or"), 1.0);
    assert_approx(param_val(&model, "sym_not"), 1.0);
    assert_approx(param_val(&model, "fn_not_spaced"), 1.0);
    assert_approx(param_val(&model, "dbl_sym_not"), 1.0);
    assert_approx(param_val(&model, "dbl_fn_not"), 0.0);
    assert_approx(param_val(&model, "not_rat_atom"), 0.0);
    assert_approx(param_val(&model, "not_div_arg"), 0.0);

    assert!(matches!(
        model.expression(param_expr(&model, "id_and")).unwrap().kind(),
        ExpressionKind::Logical {
            op: LogicalOp::And,
            args
        } if args.is_empty()
    ));
    assert!(matches!(
        model.expression(param_expr(&model, "id_or")).unwrap().kind(),
        ExpressionKind::Logical {
            op: LogicalOp::Or,
            args
        } if args.is_empty()
    ));

    for dbl in ["dbl_sym_not", "dbl_fn_not"] {
        let ExpressionKind::Not { child } =
            model.expression(param_expr(&model, dbl)).unwrap().kind()
        else {
            panic!("expected outer Not for {dbl}");
        };
        assert!(
            matches!(
                model.expression(*child).unwrap().kind(),
                ExpressionKind::Not { .. }
            ),
            "double negation must not be simplified away for {dbl}"
        );
    }

    let ExpressionKind::Not { child: rat_child } = model
        .expression(param_expr(&model, "not_rat_atom"))
        .unwrap()
        .kind()
    else {
        panic!("expected Not for not_rat_atom");
    };
    assert!(matches!(
        model.expression(*rat_child).unwrap().kind(),
        ExpressionKind::Rational {
            numerator: 1,
            denominator: 2
        }
    ));

    for bad_not_rat in ["a = not(1/2);", "a = not (1/2);", "a = not(-1/2);"] {
        assert_eq!(
            compile_err_code(bad_not_rat),
            DiagnosticCode::Syntax,
            "{bad_not_rat} should reject under the shared call rational guard"
        );
    }
}

#[test]
fn and_or_equal_precedence_left_associative_and_call_vs_infix_normalization() {
    let source = "model logical_precedence_and_flattening\n  left_assoc = true || false && false;\n  explicit_right = true || (false && false);\n  not_binds_tighter = !1 < 2;\n  arith_in_rel = 1 + 2 * 3 < 2 ^ 3;\n  infix_chain_and = true && false && true;\n  fn_left_infix_append = and(true, false) && true;\n  empty_fn_left_append = and() && true;\n  right_fn_preserved = true && and(false, true);\n  outer_fn_preserved = and(and(true, false), true);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "left_assoc"), 0.0);
    assert_approx(param_val(&model, "explicit_right"), 1.0);
    assert_approx(param_val(&model, "not_binds_tighter"), 1.0);
    assert_approx(param_val(&model, "arith_in_rel"), 1.0);
    assert_approx(param_val(&model, "infix_chain_and"), 0.0);
    assert_approx(param_val(&model, "fn_left_infix_append"), 0.0);
    assert_approx(param_val(&model, "empty_fn_left_append"), 1.0);
    assert_approx(param_val(&model, "right_fn_preserved"), 0.0);
    assert_approx(param_val(&model, "outer_fn_preserved"), 0.0);

    let ExpressionKind::Logical {
        op: LogicalOp::And,
        args: left_assoc_args,
    } = model
        .expression(param_expr(&model, "left_assoc"))
        .unwrap()
        .kind()
    else {
        panic!("expected outer And for left-associative `true || false && false`");
    };
    assert_eq!(left_assoc_args.len(), 2);
    assert!(matches!(
        model.expression(left_assoc_args[0]).unwrap().kind(),
        ExpressionKind::Logical {
            op: LogicalOp::Or,
            ..
        }
    ));

    for three_arg_id in ["infix_chain_and", "fn_left_infix_append"] {
        let ExpressionKind::Logical {
            op: LogicalOp::And,
            args,
        } = model
            .expression(param_expr(&model, three_arg_id))
            .unwrap()
            .kind()
        else {
            panic!("expected flattened And for {three_arg_id}");
        };
        assert_eq!(args.len(), 3, "{three_arg_id} should have 3 children");
    }

    let ExpressionKind::Logical {
        op: LogicalOp::And,
        args: empty_append_args,
    } = model
        .expression(param_expr(&model, "empty_fn_left_append"))
        .unwrap()
        .kind()
    else {
        panic!("expected one-child And for `and() && true`");
    };
    assert_eq!(empty_append_args.len(), 1);

    for nested_two_arg_id in ["right_fn_preserved", "outer_fn_preserved"] {
        let ExpressionKind::Logical {
            op: LogicalOp::And,
            args,
        } = model
            .expression(param_expr(&model, nested_two_arg_id))
            .unwrap()
            .kind()
        else {
            panic!("expected 2-arg outer And for {nested_two_arg_id}");
        };
        assert_eq!(
            args.len(),
            2,
            "{nested_two_arg_id} must preserve nested child"
        );
    }
}

#[test]
fn and_or_sequential_short_circuit_and_first_unknown_stops_evaluation() {
    let source = "model logical_short_circuit_and_unknowns\n  formula u_unknown;\n  and_short = false && (sqrt(-1) > 0);\n  or_short = true || (1 / 0 > 0);\n  fn_and_short = and(1, 0, sqrt(-1));\n  fn_or_short = or(0, 2, ln(0));\n  u_and_err = u_unknown && sqrt(-1);\n  u_or_err = u_unknown || ((1) / 0);\n  u_and_false = u_unknown && false;\n  u_or_true = u_unknown || true;\n  known_before_u_and = false && u_unknown;\n  known_before_u_or = true || u_unknown;\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "and_short"), 0.0);
    assert_approx(param_val(&model, "or_short"), 1.0);
    assert_approx(param_val(&model, "fn_and_short"), 0.0);
    assert_approx(param_val(&model, "fn_or_short"), 1.0);

    for symbolic_id in ["u_and_err", "u_or_err", "u_and_false", "u_or_true"] {
        assert_eq!(
            param_val(&model, symbolic_id),
            None,
            "{symbolic_id} must stop at first unknown operand and return None"
        );
    }

    assert_approx(param_val(&model, "known_before_u_and"), 0.0);
    assert_approx(param_val(&model, "known_before_u_or"), 1.0);

    for bad_demanded in [
        "a = true && (sqrt(-1) > 0);",
        "a = false || (1 / 0 > 0);",
        "a = and(true, ln(0));",
    ] {
        let doc = parsed(bad_demanded).expect("demanded logical domain error should parse");
        assert!(
            analyze(&doc).is_err(),
            "expected demanded logical domain error to fail analysis: {bad_demanded}"
        );
    }
}

#[test]
fn direct_relation_functions_and_infix_comparisons() {
    let source = "model relation_functions_and_infix\n  formula u_val;\n  c_lt = 1 < 2;\n  c_leq = 2 <= 2;\n  c_gt = 3 > 4;\n  c_geq = 4 >= 3;\n  c_eq = 5 == 5;\n  c_neq = 5 != 5;\n  f_eq3 = eq(2, 2, 2);\n  f_equals3 = equals(2, 2, 3);\n  f_lt3 = lt(1, 2, 3);\n  f_leq3 = leq(1, 2, 2);\n  f_gt3 = gt(3, 2, 1);\n  f_geq3 = geq(3, 2, 2);\n  f_neq2 = neq(1, 2);\n  nested_lt_call = lt(lt(1, 2), 3);\n  right_nested_infix = 1 < (2 < 3);\n  rel_with_unknown = lt(1, u_val, 3);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "c_lt"), 1.0);
    assert_approx(param_val(&model, "c_leq"), 1.0);
    assert_approx(param_val(&model, "c_gt"), 0.0);
    assert_approx(param_val(&model, "c_geq"), 1.0);
    assert_approx(param_val(&model, "c_eq"), 1.0);
    assert_approx(param_val(&model, "c_neq"), 0.0);
    assert_approx(param_val(&model, "f_eq3"), 1.0);
    assert_approx(param_val(&model, "f_equals3"), 0.0);
    assert_approx(param_val(&model, "f_lt3"), 1.0);
    assert_approx(param_val(&model, "f_leq3"), 1.0);
    assert_approx(param_val(&model, "f_gt3"), 1.0);
    assert_approx(param_val(&model, "f_geq3"), 1.0);
    assert_approx(param_val(&model, "f_neq2"), 1.0);
    assert_approx(param_val(&model, "nested_lt_call"), 1.0);
    assert_approx(param_val(&model, "right_nested_infix"), 0.0);
    assert_eq!(param_val(&model, "rel_with_unknown"), None);

    let expected_ops = [
        ("c_lt", ComparisonOp::Less, 2),
        ("c_leq", ComparisonOp::LessEqual, 2),
        ("c_gt", ComparisonOp::Greater, 2),
        ("c_geq", ComparisonOp::GreaterEqual, 2),
        ("c_eq", ComparisonOp::Equal, 2),
        ("c_neq", ComparisonOp::NotEqual, 2),
        ("f_eq3", ComparisonOp::Equal, 3),
        ("f_equals3", ComparisonOp::Equal, 3),
        ("f_lt3", ComparisonOp::Less, 3),
        ("f_leq3", ComparisonOp::LessEqual, 3),
        ("f_gt3", ComparisonOp::Greater, 3),
        ("f_geq3", ComparisonOp::GreaterEqual, 3),
        ("f_neq2", ComparisonOp::NotEqual, 2),
    ];
    for (id, expected_op, expected_len) in expected_ops {
        let ExpressionKind::Compare { op, args } =
            model.expression(param_expr(&model, id)).unwrap().kind()
        else {
            panic!("expected Compare for {id}");
        };
        assert_eq!(*op, expected_op);
        assert_eq!(args.len(), expected_len);
    }

    let ExpressionKind::Compare {
        op: ComparisonOp::Less,
        args: nested_args,
    } = model
        .expression(param_expr(&model, "nested_lt_call"))
        .unwrap()
        .kind()
    else {
        panic!("expected Compare for nested_lt_call");
    };
    assert_eq!(nested_args.len(), 2);
    assert!(matches!(
        model.expression(nested_args[0]).unwrap().kind(),
        ExpressionKind::Compare {
            op: ComparisonOp::Less,
            ..
        }
    ));

    let eager_rel_err = parsed("formula u; a = lt(1, u, sqrt(-1));").unwrap();
    assert!(
        analyze(&eager_rel_err).is_err(),
        "n-ary comparison should still reject known domain errors in other demanded arguments"
    );
}

#[test]
fn piecewise_missing_otherwise_in_initializers_vs_structural_reaction_rates() {
    let source = "model piecewise_missing_otherwise\n  matched_no_else = piecewise(3.5, 1 < 2, 7.0, 2 < 1);\n  species S1 = 2.0, S2 = 0.0;\n  k1 = 0.4;\n  k2 = 0.1;\n  r_gated: S1 => S2; piecewise(k1 * S1, S1 > 1, k2 * S1, S1 > 0);\nend";
    let (model, _) = check_roundtrip(source);

    assert_approx(param_val(&model, "matched_no_else"), 3.5);
    let ExpressionKind::Piecewise {
        branches,
        otherwise,
    } = model
        .expression(param_expr(&model, "matched_no_else"))
        .unwrap()
        .kind()
    else {
        panic!("expected Piecewise for matched_no_else");
    };
    assert_eq!(branches.len(), 2);
    assert_eq!(*otherwise, None);

    let r_gated = &model.reactions()[0];
    let ExpressionKind::Piecewise {
        branches: rate_branches,
        otherwise: rate_otherwise,
    } = model.expression(r_gated.rate()).unwrap().kind()
    else {
        panic!("expected Piecewise in reaction rate");
    };
    assert_eq!(rate_branches.len(), 2);
    assert_eq!(*rate_otherwise, None);

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(!xml.contains("<otherwise>"));
        // One zero is the authored S1 > 0 condition, not an invented fallback.
        assert_eq!(xml.matches("<cn type=\"integer\">0</cn>").count(), 1);
    }

    for unmatched_init in [
        "a = piecewise(7, false);",
        "a = piecewise(1, 2 > 3, 2, 4 == 5);",
    ] {
        let doc = parsed(unmatched_init).expect("unmatched piecewise without otherwise parses");
        assert!(
            analyze(&doc).is_err(),
            "known no-match without otherwise must reject finite initialization: {unmatched_init}"
        );
    }
}

#[test]
fn static_checks_inspect_inactive_branches_for_cycles_units_and_literals() {
    for cyclic_source in [
        "a = piecewise(a, false, 1);",
        "a = piecewise(b, false, 1); b = a;",
        "a = false && (a > 0);",
    ] {
        assert_eq!(
            compile_err_code(cyclic_source),
            DiagnosticCode::InitializationCycle,
            "expected InitializationCycle in inactive branch for {cyclic_source}"
        );
    }

    for unit_mismatch_source in [
        "unit u_m = metre; unit u_clock = second; a = 1; a has u_m; b = 2; b has u_clock; c = piecewise(a, true, b);",
        "unit u_m = metre; unit u_clock = second; a = 1; a has u_m; b = 2; b has u_clock; c = false && (a < b);",
        "unit u_m = metre; a = 1; a has u_m; c = piecewise(1, true, 2, a, 3);",
    ] {
        assert_eq!(
            compile_err_code(unit_mismatch_source),
            DiagnosticCode::UnitsInconsistency,
            "expected UnitsInconsistency in inactive branch for {unit_mismatch_source}"
        );
    }

    for invalid_literal_source in [
        "a = u || (1 / 0);",
        "a = piecewise(1, true, (1/0));",
        "a = piecewise(1, true, 1e309);",
        "a = false && ((1/0) > 0);",
    ] {
        let res = parsed(invalid_literal_source).and_then(|doc| analyze(&doc));
        assert!(
            res.is_err(),
            "expected static literal/rational restriction in inactive branch: {invalid_literal_source}"
        );
    }
}

#[test]
fn deferred_conditional_constructs_and_malformed_syntax_classifications() {
    for unsupported_source in [
        "unit u_s = second; a = 1; a has u_s;",
        "a = true and false;",
        "a = true or false;",
        "a = not false;",
        "a = 1 < 2 < 3;",
        "a = (1 < 2) < 3;",
        "a = 1 < 2 <= 3;",
        "a = 1 != 2 != 3;",
        "a = lt(1, 2) < 3;",
        "a = and(lt(1, 2), gt(3, 2)) < 4;",
        "a = +(1 < 2) < 3;",
        "a = -(-(1 < 2)) < 3;",
        "a = xor(true, false);",
        "a = implies(true, false);",
    ] {
        assert_eq!(
            compile_err_code(unsupported_source),
            DiagnosticCode::UnsupportedFeature,
            "expected UnsupportedFeature for {unsupported_source}"
        );
    }

    for syntax_err_source in [
        "a = (1 = 2);",
        "a = true & false;",
        "a = true | false;",
        "a = piecewise();",
        "a = not();",
        "a = not(true, false);",
        "a = lt(1);",
        "a = neq(1);",
        "a = neq(1, 2, 3);",
    ] {
        assert_eq!(
            compile_err_code(syntax_err_source),
            DiagnosticCode::Syntax,
            "expected Syntax error for {syntax_err_source}"
        );
    }
}

#[test]
fn named_and_unnamed_actual_species_reaction_rates_and_formatter_idempotence() {
    let source = "model conditional_kinetics_and_formatting\n  compartment cell = 1.5;\n  species Substrate in cell = piecewise((3/2), true, 0), Product in cell = 0;\n  const species Signal in cell = (1/2);\n  species Inhibitor in cell = 0.25;\n  k_fast = (4/5);\n  k_slow = (1/5);\n  v_uptake: Substrate => Product; cell * piecewise(k_fast * Substrate, (Signal >= (1/2)) && !(Inhibitor > 1), k_slow * Substrate);\n  Product =>; piecewise((1/2) * Product, eq(Product > 0, true), 0) + sin((1/4)) * 0;\n  keep_right_rel = 1 < (2 < 3);\n  keep_right_or = true || (false && false);\n  keep_not_rat = not((1/2));\n  keep_nested_and = and(and(true, false), true);\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_eq!(model.compartments()[0].id(), "cell");
    assert_eq!(model.compartments()[0].size(), Some(1.5));

    let substrate = model
        .species()
        .iter()
        .find(|s| s.id() == "Substrate")
        .unwrap();
    assert_approx(substrate.initial_concentration(), 1.5);
    assert!(!substrate.boundary_condition());

    let signal = model.species().iter().find(|s| s.id() == "Signal").unwrap();
    assert_approx(signal.initial_concentration(), 0.5);
    assert!(signal.boundary_condition());

    let v_uptake = model
        .reactions()
        .iter()
        .find(|r| r.id() == "v_uptake")
        .unwrap();
    assert_eq!(v_uptake.reactants()[0].species(), "Substrate");
    assert_eq!(v_uptake.products()[0].species(), "Product");
    // Modifier membership is set-like; canonical native ID ordering is not
    // the reference's source-encounter ordering.
    let mut modifiers = v_uptake.modifiers().to_vec();
    modifiers.sort();
    assert_eq!(modifiers, ["Inhibitor", "Signal"]);

    let decay = model.reactions().iter().find(|r| r.id() == "_J0").unwrap();
    assert_eq!(decay.reactants()[0].species(), "Product");
    assert!(decay.products().is_empty());
    assert!(decay.modifiers().is_empty());
    assert!(matches!(
        model.expression(decay.rate()).unwrap().kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Add,
            left,
            ..
        } if matches!(model.expression(*left).unwrap().kind(), ExpressionKind::Piecewise { .. })
    ));

    assert_approx(param_val(&model, "keep_right_rel"), 0.0);
    assert_approx(param_val(&model, "keep_right_or"), 1.0);
    assert_approx(param_val(&model, "keep_not_rat"), 0.0);
    assert_approx(param_val(&model, "keep_nested_and"), 0.0);
    assert!(
        !formatted.contains("not(1 / 2)") && !formatted.contains("not(1/2)"),
        "formatter must preserve grouping inside not((1/2)): {formatted}"
    );
    assert!(
        !formatted.contains(" and ") && !formatted.contains(" or "),
        "formatter must preserve symbolic/functional operators rather than textual aliases: {formatted}"
    );

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<modifierSpeciesReference species=\"Signal\"/>"));
        assert!(xml.contains("<modifierSpeciesReference species=\"Inhibitor\"/>"));
        assert!(xml.contains("<piecewise>"));
        assert!(xml.contains("<cn type=\"rational\">3<sep/>2</cn>"));
        assert!(xml.contains("<cn type=\"rational\">1<sep/>2</cn>"));
    }
}
