//! Independent species/default unit review: metadata is not numeric conversion.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, LimitKind, Limits, ModelUnit, SourceFile,
    SourceId, ValidatedModel, analyze, parse,
};

fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(
        SourceId::new(92),
        "species-unit-review",
        text,
        &Limits::default(),
    )
    .expect("bounded review source");
    analyze(&parse(&source, limits)?)
}
fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn reject(text: &str) -> Diagnostic {
    let error = compile(text).expect_err(text);
    assert!(!error.labels().is_empty());
    for label in error.labels() {
        assert_eq!(label.span().source_id(), SourceId::new(92));
        assert!(text.get(label.span().range()).is_some());
    }
    error
}

#[test]
fn concentration_annotation_and_emitted_substance_are_separate() {
    let model = compile("compartment c=3;c has litre;species S in c;S=2;S has mole;").unwrap();
    assert_eq!(model.units("S"), Some("mole"));
    assert_eq!(model.species_substance_units("S"), Some("mole_litre"));
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
    assert!(
        matches!(model.symbol_dimensions("S"),Some(DimensionalStatus::Known(d)) if d.substance()==1 && d.length()==0)
    );
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "mole_litre")
        .unwrap();
    assert_eq!(
        generated
            .components()
            .iter()
            .map(|c| c.kind().as_str())
            .collect::<Vec<_>>(),
        ["mole", "litre"]
    );
}

#[test]
fn scaled_concentration_keeps_numbers_and_scaled_substance_metadata() {
    let model = compile(
        "unit conc=0.001 mole/litre;compartment c=3;c has litre;species S in c;S=2;S has conc;",
    )
    .unwrap();
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
    assert_eq!(model.units("S"), Some("conc"));
    assert_eq!(model.species_substance_units("S"), Some("conc_litre"));
    let generated = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "conc_litre")
        .unwrap();
    assert_eq!(generated.components().len(), 1);
    assert_eq!(generated.components()[0].scale(), -3);
    assert_eq!(generated.components()[0].multiplier(), 1.0);
}

#[test]
fn generated_definitions_follow_first_species_occurrence_and_reuse() {
    let model =
        compile("unit zed=second;species B,A;B=2;A=3;B has mole;A has dimensionless;").unwrap();
    assert_eq!(
        model
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>(),
        ["zed", "mole_litre", "dimensionless_litre"]
    );
    let model =
        compile("unit custom=mole*litre;species B,A;B=2;A=3;B has mole;A has mole;").unwrap();
    assert_eq!(model.unit_definitions().len(), 1);
    assert_eq!(model.species_substance_units("B"), Some("custom"));
    assert_eq!(model.species_substance_units("A"), Some("custom"));
}

#[test]
fn forward_concentration_and_compartment_unit_references_resolve() {
    let model=compile("S has conc;S=2;species S in c;compartment c=3;c has vol;unit conc=mole/litre;unit vol=litre;").unwrap();
    assert_eq!(model.species_substance_units("S"), Some("mole"));
    assert_eq!(model.units("c"), Some("vol"));
    assert_eq!(
        model.species()[0]
            .initial_concentration()
            .expect("explicit initialized test model"),
        2.0
    );
}

#[test]
fn all_six_defaults_are_explicit_metadata_not_entity_overrides() {
    let model=compile("unit time_unit=second;unit substance=mole;unit volume=litre;unit area=metre^2;unit length=metre;unit extent=mole;species S;S=2;p=3;").unwrap();
    for (kind, expected) in [
        (ModelUnit::Time, "second"),
        (ModelUnit::Substance, "mole"),
        (ModelUnit::Volume, "litre"),
        (ModelUnit::Area, "area"),
        (ModelUnit::Length, "metre"),
        (ModelUnit::Extent, "mole"),
    ] {
        assert_eq!(model.model_units(kind), Some(expected));
    }
    assert_eq!(model.units("S"), None);
    assert_eq!(model.species_substance_units("S"), None);
    assert_eq!(model.units("p"), None);
    assert_eq!(
        model.symbol_dimensions("p"),
        Some(DimensionalStatus::Unspecified)
    );
    assert!(
        matches!(model.symbol_dimensions("S"),Some(DimensionalStatus::Known(d)) if d.substance()==1 && d.length()==-3)
    );
}

#[test]
fn explicit_compartment_units_override_default_without_rescaling() {
    let model = compile(
        "unit volume=0.001 litre;compartment c=3;c has litre;species S in c;S=2;S has mole;",
    )
    .unwrap();
    assert_eq!(model.model_units(ModelUnit::Volume), Some("volume"));
    assert_eq!(model.units("c"), Some("litre"));
    assert_eq!(model.species_substance_units("S"), Some("mole_litre"));
    assert_eq!(
        model.compartments()[0]
            .size()
            .expect("explicit initialized test model"),
        3.0
    );
    assert_eq!(
        model.species()[0]
            .initial_concentration()
            .expect("explicit initialized test model"),
        2.0
    );
}

#[test]
fn unknown_invalid_and_conflicting_metadata_fail_closed() {
    for text in [
        "compartment c=3;species S in c;S=2;S has mole;",
        "unit volume=0.001 litre;compartment c=3;species S in c;S=2;S has mole;",
        "unit mole_litre=second;species S;S=2;S has mole;",
        "mole_litre=4;species S;S=2;S has mole;",
        "species S;S=2;S has mystery;",
        "species S;S=2;S has mole;S has second;",
        "unit volume=mystery;species S;S=2;",
        "unit volume=litre;unit volume=second;species S;S=2;",
    ] {
        reject(text);
    }
}

#[test]
fn generated_units_share_entity_budget_and_identical_units_are_reused() {
    let text = "species S;S=2;S has mole;";
    assert!(
        compile_with(
            text,
            &Limits::default().with_limit(LimitKind::ModelEntities, 3)
        )
        .is_ok()
    );
    let error = compile_with(
        text,
        &Limits::default().with_limit(LimitKind::ModelEntities, 2),
    )
    .unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    let model = compile("species A,B;A=2;B=3;A has mole;B has mole;").unwrap();
    assert_eq!(model.unit_definitions().len(), 1);
}

#[test]
fn concurrent_defaults_and_generated_units_have_no_global_state() {
    let threads: Vec<_> = (0..4)
        .map(|n| {
            std::thread::spawn(move || {
                for i in 1..=8 {
                    let kind = if n % 2 == 0 { "mole" } else { "second" };
                    let text = format!("species S;S={i};S has {kind};");
                    let model = compile(&text).unwrap();
                    assert_eq!(
                        model.species_substance_units("S"),
                        Some(format!("{kind}_litre").as_str())
                    );
                    assert_eq!(
                        model.species()[0]
                            .initial_concentration()
                            .expect("explicit initialized test model"),
                        f64::from(i)
                    );
                    assert_eq!(model.model_units(ModelUnit::Volume), None);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
#[cfg(feature = "sbml")]
fn species_units_export_is_repeatable_and_does_not_change_initialization_mode() {
    let model = compile("unit conc=mole/litre;species S;S=k+1;k=2;S has conc;").unwrap();
    let xml = antimony::write_sbml(&model).unwrap();
    assert!(xml.xml().contains("substanceUnits=\"mole\""));
    assert!(xml.xml().contains("symbol=\"S\""));
    assert!(!xml.xml().contains("initialConcentration="));
    assert_eq!(xml.xml(), antimony::write_sbml(&model).unwrap().xml());
}

/// Original review probe inputs; exact projection evidence is in the A review.
#[test]
fn frozen_species_default_probe_acceptance() {
    for (name, text, accepted) in [
        ("duplicate_identical", "species S;S=2;S has litre;", true),
        (
            "duplicate_dimless",
            "compartment c=3;c has dimensionless;species S in c;S=2;S has dimensionless;",
            true,
        ),
        (
            "alias_raw_inverse",
            "unit base=mole/litre;unit conc=base;species S;S=2;S has conc;",
            true,
        ),
        (
            "reversed_reuse",
            "unit custom=litre*mole;species S;S=2;S has mole;",
            true,
        ),
        (
            "multiple_reuse",
            "unit z=mole*litre;unit a=mole*litre;species S;S=2;S has mole;",
            true,
        ),
        (
            "future_reuse",
            "species S;S=2;S has mole;unit custom=mole*litre;",
            true,
        ),
        (
            "generated_order_reference",
            "x=A*B;species B,A;B=2;A=3;B has mole;A has dimensionless;",
            true,
        ),
        (
            "generated_species_collision",
            "species mole_litre,S;mole_litre=4;S=2;S has mole;",
            false,
        ),
        (
            "generated_reaction_collision",
            "S=2;P=0;S has mole;mole_litre:S=>P;S;",
            false,
        ),
        (
            "default_forward",
            "unit volume=vol;unit vol=0.001 litre;species S;S=2;",
            true,
        ),
        (
            "default_alias_dimless",
            "unit vol=dimensionless;unit volume=vol;species S;S=2;",
            true,
        ),
        (
            "default_explicit_ref",
            "unit volume=0.001 litre;compartment c=3;c has volume;species S in c;S=2;S has mole;",
            true,
        ),
        (
            "default_collapse_param",
            "unit substance=mole;p=3;p has substance;",
            true,
        ),
        (
            "default_collapse_species",
            "unit substance=mole;species S;S=2;S has substance;",
            true,
        ),
        (
            "default_cross_alias",
            "unit substance=mole;unit extent=substance;species S;S=2;",
            true,
        ),
        (
            "default_dimensionless_mapping",
            "unit volume=dimensionless;species S;S=2;S has mole;",
            false,
        ),
        (
            "inherit_substance_missing_volume",
            "unit substance=mole;species S;S=2;",
            true,
        ),
        (
            "annotation_dimensionless_explicit",
            "compartment c=3;c has dimensionless;species S in c;S=2;S has mole;",
            true,
        ),
        (
            "default_badvolume",
            "unit volume=second;species S;S=2;",
            false,
        ),
        (
            "known_species_conflict",
            "unit conc=mole/litre;species S;S=2;S has conc;p=3;p has second;S2=S+p;species S2;",
            false,
        ),
    ] {
        let result = compile(text);
        assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
        if !accepted {
            reject(text);
        }
    }
}
