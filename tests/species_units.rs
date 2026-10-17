//! Reference-backed concentration metadata, model defaults, and bounded generation.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, LimitKind, Limits, ModelUnit, SourceFile,
    SourceId, ValidatedModel, analyze, parse,
};
fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source =
        SourceFile::new(SourceId::new(42), "species-units", text, &Limits::default()).unwrap();
    analyze(&parse(&source, limits)?)
}
fn dimensions(model: &ValidatedModel, name: &str) -> (i32, i32, i32) {
    match model.symbol_dimensions(name).unwrap() {
        DimensionalStatus::Known(d) => (d.substance(), d.time(), d.length()),
        other => panic!("expected known dimensions for {name}: {other:?}"),
    }
}
#[test]
fn absent_model_defaults_remain_absent() {
    let model = compile("species S=2;p=3;").unwrap();
    for default in [
        ModelUnit::Substance,
        ModelUnit::Volume,
        ModelUnit::Area,
        ModelUnit::Length,
        ModelUnit::Extent,
        ModelUnit::Time,
    ] {
        assert_eq!(model.model_units(default), None);
    }
    assert_eq!(model.units("S"), None);
    assert_eq!(model.species_substance_units("S"), None);
    assert_eq!(
        model.symbol_dimensions("S"),
        Some(DimensionalStatus::Unspecified)
    );
    assert_eq!(
        model.species()[0]
            .initial_concentration()
            .expect("explicit initialized test model"),
        2.0
    );
}
#[test]
fn explicit_concentration_dimensions_check_initialization_not_substance() {
    let good = "unit conc=mole/litre;species S=2;S has conc;p=3;p has conc;species T=p;T has conc;";
    let model = compile(good).unwrap();
    assert_eq!(model.units("S"), Some("conc"));
    assert_eq!(dimensions(&model, "S"), (1, 0, -3));
    assert_eq!(
        model.species()[1]
            .initial_concentration()
            .expect("explicit initialized test model"),
        3.0
    );
    assert_eq!(
        compile(&good.replace("p has conc", "p has second"))
            .unwrap_err()
            .code(),
        DiagnosticCode::UnitsInconsistency
    );
}
#[test]
fn parameter_units_are_not_inherited_from_model_defaults() {
    let model =
        compile("unit substance=mole;unit volume=litre;unit time_unit=second;p=3;").unwrap();
    assert_eq!(model.units("p"), None);
    assert_eq!(
        model.symbol_dimensions("p"),
        Some(DimensionalStatus::Unspecified)
    );
}
#[test]
fn source_unit_definitions_share_entity_budget_with_numeric_symbols() {
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 1);
    assert_eq!(
        compile_with("unit substance=mole;p=1;", &limits)
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn scaled_volume_components_are_preserved_not_numerically_cancelled() {
    let model=compile("unit conc=mole/litre;unit small=0.001 litre;compartment c=3;c has small;species S in c=2;S has conc;").unwrap();
    assert_eq!(model.species_substance_units("S"), Some("conc_small"));
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "conc_small")
        .unwrap();
    assert_eq!(
        generated
            .components()
            .iter()
            .map(|c| (c.kind().as_str(), c.exponent(), c.scale()))
            .collect::<Vec<_>>(),
        [("mole", 1, 0), ("litre", -1, 0), ("litre", 1, -3)]
    );
    assert_eq!(
        model.species()[0]
            .initial_concentration()
            .expect("explicit initialized test model"),
        2.0
    );
    assert_eq!(
        model.compartments()[0]
            .size()
            .expect("explicit initialized test model"),
        3.0
    );
}
#[test]
fn aliases_reduce_before_expansion_not_after() {
    let model = compile("unit base=mole/litre;unit conc=base;species S=2;S has conc;").unwrap();
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "conc_litre")
        .unwrap();
    assert_eq!(
        generated
            .components()
            .iter()
            .map(|c| (c.kind().as_str(), c.exponent()))
            .collect::<Vec<_>>(),
        [("mole", 1), ("litre", -1), ("litre", 1)]
    );
}
#[test]
fn two_component_inverse_pair_is_preserved_upstream() {
    let model = compile("unit inv=1/litre;species S=2;S has inv;").unwrap();
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "inv_litre")
        .unwrap();
    assert_eq!(generated.components().len(), 2);
    assert_eq!(generated.components()[0].exponent(), -1);
    assert_eq!(generated.components()[1].exponent(), 1);
}
#[test]
fn special_defaults_redirect_to_first_canonical_match_even_forward() {
    let model=compile("p=2;p has substance;unit substance=0.001 mole;unit first=0.001 mole;unit later=0.001 mole;").unwrap();
    assert_eq!(model.model_units(ModelUnit::Substance), Some("first"));
    assert_eq!(model.units("p"), Some("first"));
    assert_eq!(
        model
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>(),
        ["first", "later"]
    );
}
#[test]
fn ordinary_alias_is_not_collapsed_like_a_special_default() {
    let model = compile("unit amount=mole;p=2;p has amount;unit substance=amount;").unwrap();
    assert_eq!(model.units("p"), Some("amount"));
    assert_eq!(model.model_units(ModelUnit::Substance), Some("mole"));
    assert_eq!(model.unit_definitions()[0].id(), "amount");
}
#[test]
fn source_species_default_annotation_is_not_replaced_by_export_redirect() {
    let model = compile("unit substance=mole;species S=2;S has substance;").unwrap();
    assert_eq!(model.units("S"), Some("substance"));
    assert_eq!(model.species_substance_units("S"), Some("substance_litre"));
    assert_eq!(dimensions(&model, "S"), (1, 0, 0));
}
#[test]
fn generated_reuse_compares_components_but_preserves_existing_order() {
    let model = compile("unit custom=litre*mole;species S=2;S has mole;").unwrap();
    assert_eq!(model.species_substance_units("S"), Some("custom"));
    assert_eq!(model.unit_definitions().len(), 1);
    assert_eq!(
        model.unit_definitions()[0].components()[0].kind().as_str(),
        "litre"
    );
}
#[test]
fn same_name_matching_precedes_first_equivalent_definition() {
    let model = compile("unit other=litre*mole;unit mole_litre=mole*litre;species S=2;S has mole;")
        .unwrap();
    assert_eq!(model.species_substance_units("S"), Some("mole_litre"));
}
#[test]
fn generated_name_collision_rejects_all_numeric_symbol_classes() {
    for text in [
        "mole_litre=2;species S=1;S has mole;",
        "species mole_litre=2;species S=1;S has mole;",
        "compartment mole_litre=2;species S=1;S has mole;",
        "species S=1;S has mole;mole_litre:S=>;S;",
        "unit mole_litre=second;species S=1;S has mole;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict,
            "{text}"
        );
    }
}
#[test]
fn missing_export_time_definition_is_not_invented() {
    for text in [
        "compartment c=2;species S in c=1;S has mole;",
        "unit volume=0.001 litre;unit conc=mole/litre;compartment c=2;species S in c=1;S has conc;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{text}"
        );
    }
    // No generated definition is necessary when an existing builtin is found.
    assert_eq!(
        compile("unit conc=mole/litre;compartment c=2;species S in c=1;S has conc;")
            .unwrap()
            .species_substance_units("S"),
        Some("mole")
    );
}
#[test]
fn dimensionless_default_does_not_silently_change_annotated_concentration_dimensions() {
    assert_eq!(
        compile("unit volume=dimensionless;species S=1;S has mole;")
            .unwrap_err()
            .code(),
        DiagnosticCode::UnitsInconsistency
    );
    let model = compile("unit volume=dimensionless;unit substance=mole;species S=1;").unwrap();
    assert_eq!(dimensions(&model, "S"), (1, 0, 0));
    assert_eq!(model.species_substance_units("S"), None);
}
#[test]
fn incomplete_defaults_do_not_invent_dimensional_knowledge() {
    for text in [
        "unit substance=mole;species S=1;",
        "unit volume=litre;species S=1;",
    ] {
        assert_eq!(
            compile(text).unwrap().symbol_dimensions("S"),
            Some(DimensionalStatus::Unspecified)
        );
    }
    let model =
        compile("unit substance=mole;compartment c=2;c has litre;species S in c=1;").unwrap();
    assert_eq!(dimensions(&model, "S"), (1, 0, -3));
}
#[test]
fn all_six_default_roles_reject_provably_wrong_dimensions() {
    for name in ["substance", "volume", "area", "length", "extent"] {
        assert_eq!(
            compile(&format!("unit {name}=second;")).unwrap_err().code(),
            DiagnosticCode::UnitsInconsistency
        );
    }
    assert_eq!(
        compile("unit time_unit=mole;").unwrap_err().code(),
        DiagnosticCode::UnitsInconsistency
    );
}
#[test]
fn special_unit_namespace_conflicts_and_cycles_fail_closed() {
    for text in [
        "unit substance=extent;unit extent=substance;",
        "unit substance=mole;unit substance=mole;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict,
            "{text}"
        );
    }
    for text in [
        "unit volume=missing;",
        "volume has litre;",
        "unit volume=litre;volume has litre;",
        "unit substance=mole;species S=1;S has missing;",
    ] {
        assert!(compile(text).is_err(), "{text}");
    }
}
#[test]
fn collapsed_defaults_still_consume_source_entity_budget() {
    let source = "unit substance=mole;unit volume=litre;species S=1;";
    assert!(
        compile_with(
            source,
            &Limits::default().with_limit(LimitKind::ModelEntities, 4)
        )
        .is_ok()
    );
    assert_eq!(
        compile_with(
            source,
            &Limits::default().with_limit(LimitKind::ModelEntities, 3)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
}
#[test]
fn many_species_reuse_one_generated_definition_at_exact_entity_budget() {
    let mut source = "unit conc=0.001 mole/litre;".to_owned();
    for i in 0..512 {
        source.push_str(&format!("species s{i}=1;s{i} has conc;"));
    }
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 515);
    let model = compile_with(&source, &limits).unwrap();
    assert_eq!(model.unit_definitions().len(), 2);
    assert!(
        model
            .species()
            .iter()
            .all(|s| model.species_substance_units(s.id()) == Some("conc_litre"))
    );
    assert_eq!(
        compile_with(&source, &limits.with_limit(LimitKind::ModelEntities, 514))
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
}
#[test]
fn generated_expansion_preserves_repeated_kinds_and_resolved_bounds() {
    let source = "unit conc=mole^16/second^16/metre^16/litre^16/dimensionless^16;unit high=conc^4;unit cube=metre^3;compartment c=2;c has cube;species S in c=1;S has high;";
    let model = compile(source).unwrap();
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "high_cube")
        .unwrap();
    assert_eq!(generated.components().len(), 6);
    assert_eq!(generated.components()[0].exponent(), 64);
    assert_eq!(generated.components()[2].exponent(), -64);
    assert_eq!(generated.components()[5].exponent(), 3);
    assert_eq!(antimony::UnitLimits::GENERATED_COMPONENTS, 10);
}

#[test]
fn model_default_names_are_never_silently_treated_as_numeric_symbols() {
    for name in [
        "substance",
        "volume",
        "area",
        "length",
        "extent",
        "time_unit",
    ] {
        for text in [
            format!("{name}=2;"),
            format!("formula {name}=2;"),
            format!("species {name}=2;"),
            format!("compartment {name}=2;"),
            format!("p={name};"),
        ] {
            assert_eq!(
                compile(&text).unwrap_err().code(),
                DiagnosticCode::UnsupportedFeature,
                "{text}"
            );
        }
    }
}
