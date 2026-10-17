//! Bounded units preserve metadata and expose uncertainty, never rescale values.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionKind, LimitKind, Limits, SourceFile,
    SourceId, UnitKind, UnitLimits, ValidatedModel, analyze, parse,
};
fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(SourceId::new(8), "units", text, &Limits::default()).unwrap();
    analyze(&parse(&source, limits)?)
}

#[test]
fn builtin_annotations_are_explicit_canonical_metadata() {
    for (name, canonical) in [
        ("mole", "mole"),
        ("moles", "mole"),
        ("second", "second"),
        ("seconds", "second"),
        ("litre", "litre"),
        ("liter", "litre"),
        ("liters", "litre"),
        ("litres", "litre"),
        ("metre", "metre"),
        ("metres", "metre"),
        ("meters", "metre"),
        ("dimensionless", "dimensionless"),
    ] {
        let model = compile(&format!("p=2;p has {name};")).unwrap();
        assert_eq!(model.units("p"), Some(canonical));
        assert!(model.unit_definitions().is_empty());
        assert_eq!(
            model.parameters()[0]
                .value()
                .expect("explicit initialized test model"),
            2.0
        );
    }
}
#[test]
fn scalar_unit_normalization_preserves_numbers() {
    for (coefficient, multiplier, scale) in [
        ("0.002", 2.0, -3),
        ("3600", 3.6, 3),
        ("123", 1.23, 2),
        ("0.12", 1.2, -1),
        ("1.2345", 1.2345, 0),
        ("1e-12", 1.0, -12),
        ("1e12", 1.0, 12),
    ] {
        let model = compile(&format!("unit uu={coefficient} second;p=2;p has uu;")).unwrap();
        let component = &model.unit_definitions()[0].components()[0];
        assert_eq!(
            (
                component.kind(),
                component.exponent(),
                component.multiplier(),
                component.scale()
            ),
            (UnitKind::Second, 1, multiplier, scale)
        );
        assert_eq!(
            model.parameters()[0]
                .value()
                .expect("explicit initialized test model"),
            2.0
        );
    }
}
#[test]
fn products_quotients_and_integer_powers_preserve_component_order() {
    let model = compile("unit uu=second^-2*mole/metre^3;p=1;p has uu;").unwrap();
    let components = model.unit_definitions()[0].components();
    assert_eq!(
        components
            .iter()
            .map(|c| (c.kind(), c.exponent()))
            .collect::<Vec<_>>(),
        [
            (UnitKind::Second, -2),
            (UnitKind::Mole, 1),
            (UnitKind::Metre, -3)
        ]
    );
    let DimensionalStatus::Known(d) = model.symbol_dimensions("p").unwrap() else {
        panic!()
    };
    assert_eq!((d.substance(), d.time(), d.length()), (1, -2, -3));
}
#[test]
fn scaled_concentration_definition_is_not_a_species_annotation() {
    let model = compile("unit conc=0.001*mole/litre;p=5;p has conc;").unwrap();
    assert_eq!(
        model.parameters()[0]
            .value()
            .expect("explicit initialized test model"),
        5.0
    );
    assert_eq!(model.unit_definitions()[0].components()[0].scale(), -3);
    let model = compile("unit conc=mole/litre;species S;S=2;S has conc;").unwrap();
    assert_eq!(model.units("S"), Some("conc"));
    assert_eq!(model.species_substance_units("S"), Some("mole"));
}
#[test]
fn aliases_expand_iteratively_and_keep_definition_order() {
    let model =
        compile("p=2;p has last;unit last=middle;unit middle=first;unit first=mole/second;")
            .unwrap();
    assert_eq!(
        model
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>(),
        ["last", "middle", "first"]
    );
    assert!(
        model
            .unit_definitions()
            .iter()
            .all(|u| u.components().len() == 2)
    );
}
#[test]
fn alias_powers_preserve_component_scale_without_value_conversion() {
    let model = compile("unit small=0.001 metre;unit square=small^2;p=3;p has square;").unwrap();
    let u = model
        .unit_definitions()
        .iter()
        .find(|u| u.id() == "square")
        .unwrap();
    assert_eq!(
        (u.components()[0].exponent(), u.components()[0].scale()),
        (2, -3)
    );
    assert_eq!(
        model.parameters()[0]
            .value()
            .expect("explicit initialized test model"),
        3.0
    );
}
#[test]
fn source_and_expanded_exponent_bounds_are_enforced() {
    assert_eq!(UnitLimits::SOURCE_EXPONENT, 16);
    assert_eq!(UnitLimits::RESOLVED_EXPONENT, 64);
    assert!(compile("unit first=metre^16;unit last=first^4;").is_ok());
    assert_eq!(
        compile("unit first=metre^16;unit last=first^5;")
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
    for text in [
        "unit uu=metre^17;",
        "unit uu=metre^0;",
        "unit uu=metre^0.5;",
        "unit uu=metre^999999999999999999999;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature
        );
    }
}
#[test]
fn coefficient_limits_and_unverified_forms_fail_closed() {
    for text in [
        "unit uu=0 mole;",
        "unit uu=-1 mole;",
        "unit uu=1e13 mole;",
        "unit uu=1e-13 mole;",
        "unit uu=1e999 mole;",
        "unit uu=1.234567890123456 mole;",
        "unit uu=2 mole^2;",
        "unit uu=2*mole^2;",
        "unit uu=(2*mole)^2;",
        "unit uu=mole/2;",
        "unit uu=mole/5 litre;",
        "unit base=second;unit uu=2*base;",
        "unit uu=mole*mole;",
        "unit uu=mole/mole;",
        "unit uu=2;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{text}"
        );
    }
}
#[test]
fn unknown_units_cycles_and_namespace_conflicts_are_rejected() {
    for text in ["unit uu=missing;", "p=2;p has missing;"] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature
        );
    }
    for text in [
        "unit aa=bb;unit bb=aa;",
        "unit aa=aa;",
        "unit aa=mole;unit aa=second;",
        "unit aa=mole;aa=1;",
        "p=2;mole=1;p has mole;",
        "x=2;unit aa=x;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::DeclarationConflict,
            "{text}"
        );
    }
}
#[test]
fn exotic_builtin_names_and_extended_unit_syntax_stay_outside_slice() {
    for text in [
        "unit hrs=second;",
        "p=2;p has meter;",
        "p=2;p has kelvin;",
        "p=2 mole;",
        "p=2;p has mole/second;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnsupportedFeature,
            "{text}"
        );
    }
}
#[test]
fn annotated_compartments_accept_volume_or_dimensionless_not_time() {
    for (text, unit) in [
        ("compartment c=2;c has litre;", "litre"),
        ("unit cube=metre^3;compartment c=2;c has cube;", "cube"),
        ("compartment c=2;c has dimensionless;", "dimensionless"),
    ] {
        let model = compile(text).unwrap();
        assert_eq!(
            model.compartments()[0]
                .size()
                .expect("explicit initialized test model"),
            2.0
        );
        assert_eq!(model.units("c"), Some(unit));
    }
    assert_eq!(
        compile("compartment c=2;c has second;").unwrap_err().code(),
        DiagnosticCode::UnitsInconsistency
    );
}
#[test]
fn absence_and_dimensionless_are_distinct_public_states() {
    let model = compile("a=1;b=2;b has dimensionless;c=a+b;").unwrap();
    assert_eq!(
        model.symbol_dimensions("a"),
        Some(DimensionalStatus::Unspecified)
    );
    let Some(DimensionalStatus::Known(d)) = model.symbol_dimensions("b") else {
        panic!()
    };
    assert_eq!((d.substance(), d.time(), d.length()), (0, 0, 0));
    assert_eq!(model.symbol_dimensions("unknown"), None);
    assert_eq!(
        model.expression_dimensions(model.initial_assignments()[0].expression()),
        Some(DimensionalStatus::Indeterminate)
    );
}
#[test]
fn known_unit_conflicts_reject_even_inside_unknown_expression() {
    for text in [
        "a=1;a has mole;b=2;b has second;c=a+b;",
        "a=1;a has mole;b=2;b has second;c=(a-b)*2;",
        "a=1;a has mole;b=2;b has second;c=a;c has second;",
        "a=2;a has mole;b=3;b has second;c=a^b;",
    ] {
        assert_eq!(
            compile(text).unwrap_err().code(),
            DiagnosticCode::UnitsInconsistency,
            "{text}"
        );
    }
}
#[test]
fn initialized_target_check_does_not_infer_or_rescale_unspecified_units() {
    let model = compile("a=2;a has mole;b=a*2;b has second;c=a;").unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "b")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        4.0
    );
    assert_eq!(
        model.symbol_dimensions("c"),
        Some(DimensionalStatus::Unspecified)
    );
    assert!(
        model
            .warnings()
            .iter()
            .any(|d| d.code() == DiagnosticCode::UnspecifiedUnits)
    );
    assert_eq!(
        compile("a=2;a has mole;b=a;b has second;")
            .unwrap_err()
            .code(),
        DiagnosticCode::UnitsInconsistency
    );
}
#[test]
fn numeric_arithmetic_and_power_operands_remain_unspecified() {
    let model = compile("a=2;a has metre;b=a^2;b has mole;").unwrap();
    let root = model.initial_assignments()[0].expression();
    assert_eq!(
        model.expression_dimensions(root),
        Some(DimensionalStatus::Indeterminate)
    );
    let ExpressionKind::Binary { right, .. } = model.expression(root).unwrap().kind() else {
        panic!()
    };
    assert_eq!(
        model.expression_dimensions(*right),
        Some(DimensionalStatus::Unspecified)
    );
}
#[test]
fn known_products_and_quotients_use_dimensions_not_scale() {
    let model=compile("unit small=0.001 litre;a=2;a has small;b=3;b has metre;c=a/b;unit area_u=metre^2;c has area_u;").unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "c")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        2.0 / 3.0
    );
    let Some(DimensionalStatus::Known(d)) =
        model.expression_dimensions(model.initial_assignments()[0].expression())
    else {
        panic!()
    };
    assert_eq!(d.length(), 2);
}
#[test]
fn unit_entities_share_model_entity_budget() {
    assert_eq!(
        compile_with(
            "unit aa=second;p=2;p has aa;",
            &Limits::default().with_limit(LimitKind::ModelEntities, 1)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
    assert_eq!(
        compile_with(
            "unit aa=second;",
            &Limits::default().with_limit(LimitKind::Diagnostics, 0)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
}
#[test]
fn ten_thousand_aliases_and_cycle_are_stack_safe() {
    let mut source = String::new();
    for i in 0..10000 {
        source.push_str(&format!("unit u{i}=u{};", i + 1));
    }
    source.push_str("unit u10000=second;");
    assert_eq!(compile(&source).unwrap().unit_definitions().len(), 10001);
    assert_eq!(
        compile(&source.replace("unit u10000=second;", "unit u10000=u0;"))
            .unwrap_err()
            .code(),
        DiagnosticCode::DeclarationConflict
    );
}
#[cfg(feature = "sbml")]
#[test]
fn export_preserves_unit_metadata_and_leaves_values_unchanged() {
    let model =
        compile("unit minute=60 second;p=2;p has minute;compartment c=3;c has litre;").unwrap();
    let out = antimony::write_sbml(&model).unwrap();
    assert!(
        out.xml()
            .contains("<unit kind=\"second\" exponent=\"1\" scale=\"1\" multiplier=\"6\"/>")
    );
    assert!(out.xml().contains("units=\"minute\" value=\"2\""));
    assert!(out.xml().contains("size=\"3\""));
    assert!(!out.xml().contains("substanceUnits="));
    assert!(!out.xml().contains("timeUnits="));
}
