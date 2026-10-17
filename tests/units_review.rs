//! Independent bounded unit review. These are not SI-conversion/certification tests.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, LimitKind, Limits, SourceFile, SourceId,
    ValidatedModel, analyze, parse,
};

fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(SourceId::new(91), "units-review", text, &Limits::default())
        .expect("bounded review source");
    analyze(&parse(&source, limits)?)
}
fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with(text, &Limits::default())
}
fn rejected(text: &str, expected: Option<DiagnosticCode>) {
    let error = compile(text).expect_err(text);
    if let Some(code) = expected {
        assert_eq!(error.code(), code, "{text}: {error:?}");
    }
    assert!(!error.labels().is_empty(), "{text}");
    for label in error.labels() {
        assert_eq!(label.span().source_id(), SourceId::new(91));
        assert!(text.get(label.span().range()).is_some(), "{text}");
    }
}

#[test]
fn explicit_dimensionless_and_unspecified_are_distinct() {
    let model = compile("a=2;a has dimensionless;b=3;c=b+1;").unwrap();
    assert!(
        matches!(model.symbol_dimensions("a"), Some(DimensionalStatus::Known(d)) if d.substance()==0 && d.time()==0 && d.length()==0)
    );
    assert_eq!(
        model.symbol_dimensions("b"),
        Some(DimensionalStatus::Unspecified)
    );
    assert_eq!(model.symbol_dimensions("missing"), None);
    assert_eq!(model.units("b"), None);
    let root = model.initial_assignments()[0].expression();
    assert!(!matches!(
        model.expression_dimensions(root),
        Some(DimensionalStatus::Known(_))
    ));
}

#[test]
fn scalar_unit_metadata_never_rescales_numeric_initial_values() {
    let model = compile("unit small=0.001 second;x=7;x has small;y=x+1;y has second;").unwrap();
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "x")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        7.0
    );
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "y")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        8.0
    );
    let component = &model.unit_definitions()[0].components()[0];
    assert_eq!(component.scale(), -3);
    assert_eq!(component.multiplier(), 1.0);
    assert_eq!(component.exponent(), 1);
    assert_eq!(model.units("x"), Some("small"));
}

#[test]
fn known_conflicts_are_detected_even_below_unknown_expressions() {
    for text in [
        "a=2;a has mole;b=3;b has second;c=a+b;",
        "a=2;a has mole;b=3;b has second;c=(a-b)*2;",
        "a=2;a has metre;b=a;b has second;",
        "a=2;a has mole;b=3;b has second;c=a*b;c has mole;",
        "a=2;a has mole;b=3;b has second;c=a^b;",
    ] {
        rejected(text, Some(DiagnosticCode::UnitsInconsistency));
    }
}

#[test]
fn contextual_literals_do_not_create_false_dimension_conflicts() {
    for text in [
        "a=2;a has mole;b=a+1;b has mole;",
        "a=2;a has mole;b=a*2;b has second;",
        "a=2;a has mole;b=3;c=a+b;c has second;",
        "a=2;a has mole;b=a+a;b has mole;",
        "a=2;a has mole;b=3;c=a^b;c has second;",
    ] {
        compile(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    }
}

#[test]
fn volume_dimensions_and_definition_order_are_not_numeric_conversions() {
    let model = compile("unit cubic=metre^3;compartment c=2;c has litre;p=c;p has cubic;").unwrap();
    assert_eq!(
        model.compartments()[0]
            .size()
            .expect("explicit initialized test model"),
        2.0
    );
    assert_eq!(
        model.parameters()[0]
            .value()
            .expect("explicit initialized test model"),
        2.0
    );
    assert_eq!(model.symbol_dimensions("p"), model.symbol_dimensions("c"));
    let model = compile("unit later=base;unit base=second;v=2;v has later;").unwrap();
    assert_eq!(
        model
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>(),
        ["later", "base"]
    );
    assert_eq!(
        model.unit_definitions()[0].components()[0].kind().as_str(),
        "second"
    );
}

#[test]
fn unknown_cyclic_redefined_and_colliding_units_fail_closed() {
    for text in [
        "unit u=missing;p=2;p has u;",
        "p=2;p has missing;",
        "unit u=v;unit v=u;p=2;p has u;",
        "unit u=second;unit u=mole;p=2;p has u;",
        "unit u=second;u=2;",
        "p=2;p has mole;p has second;",
    ] {
        rejected(text, None);
    }
}

#[test]
fn unsupported_metadata_and_unbounded_forms_never_disappear() {
    for text in [
        "time_unit=second;p=2;",
        "unit u=second^0.5;p=2;",
        "unit u=second^2147483647;p=2;",
        "unit u=second^0;p=2;",
        "unit u=0 second;p=2;",
        "unit u=1e300 second;p=2;",
        "unit u=second/second;p=2;",
        "unit u=(mole/second);p=2;",
        "p=2;p has mole/second;",
    ] {
        rejected(text, None);
    }
}

#[test]
fn unit_alias_chains_obey_entity_budget_and_detect_cycles_iteratively() {
    let mut text = String::new();
    for i in 0..64 {
        text.push_str(&format!("unit u{i}=u{};", i + 1));
    }
    text.push_str("unit u64=second;p=2;p has u0;");
    let model = compile(&text).unwrap();
    assert_eq!(model.unit_definitions().len(), 65);
    let error = compile_with(
        &text,
        &Limits::default().with_limit(LimitKind::ModelEntities, 32),
    )
    .unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    rejected(&text.replace("unit u64=second", "unit u64=u0"), None);
}

#[test]
fn repeated_threaded_units_have_no_shared_registry() {
    let threads: Vec<_> = (0..4)
        .map(|thread| {
            std::thread::spawn(move || {
                for n in 1..=12 {
                    let kind = if thread % 2 == 0 { "second" } else { "mole" };
                    let text = format!("unit local={kind};p={n};p has local;");
                    let model = compile(&text).unwrap();
                    assert_eq!(
                        model.unit_definitions()[0].components()[0].kind().as_str(),
                        kind
                    );
                    assert_eq!(
                        model.parameters()[0]
                            .value()
                            .expect("explicit initialized test model"),
                        f64::from(n)
                    );
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
fn units_export_keeps_alias_metadata_and_initial_assignments() {
    let model = compile("unit small=0.001 second;a=2;a has small;b=a+a;b has small;").unwrap();
    let xml = antimony::write_sbml(&model).unwrap();
    assert!(xml.xml().contains("units=\"small\""));
    assert!(xml.xml().contains("symbol=\"b\""));
    assert!(!xml.xml().contains("assignmentRule"));
    assert_eq!(xml.xml(), antimony::write_sbml(&model).unwrap().xml());
}

/// Frozen independent live-oracle probe inputs; see the units review for projection evidence.
#[test]
fn frozen_units_review_probe_acceptance() {
    for (name, text, accepted) in [
        ("scale_low", "unit tiny=1e-12 second;p=7;p has tiny;", true),
        ("scale_high", "unit huge=1e12 mole;p=7;p has huge;", true),
        (
            "scale_decimal",
            "unit odd=1.2300 metre;p=7;p has odd;",
            true,
        ),
        (
            "scale_leadingzeros",
            "unit odd=0002.50 second;p=7;p has odd;",
            true,
        ),
        (
            "scaled_alias_power",
            "unit small=0.001 second;unit square=small^2;p=7;p has square;",
            true,
        ),
        (
            "composite_inverse",
            "unit speed=metre/second;unit inverse=1/speed;p=7;p has inverse;",
            true,
        ),
        (
            "resolved_limit",
            "unit u=metre^16;unit v=u^4;p=7;p has v;",
            true,
        ),
        (
            "resolved_excess",
            "unit u=metre^16;unit v=u^4;unit w=v^2;p=7;p has w;",
            false,
        ),
        (
            "dimensionless_component",
            "unit u=dimensionless*mole;p=7;p has u;",
            true,
        ),
        (
            "order_annotation",
            "p=7;p has later;unit base=second;unit later=base;",
            true,
        ),
        (
            "multi_quotient",
            "unit u=mole/second/metre;p=7;p has u;",
            true,
        ),
        (
            "quotient_negative",
            "unit u=mole/second^-2;p=7;p has u;",
            true,
        ),
        (
            "volume_dimensionless",
            "compartment c=2;c has dimensionless;p=c;",
            true,
        ),
        ("unit_only", "unit u=second;", true),
        (
            "annotated_builtin_symbol",
            "second=2;p=3;p has second;",
            false,
        ),
        (
            "known_nested_mismatch",
            "a=2;a has mole;b=3;b has second;c=(a-b)*2;",
            false,
        ),
        (
            "dimensioned_exponent",
            "a=2;a has mole;b=3;b has second;c=a^b;",
            false,
        ),
        (
            "unknown_exponent",
            "a=2;a has mole;b=3;c=a^b;c has second;",
            true,
        ),
    ] {
        let result = compile(text);
        assert_eq!(result.is_ok(), accepted, "{name}: {result:?}");
        if !accepted {
            rejected(text, None);
        }
    }
}
