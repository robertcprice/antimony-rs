//! Canonical source layout and semantic/resource invariants.
use antimony::{
    DiagnosticCode, LimitKind, Limits, ParsedDocument, SourceFile, SourceId, analyze,
    format_antimony, parse,
};

fn parsed(text: &str, limits: &Limits) -> ParsedDocument {
    let source = SourceFile::new(SourceId::new(81), "not/a/path;$(false)", text, limits).unwrap();
    parse(&source, limits).unwrap()
}
fn canonical(text: &str) -> String {
    let limits = Limits::default();
    let original = parsed(text, &limits);
    let before = analyze(&original).unwrap();
    let output = format_antimony(&original).unwrap();
    let after = parsed(&output, &limits);
    let after_model = analyze(&after).unwrap();
    assert_eq!(format_antimony(&after).unwrap(), output);
    assert_eq!(before.id(), after_model.id());
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&before).unwrap().xml(),
        antimony::write_sbml(&after_model).unwrap().xml(),
        "source: {text}\nformatted: {output}"
    );
    output
}

#[test]
fn named_layout_and_reaction_rate_separators() {
    assert_eq!(
        canonical(";\r\nmodel demo ( ) ;species A=2,B=0; k=.2;J:A=>B;/*\r\n*/ k*A;end;;;"),
        "model demo()\n  species A = 2, B = 0;\n  k = .2;\n  J: A => B; k * A;\nend\n"
    );
}
#[test]
fn anonymous_envelope_is_not_invented() {
    assert_eq!(canonical("\n;;k=1"), "k = 1;\n");
    assert_eq!(
        canonical("model __main();k=1 end"),
        "model __main()\n  k = 1;\nend\n"
    );
}
#[test]
fn unicode_comments_are_discarded_without_joining_tokens() {
    assert_eq!(
        canonical("/*微生物*/ species/*α*/S=2;// 🧪\r\nJ:2/*×*/S=>;S/*ignored\nnewline*/+1;"),
        "species S = 2;\nJ: 2 S =>; S + 1;\n"
    );
}
#[test]
fn source_boundary_spelling_and_declaration_groups_survive() {
    assert_eq!(
        canonical("const species A=1,$B=2;species $C=3;J:$A+2$B->$C;A;"),
        "const species A = 1, $B = 2;\nspecies $C = 3;\nJ: $A + 2 $B -> $C; A;\n"
    );
}
#[test]
fn source_sink_and_reversible_reactions_survive() {
    let output = canonical("species A=1;J:=>A;2;K:A->;A;");
    assert!(output.contains("J: => A; 2;\n"));
    assert!(output.contains("K: A ->; A;\n"));
}
#[test]
fn every_scalar_number_lexeme_survives() {
    assert_eq!(
        canonical("a=0002.;b=.50;c=2E+03;d=-0.0;e=1e-200;f=1.23456789012345;"),
        "a = 0002.;\nb = .50;\nc = 2E+03;\nd = -0.0;\ne = 1e-200;\nf = 1.23456789012345;\n"
    );
}
#[test]
fn literal_versus_initial_assignment_is_never_normalized() {
    let text = "a=-2;b=+2;c=(2);d=((-2));e=2+0;";
    let output = canonical(text);
    assert_eq!(
        output,
        "a = -2;\nb = +2;\nc = (2);\nd = ((-2));\ne = 2 + 0;\n"
    );
    assert_eq!(
        analyze(&parsed(&output, &Limits::default()))
            .unwrap()
            .initial_assignments()
            .len(),
        4
    );
}
#[test]
fn explicit_parentheses_and_operator_association_survive() {
    assert_eq!(
        canonical("a=2;b=3;c=4;r=(-a^2)+(b-c)/(a/b)^-2;"),
        "a = 2;\nb = 3;\nc = 4;\nr = (-a ^ 2) + (b - c) / (a / b) ^ -2;\n"
    );
}
#[test]
fn parenthesized_unary_and_subtraction_remain_distinct() {
    assert_eq!(
        canonical("a=2;b=a-(-2);c=a+(-a);d=-(a+2);e=2^-2;"),
        "a = 2;\nb = a - (-2);\nc = a + (-a);\nd = -(a + 2);\ne = 2 ^ -2;\n"
    );
}
#[test]
fn unit_juxtaposition_explicit_multiply_reciprocal_and_exponents_survive() {
    let text = "unit small=1e-3 mole;unit large=2*litre;unit inverse=1/second;unit length2=metre^2;unit flow=mole*litre^-1/second;k=1;";
    let output = canonical(text);
    assert!(output.contains("unit small = 1e-3 mole;"));
    assert!(output.contains("unit large = 2 * litre;"));
    assert!(output.contains("unit inverse = 1 / second;"));
    assert!(output.contains("unit length2 = metre ^ 2;"));
    assert!(output.contains("unit flow = mole * litre ^ -1 / second;"));
}
#[test]
fn all_six_default_unit_declarations_and_aliases_survive() {
    let text = "unit substance=mole;unit volume=litre;unit area=metre^2;unit length=metre;unit extent=mole;unit time_unit=second;unit copy=substance;k=1;";
    let output = canonical(text);
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("unit "))
            .count(),
        7
    );
    assert!(output.contains("unit copy = substance;"));
}
#[test]
fn source_species_units_do_not_become_generated_sbml_definitions() {
    let output = canonical("unit conc=base;unit base=mole/litre;species S=2;S has conc;");
    assert_eq!(
        output,
        "unit conc = base;\nunit base = mole / litre;\nspecies S = 2;\nS has conc;\n"
    );
}
#[test]
fn first_occurrence_and_forward_declaration_order_survive() {
    let text = "k=B+A;species A=2,B=3;A has mole;B has mole;";
    let output = canonical(text);
    assert!(output.starts_with("k = B + A;\nspecies A = 2, B = 3;"));
}
#[test]
fn explicit_compartment_and_formula_declarations_survive() {
    assert_eq!(
        canonical(
            "compartment cell=2.00;cell has litre;formula k;k=2;species S in cell=3;J:S=>;k*S;"
        ),
        "compartment cell = 2.00;\ncell has litre;\nformula k;\nk = 2;\nspecies S in cell = 3;\nJ: S =>; k * S;\n"
    );
}
#[test]
fn format_semantic_rejections_are_not_pretty_printed_successfully() {
    assert_eq!(
        format_antimony(&parsed("compartment c=(2);", &Limits::default())).unwrap(),
        "compartment c = (2);\n"
    );
    for (text, code) in [
        ("a=b/0;", DiagnosticCode::DeclarationConflict),
        ("a=b;b=a;", DiagnosticCode::InitializationCycle),
        ("compartment c=0;", DiagnosticCode::DeclarationConflict),
        (
            "species a;compartment a;",
            DiagnosticCode::DeclarationConflict,
        ),
    ] {
        let d = format_antimony(&parsed(text, &Limits::default())).unwrap_err();
        assert_eq!(d.code(), code, "{text}");
    }
}
#[test]
fn source_expansion_budget_is_inclusive_and_uses_original_locations() {
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, 6);
    let doc = parsed("k=1;", &limits);
    let d = format_antimony(&doc).unwrap_err();
    assert_eq!(d.code(), DiagnosticCode::ResourceLimit);
    for label in d.labels() {
        assert_eq!(label.span().source_id(), SourceId::new(81));
        doc.source().slice(label.span()).unwrap();
    }
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, 7);
    assert_eq!(
        format_antimony(&parsed("k=1;", &limits)).unwrap(),
        "k = 1;\n"
    );
}
#[test]
fn token_expansion_budget_is_rechecked_and_remapped_to_original_source() {
    let limits = Limits::default().with_limit(LimitKind::Tokens, 5);
    let doc = parsed("k=1;", &limits);
    let d = format_antimony(&doc).unwrap_err();
    assert_eq!(d.code(), DiagnosticCode::ResourceLimit);
    assert!(!d.notes().is_empty());
    for label in d.labels() {
        doc.source().slice(label.span()).unwrap();
    }
    let limits = Limits::default().with_limit(LimitKind::Tokens, 6);
    assert_eq!(
        format_antimony(&parsed("k=1;", &limits)).unwrap(),
        "k = 1;\n"
    );
}
#[test]
fn deterministic_repeated_and_concurrent_calls_share_no_state() {
    let doc = std::sync::Arc::new(parsed("model repeat();k=(2+3);end", &Limits::default()));
    let expected = format_antimony(&doc).unwrap();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let doc = doc.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                for _ in 0..16 {
                    assert_eq!(format_antimony(&doc).unwrap(), expected);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
#[test]
fn formatting_large_flat_source_is_bounded_and_iterative() {
    let mut text = String::new();
    for n in 0..2000 {
        text.push_str(&format!("p{n}={n};"));
    }
    assert_eq!(canonical(&text).lines().count(), 2000);
}
#[test]
fn trivia_variants_converge_to_the_same_canonical_layout() {
    let variants = [
        "a=2;b=(a+3);",
        " a = 2\r b = ( a + 3 ) ",
        "/* α */a/* β */=/* γ */2;;;\r\nb=(a+3)//last",
    ];
    let expected = canonical(variants[0]);
    for text in variants {
        assert_eq!(canonical(text), expected);
    }
}

#[test]
fn unsupported_syntax_is_rejected_before_formatting() {
    let limits = Limits::default();
    assert_eq!(
        format_antimony(&parsed("k=0; at (k>1):k=2;", &limits)).unwrap(),
        "k = 0;\nat (k > 1): k = 2;\n"
    );
    for text in [
        "k:=;",
        "import \"secret.ant\";",
        "λ=1;",
        "k=delay(2,1);",
        "k=--2;",
        "k=1e999;",
        "k=/*unterminated",
    ] {
        let source = SourceFile::new(SourceId::new(81), "untrusted", text, &limits).unwrap();
        assert!(parse(&source, &limits).is_err(), "{text}");
    }
}
#[test]
fn exhausted_input_depth_and_entity_limits_never_produce_output() {
    let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 2);
    let source = SourceFile::new(SourceId::new(81), "depth", "k=(((2)));", &limits).unwrap();
    assert_eq!(
        parse(&source, &limits).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::ModelEntities, 0);
    assert_eq!(
        format_antimony(&parsed("k=1;", &limits))
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
    let limits = Limits::default().with_limit(LimitKind::Diagnostics, 0);
    assert_eq!(
        format_antimony(&parsed("k=1;", &limits))
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn scalar_envelope_rejects_underflow_subnormals_and_unverified_extremes_in_all_roles() {
    let limits = Limits::default();
    for number in [
        "1e-999",
        "5e-324",
        "1e-308",
        "2.2250738585072014e-308",
        "1.7976931348623157e308",
    ] {
        for text in [
            format!("p={number};"),
            format!("p=-{number};"),
            format!("species S={number};"),
            format!("compartment c={number};"),
            format!("species S=1;J:{number} S=>;S;"),
        ] {
            let source =
                SourceFile::new(SourceId::new(81), "scalar envelope", &text, &limits).unwrap();
            let d = parse(&source, &limits).unwrap_err();
            assert_eq!(
                d.code(),
                DiagnosticCode::UnsupportedFeature,
                "{}",
                source.text()
            );
            for label in d.labels() {
                source.slice(label.span()).unwrap();
            }
        }
    }
}
#[test]
fn scalar_envelope_endpoints_and_lexical_zero_preserve_original_values() {
    for number in ["1e-307", "1e308"] {
        for text in [
            format!("p={number};"),
            format!("p=-{number};"),
            format!("species S={number};"),
            format!("compartment c={number};"),
            format!("species S=1;J:{number} S=>;S;"),
        ] {
            assert!(canonical(&text).contains(number));
        }
    }
    assert_eq!(
        canonical("a=-0;b=0e-999;c=-000.00E-999;"),
        "a = -0;\nb = 0e-999;\nc = -000.00E-999;\n"
    );
}

#[test]
fn excess_scalar_precision_is_rejected_consistently_in_every_role() {
    let limits = Limits::default();
    for number in [
        "1.234567890123456",
        "1.0000000000000002",
        "1000000000000001",
        "0.10000000000000002",
        "0001.000000000000000",
    ] {
        for text in [
            format!("p={number};"),
            format!("p=-{number};"),
            format!("species S={number};"),
            format!("compartment c={number};"),
            format!("species S=1;J:{number} S=>;S;"),
            format!("species S=1;J:=>{number} S;S;"),
        ] {
            let source =
                SourceFile::new(SourceId::new(81), "precision envelope", &text, &limits).unwrap();
            let d = parse(&source, &limits).unwrap_err();
            assert_eq!(d.code(), DiagnosticCode::UnsupportedFeature, "{text}");
            assert!(d.message().contains("15 significant digits"));
        }
    }
}
#[test]
fn significant_digit_count_ignores_leading_zeros_and_exponent_not_trailing_zeros() {
    for number in [
        "1.23456789012345",
        "0001.23456789012345",
        "000.000000123456789012345",
        "1.23456789012345e100",
        "1.00000000000000",
        "0.00000000000000000000e-999",
    ] {
        assert!(canonical(&format!("p={number};")).contains(number));
    }
}
