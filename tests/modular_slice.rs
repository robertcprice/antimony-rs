//! Integration tests for Phases 6–8 (`modular-core-v0.8-slice`):
//! - Phase 6: `substanceOnly` species, dynamic compartments (`=`, `:=`, `'=`),
//!   reaction kinetic-law replacements (`:=` and `=`).
//! - Phase 7: Discrete `at` events (`delay after`, `t0`, `persistent`, `fromTrigger`,
//!   `priority`, multiple assignments, `const` clearing, dimensional checks, negative rejections).
//! - Phase 8: `is` aliases, multi-module composition (`model sub(ports) ... end`),
//!   named (`M1: sub(...)`) and unnamed (`sub(...)` -> `_sys0_`) submodel instantiations,
//!   port binding and `M1.x` member overrides, nested submodels, and negative rejections.

use antimony::{
    Diagnostic, DiagnosticCode, ExpressionKind, IMPLEMENTED_PROFILE, Limits, ParsedDocument,
    RuleKind, SourceFile, SourceId, SymbolKind, ValidatedModel, analyze, format_antimony, parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(801), "modular_slice", text, &limits).unwrap();
    parse(&source, &limits)
}

fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    let doc = parsed(text)?;
    analyze(&doc)
}

fn compile_err_code(text: &str) -> DiagnosticCode {
    match parsed(text) {
        Err(diag) => diag.code(),
        Ok(doc) => analyze(&doc)
            .expect_err(&format!("expected compilation failure for:\n{text}"))
            .code(),
    }
}

fn check_roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let doc = parsed(text).expect("source should parse");
    let model = analyze(&doc).expect("source should analyze");
    let formatted = format_antimony(&doc).expect("source should format");

    let reparsed_source = SourceFile::new(
        SourceId::new(802),
        "modular_slice_formatted",
        &formatted,
        &limits,
    )
    .unwrap();
    let reparsed = parse(&reparsed_source, &limits).expect("formatted output should parse");
    let reanalyzed = analyze(&reparsed).expect("formatted output should analyze");
    let reformatted = format_antimony(&reparsed).expect("formatted output should reformat");

    assert_eq!(
        reformatted, formatted,
        "formatter must be byte-idempotent for:\n{text}"
    );
    assert_eq!(model.id(), reanalyzed.id());

    #[cfg(feature = "sbml")]
    {
        let xml1 = antimony::write_sbml(&model).unwrap().xml().to_owned();
        let xml2 = antimony::write_sbml(&reanalyzed).unwrap().xml().to_owned();
        assert_eq!(xml1, xml2, "SBML must be identical after format roundtrip");
    }

    (model, formatted)
}

#[test]
fn phase6_substance_only_species_ir_and_sbml_attributes() {
    let source = "\
model substance_only_demo
  compartment cell = 2.5;
  substanceOnly species S1 in cell = 10;
  substanceOnly species S2 in cell = 4 + 6;
  species S3 in cell = 5;
  const substanceOnly species S_ext in cell = 20;
  J0: S1 -> S2 + S3; 0.1 * S1;
end";
    let (model, formatted) = check_roundtrip(source);
    assert_eq!(IMPLEMENTED_PROFILE, "modular-core-v0.8-slice");
    assert_eq!(model.profile_id(), "modular-core-v0.8-slice");
    assert!(formatted.contains("substanceOnly species S1 in cell = 10;"));
    assert!(formatted.contains("const substanceOnly species S_ext in cell = 20;"));

    let s1 = model.species().iter().find(|s| s.id() == "S1").unwrap();
    assert!(s1.has_only_substance_units());
    assert_eq!(s1.initial_amount(), Some(10.0));
    assert_eq!(s1.initial_concentration(), None);
    assert_eq!(s1.initial_expression(), None);
    assert!(!s1.boundary_condition());

    let s2 = model.species().iter().find(|s| s.id() == "S2").unwrap();
    assert!(s2.has_only_substance_units());
    assert_eq!(s2.initial_amount(), Some(10.0));
    assert_eq!(s2.initial_concentration(), None);
    assert!(s2.initial_expression().is_some());

    let s3 = model.species().iter().find(|s| s.id() == "S3").unwrap();
    assert!(!s3.has_only_substance_units());
    assert_eq!(s3.initial_amount(), None);
    assert_eq!(s3.initial_concentration(), Some(5.0));

    let s_ext = model.species().iter().find(|s| s.id() == "S_ext").unwrap();
    assert!(s_ext.has_only_substance_units());
    assert!(s_ext.boundary_condition());
    assert_eq!(s_ext.initial_amount(), Some(20.0));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains(
            "<species id=\"S1\" compartment=\"cell\" hasOnlySubstanceUnits=\"true\" boundaryCondition=\"false\" constant=\"false\" initialAmount=\"10\"/>"
        ));
        assert!(xml.contains(
            "<species id=\"S2\" compartment=\"cell\" hasOnlySubstanceUnits=\"true\" boundaryCondition=\"false\" constant=\"false\"/>"
        ));
        assert!(xml.contains(
            "<species id=\"S3\" compartment=\"cell\" hasOnlySubstanceUnits=\"false\" boundaryCondition=\"false\" constant=\"false\" initialConcentration=\"5\"/>"
        ));
        assert!(xml.contains(
            "<species id=\"S_ext\" compartment=\"cell\" hasOnlySubstanceUnits=\"true\" boundaryCondition=\"true\" constant=\"false\" initialAmount=\"20\"/>"
        ));
    }
}

#[test]
fn phase6_substance_only_redeclaration_precedence_and_units() {
    // Redeclaring `species S` as `substanceOnly species S` promotes to substance-only,
    // while redeclaring `substanceOnly species S` as `species S` resets to concentration.
    let m1 =
        compile("compartment c = 2; species S in c = 4; substanceOnly species S in c;").unwrap();
    let s1 = &m1.species()[0];
    assert!(s1.has_only_substance_units());
    assert_eq!(s1.initial_amount(), Some(4.0));
    assert_eq!(s1.initial_concentration(), None);

    let m2 =
        compile("compartment c = 2; substanceOnly species S in c = 4; species S in c;").unwrap();
    let s2 = &m2.species()[0];
    assert!(!s2.has_only_substance_units());
    assert_eq!(s2.initial_amount(), None);
    assert_eq!(s2.initial_concentration(), Some(4.0));

    // Reaction participant reference does NOT clear `substanceOnly`.
    let m3 =
        compile("compartment c = 2; substanceOnly species S in c = 4; S -> ; 0.1 * S;").unwrap();
    assert!(m3.species()[0].has_only_substance_units());

    // Unit annotation on `substanceOnly species S; S has mole_u` sets substanceUnits directly
    // without synthesizing a `<mole>_per_<litre>` compound unit definition.
    let m4 = compile(
        "unit mole_u = mole; compartment c = 2; c has litre; substanceOnly species S in c = 5; S has mole_u;",
    )
    .unwrap();
    assert!(m4.species()[0].has_only_substance_units());
    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&m4).unwrap().xml().to_owned();
        assert!(xml.contains("substanceUnits=\"mole_u\""));
        assert!(!xml.contains("_per_"));
    }

    // Dimensional mismatch on substanceOnly species rejects with UnitsInconsistency.
    assert_eq!(
        compile_err_code("substanceOnly species S = 1; S has mole; k = 2; k has second; S := k;"),
        DiagnosticCode::UnitsInconsistency
    );
}

#[test]
fn phase6_dynamic_compartments_and_reaction_rate_overrides() {
    let source = "\
model dynamic_comp_and_rxn_override
  k0 = 1.5;
  compartment c_init = 2 * k0;
  compartment c_assign = 1.0;
  compartment c_rate = 2.0;
  c_assign := 1.0 + 0.2 * time;
  c_rate' = 0.05 * c_rate;
  species A in c_init = 10;
  species B in c_assign = 0;
  k1 = 0.1;
  k2 = 0.4;
  J0: A -> B; k1 * A;
  J0 := k2 * A;
end";
    let (model, _) = check_roundtrip(source);

    let c_init = model
        .compartments()
        .iter()
        .find(|c| c.id() == "c_init")
        .unwrap();
    assert_eq!(c_init.size(), Some(3.0));
    assert!(c_init.initial_expression().is_some());
    assert!(c_init.constant());

    let c_assign = model
        .compartments()
        .iter()
        .find(|c| c.id() == "c_assign")
        .unwrap();
    assert_eq!(c_assign.size(), None);
    assert!(!c_assign.constant());

    let c_rate = model
        .compartments()
        .iter()
        .find(|c| c.id() == "c_rate")
        .unwrap();
    assert_eq!(c_rate.size(), Some(2.0));
    assert!(!c_rate.constant());

    // Reaction J0 rate law was replaced by `J0 := k2 * A;` and does not emit an SBML rule.
    assert_eq!(model.rules().len(), 2);
    assert_eq!(model.rules()[0].target(), "c_assign");
    assert_eq!(model.rules()[0].kind(), RuleKind::Assignment);
    assert_eq!(model.rules()[1].target(), "c_rate");
    assert_eq!(model.rules()[1].kind(), RuleKind::Rate);

    let j0_rate = model.expression(model.reactions()[0].rate()).unwrap();
    let ExpressionKind::Binary { left, .. } = j0_rate.kind() else {
        panic!("expected binary multiply rate law");
    };
    assert!(matches!(
        model.expression(*left).unwrap().kind(),
        ExpressionKind::Symbol {
            name,
            kind: SymbolKind::Parameter
        } if name == "k2"
    ));

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(
            xml.contains("<compartment id=\"c_init\" spatialDimensions=\"3\" constant=\"true\"/>")
        );
        assert!(
            xml.contains(
                "<compartment id=\"c_assign\" spatialDimensions=\"3\" constant=\"false\"/>"
            )
        );
        assert!(xml.contains(
            "<compartment id=\"c_rate\" size=\"2\" spatialDimensions=\"3\" constant=\"false\"/>"
        ));
        assert!(xml.contains("<initialAssignment symbol=\"c_init\">"));
        assert!(xml.contains("<assignmentRule variable=\"c_assign\">"));
        assert!(xml.contains("<rateRule variable=\"c_rate\">"));
        assert!(!xml.contains("variable=\"J0\""));
    }
}

#[test]
fn phase7_discrete_events_options_and_sbml_serialization() {
    let source = "\
model discrete_events_demo
  compartment cell = 1;
  species S in cell = 10;
  species P in cell = 0;
  const k = 0.2;
  d = 1.5;
  J0: S -> P; k * S;
  E_dose: at d after (time >= 5), t0 = false, persistent = false, fromTrigger = false, priority = 2 * k: S = S + 5, k = 0.5;
  at (S < 2): P = P + 1;
end";
    let (model, formatted) = check_roundtrip(source);
    assert!(formatted.contains(
        "E_dose: at d after (time >= 5), t0 = false, persistent = false, fromTrigger = false, priority = 2 * k: S = S + 5, k = 0.5;"
    ));
    assert!(formatted.contains("at (S < 2): P = P + 1;"));

    assert_eq!(model.events().len(), 2);
    let e0 = &model.events()[0];
    assert_eq!(e0.id(), "E_dose");
    assert!(!e0.use_values_from_trigger_time());
    assert!(!e0.trigger().initial_value());
    assert!(!e0.trigger().persistent());
    assert!(e0.delay().is_some());
    assert!(e0.priority().is_some());
    assert_eq!(e0.assignments().len(), 2);
    assert_eq!(e0.assignments()[0].variable(), "S");
    assert_eq!(e0.assignments()[1].variable(), "k");

    // `const k = 0.2` followed by event assignment `k = 0.5` clears `constant` on `k`.
    let k_param = model.parameters().iter().find(|p| p.id() == "k").unwrap();
    assert!(!k_param.constant());

    let e1 = &model.events()[1];
    assert_eq!(e1.id(), "_E0");
    assert!(e1.use_values_from_trigger_time());
    assert!(e1.trigger().initial_value());
    assert!(e1.trigger().persistent());
    assert!(e1.delay().is_none());
    assert!(e1.priority().is_none());

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<listOfEvents>"));
        assert!(xml.contains("<event id=\"E_dose\" useValuesFromTriggerTime=\"false\">"));
        assert!(xml.contains("<trigger initialValue=\"false\" persistent=\"false\">"));
        assert!(xml.contains("<delay>"));
        assert!(xml.contains("<priority>"));
        assert!(xml.contains("<listOfEventAssignments>"));
        assert!(xml.contains("<eventAssignment variable=\"S\">"));
        assert!(xml.contains("<eventAssignment variable=\"k\">"));
        // Verify child element ordering: <trigger> before <delay> before <priority> before <listOfEventAssignments>
        let trig_pos = xml.find("<trigger initialValue=\"false\"").unwrap();
        let delay_pos = xml.find("<delay>").unwrap();
        let prio_pos = xml.find("<priority>").unwrap();
        let list_pos = xml.find("<listOfEventAssignments>").unwrap();
        assert!(trig_pos < delay_pos && delay_pos < prio_pos && prio_pos < list_pos);
    }
}

#[test]
fn phase8_submodels_ports_member_overrides_aliases_and_flattening() {
    let source = "\
function hill_rate(S, Vm, Km)
  Vm * S / (Km + S)
end
model enzyme_step(S, P, kcat)
  compartment c = 1;
  species S in c = 10;
  species P in c = 0;
  kcat = 0.25;
  Km = 2.0;
  J0: S -> P; hill_rate(S, kcat, Km);
  E0: at (time > 4): kcat = 0.5;
end
model *cascade()
  compartment reactor = 2;
  species A in reactor = 20;
  species B in reactor = 0;
  species C in reactor = 0;
  k_fast = 0.8;
  M1: enzyme_step(A, B, k_fast);
  M2: enzyme_step(B, C, k_slow);
  M2.Km = 0.5;
  k_alias is M1.Km;
  k_alias = 1.2;
end";
    let (model, formatted) = check_roundtrip(source);
    assert_eq!(model.id(), "cascade");
    assert!(formatted.contains("model enzyme_step(S, P, kcat)"));
    assert!(formatted.contains("model *cascade()"));
    assert!(formatted.contains("M1: enzyme_step(A, B, k_fast);"));
    assert!(formatted.contains("M2.Km = 0.5;"));
    assert!(formatted.contains("k_alias is M1.Km;"));

    // Species A, B, C remain in caller's compartment `reactor` and keep caller's initial values.
    for (id, expected_conc) in [("A", 20.0), ("B", 0.0), ("C", 0.0)] {
        let sp = model.species().iter().find(|s| s.id() == id).unwrap();
        assert_eq!(sp.compartment(), "reactor");
        assert_eq!(sp.initial_concentration(), Some(expected_conc));
    }

    // M1_Km was aliased to `k_alias` and overridden to 1.2; M2_Km was overridden via `M2.Km = 0.5`.
    let k_alias = model
        .parameters()
        .iter()
        .find(|p| p.id() == "k_alias")
        .unwrap();
    assert_eq!(k_alias.value(), Some(1.2));
    let m2_km = model
        .parameters()
        .iter()
        .find(|p| p.id() == "M2_Km")
        .unwrap();
    assert_eq!(m2_km.value(), Some(0.5));
    let k_slow = model
        .parameters()
        .iter()
        .find(|p| p.id() == "k_slow")
        .unwrap();
    assert_eq!(k_slow.value(), Some(0.25));

    // Flattened reactions M1_J0 and M2_J0 and events M1_E0 and M2_E0 exist.
    assert_eq!(
        model.reactions().iter().map(|r| r.id()).collect::<Vec<_>>(),
        vec!["M1_J0", "M2_J0"]
    );
    assert_eq!(
        model.events().iter().map(|e| e.id()).collect::<Vec<_>>(),
        vec!["M1_E0", "M2_E0"]
    );
}

#[test]
fn phase8_unnamed_submodels_and_nested_hierarchy() {
    let source = "\
model leaf(x)
  k = 2;
   -> x; k;
end
model mid(y)
  L: leaf(y);
  L.k = 5;
end
model *top()
  species S = 1;
  mid(S);
end";
    let (model, _) = check_roundtrip(source);
    assert_eq!(model.id(), "top");
    assert_eq!(model.reactions()[0].id(), "_sys0_L__J0");
    let k_param = model
        .parameters()
        .iter()
        .find(|p| p.id() == "_sys0_L_k")
        .unwrap();
    assert_eq!(k_param.value(), Some(5.0));
}

#[test]
fn phase6_to_8_negative_rejections() {
    // Phase 6 negatives
    assert_eq!(
        compile_err_code("compartment c = 1 - 5;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("J0' = 0.2 * A; A = 1; B = 0; J0: A -> B; 0.1 * A;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("A = 1; B = 0; J0: A -> B; 0.1 * A; J0' = 0.2;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("substanceOnly compartment c = 1;"),
        DiagnosticCode::Syntax
    );

    // Phase 7 negatives
    assert_eq!(
        compile_err_code("S = 1; at (time + 1): S = 0;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("S = 1; at (time > 1), priority = (time > 0): S = 0;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("S = 1; at (time > 1), delay = 2: S = 0;"),
        DiagnosticCode::Syntax
    );
    assert_eq!(
        compile_err_code("S = 1; at (time > 1): S = 0, S = 2;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("S = 1; S := 2; at (time > 1): S = 3;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("at (time > 1): k = 2; const k = 1;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code("S = 1; J0: S -> ; 0.1 * S; at (time > 1): J0 = 0;"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code(
            "unit time_unit = second; k = 1; k has metre; at k after (time > 1): k = 2;"
        ),
        DiagnosticCode::UnitsInconsistency
    );

    // Phase 8 negatives
    assert_eq!(
        compile_err_code("model sub(x, y)\n  x = 1; y = 2;\nend\nmodel *top()\n  M1: sub(a);\nend"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code(
            "model sub(x)\n  x = 1;\nend\nmodel *top()\n  M1: sub(a);\n  M1.missing = 2;\nend"
        ),
        DiagnosticCode::UnresolvedSymbol
    );
    assert_eq!(
        compile_err_code(
            "model sub(x)\n  y = 2;\n  x is y;\nend\nmodel *top()\n  M1: sub(a);\nend"
        ),
        DiagnosticCode::UnsupportedFeature
    );
    assert_eq!(
        compile_err_code(
            "model sub()\n  unit u = litre;\n  x = 1 has u;\nend\nmodel *top()\n  M1: sub();\nend"
        ),
        DiagnosticCode::UnsupportedFeature
    );
}

#[test]
fn phase9_module_keyword_deletions_and_conversion_factors() {
    let source = "\
module sub_pathway(S_in)
  compartment c = 1;
  species S_in in c = 10;
  species P in c = 0;
  species Q in c = 0;
  k1 = 0.2;
  k2 = 0.05;
  x_rate = 1.0;
  x_rate' = k1 * time;
  J0: S_in -> P + Q; k1 * S_in;
  J1: P -> Q; k2 * P;
  E_unused: at (time > 3): Q = 0;
  E_active: at 2 after (time > 5): P = time;
end
module *main_pathway()
  compartment c = 1;
  species A in c = 15;
  tf = 60;
  ef = 1000;
  M1: sub_pathway(A), timeconv = tf, extentconv = ef;
  delete M1.J1, M1.E_unused, M1.Q, M1.k2;
end";
    let (model, formatted) = check_roundtrip(source);
    assert_eq!(model.id(), "main_pathway");
    assert!(formatted.contains("module sub_pathway(S_in)"));
    assert!(formatted.contains("module *main_pathway()"));
    assert!(formatted.contains("M1: sub_pathway(A), timeconv = tf, extentconv = ef;"));
    assert!(formatted.contains("delete M1.J1, M1.E_unused, M1.Q, M1.k2;"));

    // Deleted species M1_Q, parameter M1_k2, reaction M1_J1, and event M1_E_unused are absent.
    let sp_ids: Vec<&str> = model.species().iter().map(|s| s.id()).collect();
    assert_eq!(sp_ids, vec!["A", "M1_P"]);
    let param_ids: Vec<&str> = model.parameters().iter().map(|p| p.id()).collect();
    assert!(!param_ids.contains(&"M1_k2"));
    assert!(param_ids.contains(&"tf"));
    assert!(param_ids.contains(&"ef"));

    // Reaction M1_J0 had deleted species M1_Q removed from its products, leaving only M1_P.
    assert_eq!(model.reactions().len(), 1);
    assert_eq!(model.reactions()[0].id(), "M1_J0");
    assert_eq!(model.reactions()[0].products().len(), 1);
    assert_eq!(model.reactions()[0].products()[0].species(), "M1_P");

    // Only M1_E_active remains.
    assert_eq!(model.events().len(), 1);
    assert_eq!(model.events()[0].id(), "M1_E_active");

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(!xml.contains("M1_Q"));
        assert!(!xml.contains("M1_k2"));
        assert!(!xml.contains("M1_J1"));
        assert!(!xml.contains("M1_E_unused"));
        assert!(xml.contains("<ci>tf</ci>"));
        assert!(xml.contains("<ci>ef</ci>"));
    }

    // Negative checks for `delete` and `timeconv`/`extentconv`
    assert_eq!(
        compile_err_code("model *main()\n  x = 1;\n  delete x;\nend"),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code(
            "model sub(S)\n  species S = 1;\nend\nmodel *main()\n  species A = 2;\n  M1: sub(A);\n  delete M1.S;\nend"
        ),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code(
            "model sub()\n  S -> P; k1 * S;\n  k1 = 0.1; S = 10; P = 0;\nend\nmodel *main()\n  M1: sub();\n  delete M1.k1;\nend"
        ),
        DiagnosticCode::DeclarationConflict
    );
    assert_eq!(
        compile_err_code(
            "model sub()\n  S -> P; k1 * S;\n  k1 = 0.1; S = 10; P = 0;\nend\nmodel *main()\n  M1: sub(), timeconv = 60;\nend"
        ),
        DiagnosticCode::UnsupportedFeature
    );
    assert_eq!(
        compile_err_code(
            "model sub()\n  S -> P; k1 * S;\n  k1 = 0.1; S = 10; P = 0;\nend\nmodel *main()\n  species tf = 60;\n  M1: sub(), timeconv = tf;\nend"
        ),
        DiagnosticCode::DeclarationConflict
    );
}
