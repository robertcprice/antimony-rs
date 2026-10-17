//! Reviewer-authored Phase 2 regressions: absence, dependency safety and source order.
use antimony::{
    Diagnostic, DiagnosticCode, ExpressionKind, LimitKind, Limits, SourceFile, SourceId,
    ValidatedModel, analyze, format_antimony, parse,
};

fn compile_with(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(SourceId::new(204), "independent symbols", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}
fn compile(text: &str) -> ValidatedModel {
    compile_with(text, &Limits::default()).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}
fn parameter<'a>(m: &'a ValidatedModel, id: &str) -> &'a antimony::Parameter {
    m.parameters().iter().find(|p| p.id() == id).unwrap()
}
fn species<'a>(m: &'a ValidatedModel, id: &str) -> &'a antimony::Species {
    m.species().iter().find(|p| p.id() == id).unwrap()
}

#[test]
fn absent_zero_and_symbolic_are_three_distinct_states() {
    let m = compile("species A,B,C;B=0;C=A+2;formula p,q,r;q=0;r=p+2;A=>B;p*A;");
    assert_eq!(species(&m, "A").initial_concentration(), None);
    assert_eq!(species(&m, "B").initial_concentration(), Some(0.0));
    assert_eq!(species(&m, "C").initial_concentration(), None);
    assert!(species(&m, "A").initial_expression().is_none());
    assert!(species(&m, "C").initial_expression().is_some());
    assert_eq!(parameter(&m, "p").value(), None);
    assert_eq!(parameter(&m, "q").value(), Some(0.0));
    assert_eq!(parameter(&m, "r").value(), None);
    assert!(parameter(&m, "p").initial_expression().is_none());
    assert!(parameter(&m, "r").initial_expression().is_some());
}

#[test]
fn symbolic_chain_keeps_every_initial_assignment() {
    let m = compile("a=b+2;c=a*3;formula b;");
    assert!(m.parameters().iter().all(|p| p.value().is_none()));
    assert_eq!(m.initial_assignments().len(), 2);
    assert!(m.parameters().iter().all(|p| p.constant()));
}

#[test]
fn filling_in_unknown_dependency_resolves_entire_chain() {
    let m = compile("a=b+2;c=a*3;formula b;b=4;");
    assert_eq!(parameter(&m, "a").value(), Some(6.0));
    assert_eq!(parameter(&m, "c").value(), Some(18.0));
    assert_eq!(m.initial_assignments().len(), 2);
}

#[test]
fn unset_is_not_simplified_to_zero_or_one() {
    let m = compile("formula u;a=u*0;b=u^0;c=1/u;");
    for id in ["a", "b", "c"] {
        assert_eq!(parameter(&m, id).value(), None);
        assert!(parameter(&m, id).initial_expression().is_some());
    }
}

#[test]
fn unset_compartment_is_not_implicit_default_size() {
    let m = compile("compartment cell;species A in cell,B;A=cell*2;A=>B;k*A;");
    assert_eq!(
        m.compartments()
            .iter()
            .find(|c| c.id() == "cell")
            .unwrap()
            .size(),
        None
    );
    assert_eq!(
        m.compartments()
            .iter()
            .find(|c| c.id() == "default_compartment")
            .unwrap()
            .size(),
        Some(1.0)
    );
    assert_eq!(species(&m, "A").initial_concentration(), None);
    assert!(species(&m, "A").initial_expression().is_some());
}

#[test]
fn explicit_initialization_overrides_symbolic_value_not_symbol_history() {
    let m = compile("a=b+1;a=7;");
    assert_eq!(parameter(&m, "a").value(), Some(7.0));
    assert_eq!(parameter(&m, "b").value(), None);
    assert!(m.initial_assignments().is_empty());
}

#[test]
fn unused_superseded_expression_does_not_create_false_cycle() {
    let m = compile("a=b+1;b=a+1;a=2;");
    assert_eq!(parameter(&m, "a").value(), Some(2.0));
    assert_eq!(parameter(&m, "b").value(), Some(3.0));
}

#[test]
fn unset_operand_does_not_hide_dependency_cycles() {
    for text in [
        "formula u;a=a+u;",
        "formula u;a=b+u;b=a+1;",
        "z=a+1;formula u;a=b+u;b=a+1;",
    ] {
        let e = compile_with(text, &Limits::default()).unwrap_err();
        assert_eq!(e.code(), DiagnosticCode::InitializationCycle, "{text}");
    }
}

#[test]
fn known_invalid_subexpressions_are_not_masked_by_unset_operands() {
    for (text, expected) in [
        // Parenthesized integer ratios are rational atoms, whose zero denominator
        // is an unsupported nonfinite literal, not a divide operation.
        ("formula u;a=u+(1/0);", DiagnosticCode::UnsupportedFeature),
        ("formula u;a=(1/0)+u;", DiagnosticCode::UnsupportedFeature),
        (
            "formula u;a=u+((1)/0);",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "formula u;a=((1)/0)+u;",
            DiagnosticCode::DeclarationConflict,
        ),
        ("formula u;a=u/0;", DiagnosticCode::DeclarationConflict),
        (
            "formula u;a=u+(-1)^0.5;",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "formula u;a=u+(1e200*1e200);",
            DiagnosticCode::DeclarationConflict,
        ),
    ] {
        let e = compile_with(text, &Limits::default()).unwrap_err();
        assert_eq!(e.code(), expected, "{text}");
        for label in e.labels() {
            assert!(text.get(label.span().range()).is_some());
        }
    }
}

#[test]
fn generated_name_respects_prior_expression_and_unit_references() {
    for text in ["a=_J0+1;A=>B;k*A;", "_J0 has second;A=>B;k*A;"] {
        let m = compile(text);
        assert_eq!(m.reactions()[0].id(), "_J1");
        assert_eq!(parameter(&m, "_J0").value(), None);
    }
}

#[test]
fn generated_names_skip_reserved_gaps_without_reserving_future_names() {
    let m = compile("formula _J1;A=>B;k*A;B=>C;k*B;C=>D;k*C;");
    assert_eq!(
        m.reactions().iter().map(|r| r.id()).collect::<Vec<_>>(),
        ["_J0", "_J2", "_J3"]
    );
}

#[test]
fn overwritten_kinetic_law_retains_old_inferred_symbols() {
    let m = compile("A=>B;k*A;_J0=q*B;");
    assert_eq!(parameter(&m, "k").value(), None);
    assert_eq!(parameter(&m, "q").value(), None);
    let rate = m.expression(m.reactions()[0].rate()).unwrap();
    let ExpressionKind::Binary { left, right, .. } = rate.kind() else {
        panic!("binary law")
    };
    for (node, expected) in [(*left, "q"), (*right, "B")] {
        assert!(
            matches!(m.expression(node).unwrap().kind(), ExpressionKind::Symbol { name, .. } if name == expected)
        );
    }
}

#[test]
fn promoted_unset_species_controls_dependent_parameter_constness() {
    let m = compile("formula A;derived=A+1;A=>B;k*A;");
    assert_eq!(species(&m, "A").initial_concentration(), None);
    assert_eq!(parameter(&m, "derived").value(), None);
    assert!(!parameter(&m, "derived").constant());
}

#[test]
fn symbolic_initialization_remains_bounded_with_long_chain() {
    let mut text = String::from("formula missing;");
    for i in 0..256 {
        let previous = if i == 0 {
            "missing".to_owned()
        } else {
            format!("p{}", i - 1)
        };
        text.push_str(&format!("p{i}={previous}+1;"));
    }
    let m = compile(&text);
    assert_eq!(m.parameters().len(), 257);
    assert_eq!(m.initial_assignments().len(), 256);
    assert!(m.parameters().iter().all(|p| p.value().is_none()));
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 64);
    assert_eq!(
        compile_with(&text, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn anonymous_reaction_formatting_preserves_semantics_and_idempotence() {
    for text in [
        "/* β */ formula _J1; A=>B; /* rate */ k*A\n B=>; k*B;",
        "model trial()\ncompartment c;species A in c,B;=>A;input;A=>B;k*A;end",
        "formula u; a=u+2; a=7; A=>;a*A;_J0=q*A;",
    ] {
        let limits = Limits::default();
        let source = SourceFile::new(SourceId::new(204), "original", text, &limits).unwrap();
        let parsed = parse(&source, &limits).unwrap();
        let formatted = format_antimony(&parsed).unwrap();
        let other = SourceFile::new(SourceId::new(204), "formatted", &formatted, &limits).unwrap();
        let parsed_other = parse(&other, &limits).unwrap();
        assert_eq!(format_antimony(&parsed_other).unwrap(), formatted);
        #[cfg(feature = "sbml")]
        assert_eq!(
            antimony::write_sbml(&analyze(&parsed).unwrap())
                .unwrap()
                .xml(),
            antimony::write_sbml(&analyze(&parsed_other).unwrap())
                .unwrap()
                .xml()
        );
    }
}

#[cfg(feature = "sbml")]
#[test]
fn exported_unset_values_remain_absent_not_fabricated_zero() {
    let m = compile("compartment c;species A in c;formula k;A=>;k*A;");
    let artifact = antimony::write_sbml(&m).unwrap();
    let xml = artifact.xml();
    assert!(!xml.contains("initialConcentration="));
    assert!(!xml.contains("initialAmount="));
    assert!(!xml.contains("size="));
    assert!(!xml.contains("value="));
    assert!(!xml.contains("<initialAssignment "));
}

#[test]
fn unit_definition_names_reserve_generated_reaction_ids() {
    let m = compile("unit _J0=second;A=>B;k*A;");
    assert_eq!(m.reactions()[0].id(), "_J1");
    assert!(!m.parameters().iter().any(|p| p.id() == "_J0"));
}

#[test]
fn scalar_law_reassignment_uses_same_numeric_meaning_as_literal_law() {
    for literal in ["9.999e-5", "-9.999e-5", "1.23456789012345e-123"] {
        let before = compile(&format!("A=>B;{literal};"));
        let after = compile(&format!("A=>B;1;_J0={literal};"));
        fn leaf_value(m: &ValidatedModel) -> f64 {
            let mut node = m.expression(m.reactions()[0].rate()).unwrap();
            if let ExpressionKind::Unary { child, .. } = node.kind() {
                node = m.expression(*child).unwrap();
            }
            match node.kind() {
                ExpressionKind::Number { value, .. } => *value,
                other => panic!("numeric law expected, got {other:?}"),
            }
        }
        assert_eq!(
            leaf_value(&before).to_bits(),
            leaf_value(&after).to_bits(),
            "{literal}"
        );
        #[cfg(feature = "sbml")]
        assert_eq!(
            antimony::write_sbml(&before).unwrap().xml(),
            antimony::write_sbml(&after).unwrap().xml()
        );
    }
}

#[test]
fn unit_mentions_reserve_names_before_later_definitions() {
    for text in [
        "unit clock=_J0;A=>B;k*A;unit _J0=second;",
        "a has _J0;A=>B;k*A;unit _J0=second;",
    ] {
        let m = compile(text);
        assert_eq!(m.reactions()[0].id(), "_J1", "{text}");
        assert!(!m.parameters().iter().any(|p| p.id() == "_J0"));
    }
}

#[test]
fn internal_reaction_separator_does_not_admit_extra_statements() {
    for text in ["A=>B;\nk*A;", "J:A=>B;\nk*A;", "A=>B;;k*A;"] {
        assert!(compile_with(text, &Limits::default()).is_err(), "{text}");
    }
    compile("A=>B;/* comment\n spanning lines */k*A;");
    compile("A=>B;k*A\nB=>;k*B;");
}

#[test]
fn redeclared_reaction_provenance_retains_first_occurrence() {
    let text = "J:A=>B;k*A;J:B=>C;q*B;";
    let m = compile(text);
    let span = m.reactions()[0].source_span();
    assert_eq!(span.start(), text.find('J').unwrap());
    assert_eq!(text.get(span.range()), Some("J"));
    assert_eq!(m.reactions()[0].reactants()[0].species(), "B");
}
