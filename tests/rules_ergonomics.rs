//! Ordinary authoring and adversarial integration tests for `rules-core-v0.4-slice`:
//! public `Rule` / `RuleKind` IR, lexical scalar-export rules, target first-seen ordering,
//! same-kind replacement retaining inferred symbols, transient dependency cycle rejection,
//! source-order species boundary promotion, conservative unit/dimension checks,
//! explicit unsupported-feature diagnostics, and formatter/SBML roundtrip idempotence.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionKind, IMPLEMENTED_PROFILE, Limits,
    ParsedDocument, RuleKind, SourceFile, SourceId, ValidatedModel, analyze, format_antimony,
    parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(601), "rules_ergonomics", text, &limits).unwrap();
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

fn check_roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let doc = parsed(text).expect("source should parse");
    let model = analyze(&doc).expect("source should analyze");
    let formatted = format_antimony(&doc).expect("source should format");

    let reparsed_source = SourceFile::new(
        SourceId::new(602),
        "rules_ergonomics_formatted",
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
    let snap_rules = |m: &ValidatedModel| {
        m.rules()
            .iter()
            .map(|r| {
                (
                    r.kind(),
                    r.target().to_owned(),
                    format!("{:?}", m.expression(r.expression()).unwrap().kind()),
                )
            })
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
    assert_eq!(snap_rules(&model), snap_rules(&reanalyzed));
    assert_eq!(snap_inits(&model), snap_inits(&reanalyzed));

    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&model).unwrap().xml(),
        antimony::write_sbml(&reanalyzed).unwrap().xml()
    );

    (model, formatted)
}

#[test]
fn public_rule_ir_and_first_seen_target_ordering() {
    let source = "\
model rule_ir_demo
  z = 10;
  a := 2.5;
  m' = -0.2 * z;
  z := a + 1;
end";
    let (model, formatted) = check_roundtrip(source);
    assert_eq!(IMPLEMENTED_PROFILE, "modular-core-v0.8-slice");
    assert_eq!(model.profile_id(), "modular-core-v0.8-slice");
    assert!(formatted.contains("a := 2.5;"));
    assert!(formatted.contains("m' = -0.2 * z;"));
    assert!(formatted.contains("z := a + 1;"));

    let rules = model.rules();
    assert_eq!(rules.len(), 3);
    assert_eq!(
        rules
            .iter()
            .map(|r| (r.kind(), r.target()))
            .collect::<Vec<_>>(),
        vec![
            (RuleKind::Assignment, "z"),
            (RuleKind::Assignment, "a"),
            (RuleKind::Rate, "m"),
        ]
    );
    for rule in rules {
        assert!(!rule.source_span().is_empty());
        assert!(model.expression(rule.expression()).is_some());
    }
    // Assignment rule on z cleared its prior `z = 10` initializer.
    assert!(model.initial_assignments().is_empty());
}

#[test]
fn lexical_scalar_export_contract_for_parameters_and_species() {
    let source = "\
model lexical_scalars
  p_pos := 2.5;
  p_neg := -2;
  p_group := (2.5);
  p_plus := +2.5;
  p_rat := (1/2);
  p_bool := true;
  p_pi := pi;
  p_expr := p_pos + 1;
  species s_pos := 3.25;
  species s_zero := -0;
  species s_group := (3.25);
end";
    let (model, _) = check_roundtrip(source);

    let param_val = |id: &str| {
        model
            .parameters()
            .iter()
            .find(|p| p.id() == id)
            .unwrap()
            .value()
    };
    let param_const = |id: &str| {
        model
            .parameters()
            .iter()
            .find(|p| p.id() == id)
            .unwrap()
            .constant()
    };
    assert_eq!(param_val("p_pos"), Some(2.5));
    assert_eq!(param_val("p_neg"), Some(-2.0));
    assert_eq!(param_val("p_group"), None);
    assert_eq!(param_val("p_plus"), None);
    assert_eq!(param_val("p_rat"), None);
    assert_eq!(param_val("p_bool"), None);
    assert_eq!(param_val("p_pi"), None);
    assert_eq!(param_val("p_expr"), None);

    for id in [
        "p_pos", "p_neg", "p_group", "p_plus", "p_rat", "p_bool", "p_pi", "p_expr",
    ] {
        assert!(
            !param_const(id),
            "rule target {id} must be non-constant in SBML"
        );
    }

    let species_conc = |id: &str| {
        model
            .species()
            .iter()
            .find(|s| s.id() == id)
            .unwrap()
            .initial_concentration()
    };
    assert_eq!(species_conc("s_pos"), Some(3.25));
    assert_eq!(species_conc("s_zero"), Some(0.0));
    assert_eq!(species_conc("s_group"), None);
    // Negative species concentration in an assignment rule rejects.
    assert_eq!(
        compile_err_code("species s_neg := -0.5;"),
        DiagnosticCode::DeclarationConflict
    );
}

#[test]
fn source_order_species_boundary_promotion_and_stoichiometry_guard() {
    // 1. Inferred rule target later promoted to `species` becomes boundaryCondition=true
    //    and may participate in reaction stoichiometry.
    let (promoted, _) = check_roundtrip("a := 2; species a; P = 0; J: a -> P; 0.1 * a;");
    let sp_a = promoted.species().iter().find(|s| s.id() == "a").unwrap();
    assert!(sp_a.boundary_condition());
    assert_eq!(sp_a.initial_concentration(), Some(2.0));

    // 2. `const` or `$` before/after rule promotes species to boundaryCondition=true.
    let (const_after, _) =
        check_roundtrip("species S; S := 2; const S; P = 0; J: S -> P; 0.1 * S;");
    assert!(
        const_after
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .boundary_condition()
    );

    let (dollar_inline, _) = check_roundtrip("species $S := 2; P = 0; J: S -> P; 0.1 * S;");
    assert!(
        dollar_inline
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .boundary_condition()
    );

    // 3. `a := 2; species a; var species a;` clears boundary promotion back to floating.
    let (var_after, _) = check_roundtrip("a := 2; species a; var species a;");
    assert!(
        !var_after
            .species()
            .iter()
            .find(|s| s.id() == "a")
            .unwrap()
            .boundary_condition()
    );
    assert_eq!(
        compile_err_code("a := 2; species a; var species a; P = 0; J: a -> P; 0.1 * a;"),
        DiagnosticCode::DeclarationConflict
    );

    // 4. Inline `species S := 2;` promotes S to boundaryCondition=true (matching upstream),
    //    whereas pre-declared floating `species S; S := 2;` stays boundaryCondition=false
    //    and may appear as a reaction modifier, but is rejected in stoichiometry.
    let (inline_species, _) = check_roundtrip("species S := 2; P = 0; J: S -> P; 0.1 * S;");
    assert!(
        inline_species
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .boundary_condition()
    );

    let (modifier_ok, _) = check_roundtrip("species S; S := 2; A = 1; B = 0; J: A -> B; S * A;");
    assert!(
        !modifier_ok
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .boundary_condition()
    );
    assert_eq!(modifier_ok.reactions()[0].modifiers(), &["S".to_owned()]);

    assert_eq!(
        compile_err_code("species S; S := 2; P = 0; J: S -> P; 0.1 * S;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("species S; S' = 0.5; P = 0; J: P -> S; 0.1 * P;"),
        DiagnosticCode::DeclarationConflict
    );
}

#[test]
fn replacement_retains_inferred_symbols_and_rate_keeps_initial_value() {
    let source = "\
model replacement_and_rate_init
  b := 1;
  a := inferred_u + 1;
  b := 2;
  a := 3;
  x = 4;
  k = 0.5;
  x' = -k * x;
  y' = 1;
  y = x + 2;
end";
    let (model, _) = check_roundtrip(source);
    assert_eq!(
        model.rules().iter().map(|r| r.target()).collect::<Vec<_>>(),
        vec!["b", "a", "x", "y"]
    );
    // `inferred_u` remains in the parameter list even after `a := 3` replaces `a := inferred_u + 1`.
    let param = |id: &str| model.parameters().iter().find(|p| p.id() == id).unwrap();
    assert_eq!(param("inferred_u").value(), None);
    assert!(param("inferred_u").constant());
    assert_eq!(param("b").value(), Some(2.0));
    assert_eq!(param("a").value(), Some(3.0));
    assert_eq!(param("x").value(), Some(4.0));
    assert!(!param("x").constant());
    assert_eq!(param("y").value(), Some(6.0));
    assert!(!param("y").constant());
    assert_eq!(model.initial_assignments().len(), 1);
    assert_eq!(model.initial_assignments()[0].symbol(), "y");

    // Mixed rule kinds and ordinary `=` after `:=` reject.
    assert_eq!(
        compile_err_code("a := 1; a' = 2;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("a' = 1; a := 2;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("a := 1; a = 2;"),
        DiagnosticCode::DeclarationConflict
    );
}

#[test]
fn transient_and_active_dependency_cycles_reject_while_rate_feedback_succeeds() {
    // Direct and mutual assignment cycles reject.
    assert_eq!(
        compile_err_code("a := a;"),
        DiagnosticCode::InitializationCycle
    );
    assert_eq!(
        compile_err_code("a := b; b := a;"),
        DiagnosticCode::InitializationCycle
    );
    // Mixed `=` and `:=` cycle rejects.
    assert_eq!(
        compile_err_code("a = b + 1; b := a + 1;"),
        DiagnosticCode::InitializationCycle
    );
    // Transient cycle rejects immediately even when a later replacement would break the cycle.
    assert_eq!(
        compile_err_code("a := b; b := a; a := 1;"),
        DiagnosticCode::InitializationCycle
    );
    assert_eq!(
        compile_err_code("a = b; b := a; a = 1;"),
        DiagnosticCode::InitializationCycle
    );
    // Inactive branch cycle in assignment rule still rejects statically.
    assert_eq!(
        compile_err_code("a := piecewise(a, false, 1);"),
        DiagnosticCode::InitializationCycle
    );

    // Rate rules represent dx/dt, so self-feedback and mutual feedback are valid ODEs,
    // and an assignment rule depending on a rate-rule target is valid.
    let (ode, _) = check_roundtrip("x = 1; y = 0; z := x + y; x' = -y + z; y' = x;");
    assert_eq!(ode.rules().len(), 3);
}

#[test]
fn deep_assignment_rule_chain_succeeds_without_recursion() {
    let mut lines = vec!["model deep_chain".to_owned(), "  x0 = 1;".to_owned()];
    for i in 1..=64 {
        lines.push(format!("  x{i} := x{};", i - 1));
    }
    lines.push("end".to_owned());
    let text = lines.join("\n");
    let (model, _) = check_roundtrip(&text);
    assert_eq!(model.rules().len(), 64);
}

#[test]
fn unit_and_dimension_checking_for_assignment_and_rate_rules() {
    let valid = "\
model rule_units_ok
  unit time_unit = second;
  unit substance = mole;
  unit volume = litre;
  unit u_rate = mole / litre / second;
  species S = 2;
  k = 0.5;
  k has u_rate;
  S' = k;
  rate_copy = 0.5;
  rate_copy has u_rate;
  rate_copy := k;
end";
    let (model, _) = check_roundtrip(valid);
    assert!(matches!(
        model.symbol_dimensions("S"),
        Some(DimensionalStatus::Known(_))
    ));

    // Assignment rule with mismatched known dimensions rejects.
    assert_eq!(
        compile_err_code(
            "unit time_unit = second; a = 1; a has mole; b = 2; b has second; a := b;"
        ),
        DiagnosticCode::UnitsInconsistency
    );
    // Rate rule whose RHS does not match target / time_unit rejects.
    assert_eq!(
        compile_err_code(
            "unit time_unit = second; unit u_clock = second; a = 1; a has u_clock; a' = a;"
        ),
        DiagnosticCode::UnitsInconsistency
    );
}

#[test]
fn explicit_rejections_for_unsupported_or_invalid_rule_constructs() {
    // Empty rules (explicit first-slice UnsupportedFeature, never silently discarded)
    assert_eq!(
        compile_err_code("a := ;"),
        DiagnosticCode::UnsupportedFeature
    );
    assert_eq!(
        compile_err_code("a' = ;"),
        DiagnosticCode::UnsupportedFeature
    );
    assert_eq!(
        compile_err_code("a := ;\nb = 1;"),
        DiagnosticCode::UnsupportedFeature
    );

    // Algebraic rule syntax
    assert_eq!(
        compile_err_code("0 = x + y;"),
        DiagnosticCode::UnsupportedFeature
    );

    // Compartment rules and reaction kinetic-law overrides are now supported in modular-core-v0.8-slice
    for src in [
        "compartment C = 1; C := 2;",
        "compartment C = 1; C' = 0.1;",
        "A = 1; B = 0; J: A -> B; 0.1 * A; J := 0.2 * A;",
        "J := 0.2 * A; A = 1; B = 0; J: A -> B; 0.1 * A;",
    ] {
        let _ = check_roundtrip(src);
    }
    assert_eq!(
        compile_err_code("J' = 0.2 * A; A = 1; B = 0; J: A -> B; 0.1 * A;"),
        DiagnosticCode::DeclarationConflict
    );

    // Unit definition targets
    assert_eq!(
        compile_err_code("unit u = 1 second; u := 2;"),
        DiagnosticCode::DeclarationConflict
    );

    // Reserved builtin targets
    assert_eq!(
        compile_err_code("time := 1;"),
        DiagnosticCode::UnsupportedFeature
    );
    assert_eq!(
        compile_err_code("time' = 1;"),
        DiagnosticCode::UnsupportedFeature
    );

    // Prime inside expression
    assert_eq!(compile_err_code("a = b';"), DiagnosticCode::Syntax);
}

#[test]
fn ordinary_user_biological_models_with_rules_and_reactions() {
    let source = "\
model goodwin_with_pool_and_forcing
  compartment cell = 1;
  species X in cell = 1.0;
  species Y in cell = 0.5;
  species $Drive in cell;
  k_syn = 1.2;
  k_deg = 0.4;
  k_trans = 0.6;
  Drive := piecewise(1.0 + 0.5 * sin(time), time >= 0, 1.0);
  total_pool := X + Y;
  X' = k_syn * Drive / (1 + Y^2) - k_deg * X;
  J_trans: -> Y; k_trans * X;
end";
    let (model, _) = check_roundtrip(source);
    // Rules are ordered by target first-seen order: X, Drive, total_pool.
    assert_eq!(
        model
            .rules()
            .iter()
            .map(|r| (r.kind(), r.target()))
            .collect::<Vec<_>>(),
        vec![
            (RuleKind::Rate, "X"),
            (RuleKind::Assignment, "Drive"),
            (RuleKind::Assignment, "total_pool"),
        ]
    );
    assert_eq!(model.reactions()[0].modifiers(), &["X".to_owned()]);
    assert!(matches!(
        model
            .expression(model.rules()[1].expression())
            .unwrap()
            .kind(),
        ExpressionKind::Piecewise { .. }
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<listOfRules>"));
        assert!(xml.contains("<assignmentRule variable=\"Drive\">"));
        assert!(xml.contains("<assignmentRule variable=\"total_pool\">"));
        assert!(xml.contains("<rateRule variable=\"X\">"));
    }
}
