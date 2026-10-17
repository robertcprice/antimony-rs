//! Ordinary ergonomic declarations, unnamed reactions, qualifier resolution,
//! formula-to-species promotion, and unset/deferred initializations.
use antimony::{
    Diagnostic, ExpressionKind, Limits, ParsedDocument, SourceFile, SourceId, SymbolKind,
    ValidatedModel, analyze, format_antimony, parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source =
        SourceFile::new(SourceId::new(202), "declarations_ergonomics", text, &limits).unwrap();
    parse(&source, &limits)
}

fn check_roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let doc = parsed(text).expect("source should parse");
    let model = analyze(&doc).expect("source should analyze");
    let formatted = format_antimony(&doc).expect("source should format");

    let reparsed_source = SourceFile::new(
        SourceId::new(203),
        "declarations_ergonomics_formatted",
        &formatted,
        &limits,
    )
    .unwrap();
    let reparsed = parse(&reparsed_source, &limits).expect("formatted output should parse");
    let reanalyzed = analyze(&reparsed).expect("formatted output should analyze");
    let reformatted = format_antimony(&reparsed).expect("formatted output should reformat");

    assert_eq!(reformatted, formatted, "formatter must be byte-idempotent");
    assert_eq!(model.id(), reanalyzed.id());
    assert_eq!(
        model
            .compartments()
            .iter()
            .map(|c| (c.id(), c.size(), c.is_implicit()))
            .collect::<Vec<_>>(),
        reanalyzed
            .compartments()
            .iter()
            .map(|c| (c.id(), c.size(), c.is_implicit()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        model
            .species()
            .iter()
            .map(|s| (
                s.id(),
                s.compartment(),
                s.initial_concentration(),
                s.initial_expression().is_some(),
                s.boundary_condition(),
                s.constant()
            ))
            .collect::<Vec<_>>(),
        reanalyzed
            .species()
            .iter()
            .map(|s| (
                s.id(),
                s.compartment(),
                s.initial_concentration(),
                s.initial_expression().is_some(),
                s.boundary_condition(),
                s.constant()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (
                p.id(),
                p.value(),
                p.initial_expression().is_some(),
                p.constant()
            ))
            .collect::<Vec<_>>(),
        reanalyzed
            .parameters()
            .iter()
            .map(|p| (
                p.id(),
                p.value(),
                p.initial_expression().is_some(),
                p.constant()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        model
            .reactions()
            .iter()
            .map(|r| (
                r.id(),
                r.reversible(),
                r.reactants().len(),
                r.products().len()
            ))
            .collect::<Vec<_>>(),
        reanalyzed
            .reactions()
            .iter()
            .map(|r| (
                r.id(),
                r.reversible(),
                r.reactants().len(),
                r.products().len()
            ))
            .collect::<Vec<_>>()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&model).unwrap().xml(),
        antimony::write_sbml(&reanalyzed).unwrap().xml()
    );

    (model, formatted)
}

#[test]
fn unnamed_conversion_chain_preserves_generated_ids_and_anonymous_formatting() {
    let source = "model pathway\n  Glucose = 5; G6P = 0; F6P = 0;\n  k1 = 0.2; k2 = 0.1; kr2 = 0.05;\n  Glucose => G6P; k1 * Glucose;\n  G6P -> F6P; k2 * G6P - kr2 * F6P;\nend";
    let (model, formatted) = check_roundtrip(source);

    assert_eq!(
        formatted,
        "model pathway\n  Glucose = 5;\n  G6P = 0;\n  F6P = 0;\n  k1 = 0.2;\n  k2 = 0.1;\n  kr2 = 0.05;\n  Glucose => G6P; k1 * Glucose;\n  G6P -> F6P; k2 * G6P - kr2 * F6P;\nend\n"
    );
    assert_eq!(
        model
            .reactions()
            .iter()
            .map(|r| (
                r.id(),
                r.reversible(),
                r.reactants()[0].species(),
                r.products()[0].species()
            ))
            .collect::<Vec<_>>(),
        [
            ("_J0", false, "Glucose", "G6P"),
            ("_J1", true, "G6P", "F6P"),
        ]
    );
}

#[test]
fn unnamed_source_and_sink_reactions_with_boundary_and_stoichiometry() {
    let source = "S = 2; Feed = 10; k_in = 1.5; k_deg = 0.25; k_feed = 0.05; => S; k_in; S =>; k_deg * S; 2 $Feed => S; k_feed * Feed;";
    let (model, formatted) = check_roundtrip(source);

    assert_eq!(
        formatted,
        "S = 2;\nFeed = 10;\nk_in = 1.5;\nk_deg = 0.25;\nk_feed = 0.05;\n=> S; k_in;\nS =>; k_deg * S;\n2 $Feed => S; k_feed * Feed;\n"
    );
    let r0 = &model.reactions()[0];
    assert_eq!(r0.id(), "_J0");
    assert!(r0.reactants().is_empty());
    assert_eq!(r0.products()[0].species(), "S");
    assert_eq!(r0.products()[0].stoichiometry(), 1.0);

    let r1 = &model.reactions()[1];
    assert_eq!(r1.id(), "_J1");
    assert_eq!(r1.reactants()[0].species(), "S");
    assert!(r1.products().is_empty());

    let r2 = &model.reactions()[2];
    assert_eq!(r2.id(), "_J2");
    assert_eq!(r2.reactants()[0].species(), "Feed");
    assert_eq!(r2.reactants()[0].stoichiometry(), 2.0);
    assert_eq!(r2.products()[0].species(), "S");

    let feed = model.species().iter().find(|s| s.id() == "Feed").unwrap();
    assert!(feed.boundary_condition());
    assert_eq!(feed.initial_concentration(), Some(10.0));
}

#[test]
fn mixed_named_and_unnamed_reactions_assign_sequential_generated_ids() {
    let source = "A = 4; B = 0; C = 0; D = 0; k1 = 0.3; k2 = 0.2; k3 = 0.1; A => B; k1 * A; committed: B => C; k2 * B; C => D; k3 * C;";
    let (model, formatted) = check_roundtrip(source);

    assert!(formatted.contains("A => B; k1 * A;\n"));
    assert!(formatted.contains("committed: B => C; k2 * B;\n"));
    assert!(formatted.contains("C => D; k3 * C;\n"));

    assert_eq!(
        model
            .reactions()
            .iter()
            .map(|r| (
                r.id(),
                r.reactants()[0].species(),
                r.products()[0].species()
            ))
            .collect::<Vec<_>>(),
        [
            ("_J0", "A", "B"),
            ("_J1", "C", "D"),
            ("committed", "B", "C"),
        ]
    );
}

#[test]
fn unnamed_reaction_respects_earlier_symbol_and_allows_later_rate_override() {
    let (reserved_before, _) = check_roundtrip("A = 2; B = 0; k = 0.1; _J0 = 4; A => B; k * A;");
    assert_eq!(
        reserved_before
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>(),
        [("_J0", Some(4.0)), ("k", Some(0.1))]
    );
    assert_eq!(reserved_before.reactions()[0].id(), "_J1");

    let (reassigned_after, _) = check_roundtrip("A = 2; B = 0; k = 0.1; A => B; k * A; _J0 = 4;");
    assert_eq!(
        reassigned_after
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>(),
        [("k", Some(0.1))]
    );
    assert_eq!(reassigned_after.reactions()[0].id(), "_J0");
    assert!(matches!(
        reassigned_after
            .expression(reassigned_after.reactions()[0].rate())
            .unwrap()
            .kind(),
        ExpressionKind::Number { value, .. } if *value == 4.0
    ));
}

#[test]
fn compatible_species_and_formula_redeclarations_merge_cleanly() {
    let source =
        "species S, P; species S; formula k; formula k = 0.15; S = 3.5; P = 0.5; J: S => P; k * S;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .species()
            .iter()
            .map(|s| (s.id(), s.initial_concentration()))
            .collect::<Vec<_>>(),
        [("P", Some(0.5)), ("S", Some(3.5))]
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>(),
        [("k", Some(0.15))]
    );
}

#[test]
fn species_redeclaration_binds_compartment_and_last_explicit_compartment_wins() {
    let source = "compartment cell = 1.5; species S; species S in cell; species P in cell; S = 2; P = 0; S => P; 0.1 * S;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(model.compartments().len(), 1);
    assert_eq!(model.compartments()[0].id(), "cell");
    assert_eq!(model.compartments()[0].size(), Some(1.5));
    assert!(!model.compartments()[0].is_implicit());
    assert!(model.species().iter().all(|s| s.compartment() == "cell"));

    let (rebound, _) = check_roundtrip("compartment c = 2, d = 3; species A in c; species A in d;");
    assert_eq!(
        rebound
            .compartments()
            .iter()
            .map(|c| (c.id(), c.size(), c.is_implicit()))
            .collect::<Vec<_>>(),
        [("c", Some(2.0), false), ("d", Some(3.0), false)]
    );
    assert_eq!(rebound.species().len(), 1);
    assert_eq!(rebound.species()[0].id(), "A");
    assert_eq!(rebound.species()[0].compartment(), "d");
    assert_eq!(rebound.species()[0].initial_concentration(), None);
}

#[test]
fn formula_promotes_to_species_when_used_in_reaction_or_species_declaration() {
    let source = "formula S = 2, P = 0, k = 0.1; J: S => P; k * S;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .species()
            .iter()
            .map(|s| (s.id(), s.initial_concentration(), s.boundary_condition()))
            .collect::<Vec<_>>(),
        [("P", Some(0.0), false), ("S", Some(2.0), false)]
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value()))
            .collect::<Vec<_>>(),
        [("k", Some(0.1))]
    );

    let reverse = "J: S => P; k * S; formula S = 5; species P = 1; formula k = 0.2;";
    let (rev_model, _) = check_roundtrip(reverse);
    assert_eq!(
        rev_model
            .species()
            .iter()
            .map(|s| (s.id(), s.initial_concentration()))
            .collect::<Vec<_>>(),
        [("P", Some(1.0)), ("S", Some(5.0))]
    );
    assert_eq!(rev_model.parameters().len(), 1);
    assert_eq!(rev_model.parameters()[0].id(), "k");
}

#[test]
fn const_and_var_parameter_qualifiers_and_dollar_formula_syntax() {
    let source = "const formula k_cf = 2; var formula k_vf = 3; const k_c = 4; var k_v = 5; formula $k_df = 6;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value(), p.constant()))
            .collect::<Vec<_>>(),
        [
            ("k_c", Some(4.0), true),
            ("k_cf", Some(2.0), true),
            ("k_df", Some(6.0), true),
            ("k_v", Some(5.0), false),
            ("k_vf", Some(3.0), false),
        ]
    );
}

#[test]
fn species_qualifier_ordering_respects_latest_const_or_var_declaration() {
    let source = "const species S_to_var = 2; var species S_to_var; var species S_to_const = 3; const species S_to_const; var species S_dollar = 4; $S_dollar => S_to_var; 0.1 * S_dollar;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .species()
            .iter()
            .map(|s| (
                s.id(),
                s.initial_concentration(),
                s.boundary_condition(),
                s.constant()
            ))
            .collect::<Vec<_>>(),
        [
            ("S_dollar", Some(4.0), true, false),
            ("S_to_const", Some(3.0), true, false),
            ("S_to_var", Some(2.0), false, false),
        ]
    );
}

#[test]
fn parameter_explicit_const_or_var_overrides_species_dependency_inference() {
    let source = "species S = 2; const species B = 3; const formula a_const = S; var formula a_var = S; var formula b_var = B; formula inferred_var = S + 1; formula inferred_const = B + 1;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| (
                p.id(),
                p.value(),
                p.constant(),
                p.initial_expression().is_some()
            ))
            .collect::<Vec<_>>(),
        [
            ("a_const", Some(2.0), true, true),
            ("a_var", Some(2.0), false, true),
            ("b_var", Some(3.0), false, true),
            ("inferred_const", Some(4.0), true, true),
            ("inferred_var", Some(3.0), false, true),
        ]
    );
}

#[test]
fn unfinished_model_preserves_none_values_then_populates_some_when_initialized() {
    let skeleton =
        "compartment cell; species S in cell, P in cell; formula k_cat; S => P; k_cat * S;";
    let (unfinished, _) = check_roundtrip(skeleton);

    let cell = &unfinished.compartments()[0];
    assert_eq!(cell.id(), "cell");
    assert_eq!(cell.size(), None);
    assert!(!cell.is_implicit());

    for s in unfinished.species() {
        assert_eq!(s.initial_concentration(), None);
        assert_eq!(s.initial_expression(), None);
    }
    let k_cat = &unfinished.parameters()[0];
    assert_eq!(k_cat.id(), "k_cat");
    assert_eq!(k_cat.value(), None);
    assert_eq!(k_cat.initial_expression(), None);
    assert!(k_cat.constant());

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&unfinished).unwrap().xml().to_owned();
        assert!(!xml.contains("size="));
        assert!(!xml.contains("initialConcentration="));
        assert!(!xml.contains("value="));
    }

    let completed_source = format!("{skeleton} cell = 2.5; S = 8; P = 0; k_cat = 0.4;");
    let (completed, _) = check_roundtrip(&completed_source);
    assert_eq!(completed.compartments()[0].size(), Some(2.5));
    assert_eq!(
        completed
            .species()
            .iter()
            .map(|s| (s.id(), s.initial_concentration()))
            .collect::<Vec<_>>(),
        [("P", Some(0.0)), ("S", Some(8.0))]
    );
    assert_eq!(completed.parameters()[0].value(), Some(0.4));
}

#[test]
fn implicit_default_compartment_has_size_one_unlike_explicit_unset_compartment() {
    let (implicit_model, _) = check_roundtrip("species S;");
    assert_eq!(implicit_model.compartments().len(), 1);
    let default_comp = &implicit_model.compartments()[0];
    assert_eq!(default_comp.id(), "default_compartment");
    assert!(default_comp.is_implicit());
    assert_eq!(default_comp.size(), Some(1.0));
    assert_eq!(implicit_model.species()[0].initial_concentration(), None);

    let (explicit_model, _) = check_roundtrip("compartment c; species S in c;");
    assert_eq!(explicit_model.compartments().len(), 1);
    let explicit_comp = &explicit_model.compartments()[0];
    assert_eq!(explicit_comp.id(), "c");
    assert!(!explicit_comp.is_implicit());
    assert_eq!(explicit_comp.size(), None);
}

#[test]
fn symbolic_initializer_with_missing_dependency_preserves_expression_and_none_value() {
    let source = "a = b + 1; species S = S_init * 2; formula S_init; c = d + 3; d = 4;";
    let (model, _) = check_roundtrip(source);

    let s = model.species().iter().find(|s| s.id() == "S").unwrap();
    assert_eq!(s.initial_concentration(), None);
    let s_expr = s
        .initial_expression()
        .expect("S must retain its symbolic initial expression");
    assert!(matches!(
        model.expression(s_expr).unwrap().kind(),
        ExpressionKind::Binary { .. }
    ));

    let a = model.parameters().iter().find(|p| p.id() == "a").unwrap();
    assert_eq!(a.value(), None);
    assert!(
        a.initial_expression().is_some(),
        "a must retain its symbolic initial expression"
    );

    let b = model.parameters().iter().find(|p| p.id() == "b").unwrap();
    assert_eq!(b.value(), None);
    assert_eq!(
        b.initial_expression(),
        None,
        "unspecified b must have no initial_expression"
    );

    let s_init = model
        .parameters()
        .iter()
        .find(|p| p.id() == "S_init")
        .unwrap();
    assert_eq!(s_init.value(), None);
    assert_eq!(s_init.initial_expression(), None);

    let c = model.parameters().iter().find(|p| p.id() == "c").unwrap();
    assert_eq!(c.value(), Some(7.0));
    assert!(c.initial_expression().is_some());

    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|ia| ia.symbol())
            .collect::<Vec<_>>(),
        ["S", "a", "c"]
    );

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(xml.contains("<initialAssignment symbol=\"S\">"));
        assert!(xml.contains("<initialAssignment symbol=\"a\">"));
        assert!(!xml.contains("initialConcentration="));
    }
}

#[test]
fn compartment_list_declarations_late_scalar_sizes_and_var_compartment() {
    let source = "compartment cyto = 2, nuc, ER = 0.5; var compartment vesicle = 0.25; nuc = 1.25; species A in cyto, B in nuc, C in vesicle; A = 3; B = 1;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(
        model
            .compartments()
            .iter()
            .map(|c| (c.id(), c.size(), c.is_implicit()))
            .collect::<Vec<_>>(),
        [
            ("ER", Some(0.5), false),
            ("cyto", Some(2.0), false),
            ("nuc", Some(1.25), false),
            ("vesicle", Some(0.25), false),
        ]
    );
    assert_eq!(
        model
            .species()
            .iter()
            .map(|s| (s.id(), s.compartment(), s.initial_concentration()))
            .collect::<Vec<_>>(),
        [
            ("A", "cyto", Some(3.0)),
            ("B", "nuc", Some(1.0)),
            ("C", "vesicle", None),
        ]
    );

    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&model).unwrap().xml().to_owned();
        assert!(
            xml.contains("id=\"vesicle\" size=\"0.25\" spatialDimensions=\"3\" constant=\"false\"")
        );
        assert!(xml.contains("id=\"cyto\" size=\"2\" spatialDimensions=\"3\" constant=\"true\""));
    }
}

#[test]
fn repeated_initializations_override_earlier_scalar_or_expression_values() {
    let source = "a = 1; a = 2.5; b = a + 3; b = 10; species S = 4; S = a * 2;";
    let (model, _) = check_roundtrip(source);

    let a = model.parameters().iter().find(|p| p.id() == "a").unwrap();
    assert_eq!(a.value(), Some(2.5));
    assert_eq!(a.initial_expression(), None);

    let b = model.parameters().iter().find(|p| p.id() == "b").unwrap();
    assert_eq!(b.value(), Some(10.0));
    assert_eq!(
        b.initial_expression(),
        None,
        "scalar override clears prior expression initializer"
    );

    let s = &model.species()[0];
    assert_eq!(s.id(), "S");
    assert_eq!(s.initial_concentration(), Some(5.0));
    assert!(s.initial_expression().is_some());
    assert_eq!(
        model
            .initial_assignments()
            .iter()
            .map(|ia| ia.symbol())
            .collect::<Vec<_>>(),
        ["S"]
    );
}

#[test]
fn units_and_annotations_compose_with_uninitialized_symbols_and_promoted_species() {
    let source = "unit mM = 1e-3 mole / litre; compartment cell; cell has litre; formula S; S has mM; k = 0.2; S =>; k * S;";
    let (model, _) = check_roundtrip(source);

    assert_eq!(model.compartments()[0].id(), "cell");
    assert_eq!(model.compartments()[0].size(), None);
    assert_eq!(model.units("cell"), Some("litre"));

    assert_eq!(model.species().len(), 1);
    let s = &model.species()[0];
    assert_eq!(s.id(), "S");
    assert_eq!(s.initial_concentration(), None);
    assert_eq!(model.units("S"), Some("mM"));

    let rate_root = model.reactions()[0].rate();
    assert!(matches!(
        model.expression(rate_root).unwrap().kind(),
        ExpressionKind::Binary { right, .. }
            if matches!(
                model.expression(*right).unwrap().kind(),
                ExpressionKind::Symbol { name, kind: SymbolKind::Species } if name == "S"
            )
    ));
}
