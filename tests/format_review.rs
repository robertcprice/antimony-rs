//! Independent canonical formatting invariants, not a lossless/comment-preserving editor.
use antimony::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, SourceId, ValidatedModel, analyze,
    format_antimony, parse,
};

fn format_with(text: &str, limits: &Limits) -> Result<String, Diagnostic> {
    let source =
        SourceFile::new(SourceId::new(93), "format-review", text, &Limits::default()).unwrap();
    format_antimony(&parse(&source, limits)?)
}
fn compile(text: &str) -> ValidatedModel {
    let source = SourceFile::new(SourceId::new(94), "roundtrip", text, &Limits::default()).unwrap();
    analyze(&parse(&source, &Limits::default()).unwrap()).unwrap()
}
fn roundtrip(text: &str) -> String {
    let original = compile(text);
    let formatted = format_with(text, &Limits::default()).unwrap();
    let reparsed = compile(&formatted);
    assert_eq!(
        formatted,
        format_with(&formatted, &Limits::default()).unwrap()
    );
    assert_eq!(original.id(), reparsed.id());
    assert_eq!(original.profile_id(), reparsed.profile_id());
    assert_eq!(
        original
            .initial_assignments()
            .iter()
            .map(|x| x.symbol())
            .collect::<Vec<_>>(),
        reparsed
            .initial_assignments()
            .iter()
            .map(|x| x.symbol())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        original
            .parameters()
            .iter()
            .map(|p| (
                p.id(),
                p.value().expect("explicit initialized test model"),
                p.constant()
            ))
            .collect::<Vec<_>>(),
        reparsed
            .parameters()
            .iter()
            .map(|p| (
                p.id(),
                p.value().expect("explicit initialized test model"),
                p.constant()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        original
            .species()
            .iter()
            .map(|s| (
                s.id(),
                s.initial_concentration()
                    .expect("explicit initialized test model"),
                s.boundary_condition()
            ))
            .collect::<Vec<_>>(),
        reparsed
            .species()
            .iter()
            .map(|s| (
                s.id(),
                s.initial_concentration()
                    .expect("explicit initialized test model"),
                s.boundary_condition()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        original
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>(),
        reparsed
            .unit_definitions()
            .iter()
            .map(|u| u.id())
            .collect::<Vec<_>>()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&original).unwrap().xml(),
        antimony::write_sbml(&reparsed).unwrap().xml()
    );
    formatted
}

#[test]
fn formatting_strips_only_nonsemantic_trivia_and_is_idempotent() {
    let out = roundtrip("/* β😀 */\n a /*between*/ = 2 ; // trailing\n b = a + 1 ;\n");
    assert!(!out.contains("β"));
    assert!(!out.contains("/*"));
    assert!(!out.contains("//"));
    assert!(out.ends_with('\n'));
}

#[test]
fn named_and_anonymous_envelopes_are_preserved() {
    let anon = roundtrip("a=2;b=a+1");
    assert!(!anon.contains("model"));
    let named = roundtrip("/*before*/model named()\na=2;b=a+1\nend\n//after");
    assert!(named.starts_with("model named()"));
    assert!(named.trim_end().ends_with("end"));
}

#[test]
fn literal_and_initial_assignment_modes_never_collapse() {
    let text = "a=-2;b=+2;c=(2);d=(-2);e=2;f=2+0;";
    let out = roundtrip(text);
    assert!(out.contains("+2"));
    assert!(out.contains("(2)"));
    assert_eq!(
        compile(&out)
            .initial_assignments()
            .iter()
            .map(|x| x.symbol())
            .collect::<Vec<_>>(),
        ["b", "c", "d", "f"]
    );
}

#[test]
fn numeric_lexemes_and_operator_binding_are_preserved() {
    for text in [
        "a=1.;b=1e3;c=1e-3;d=+1.;e=(-1e-3);",
        "a=2;b=3;c=4;x=-a^2;y=a^(b^2);z=(a^b)^2;",
        "a=2;b=3;c=4;x=a-(b-c);y=a/(b/c);z=(a+b)*c;",
        "a=2;b=3;c=4;x=a+b+c;y=a*b*c;z=a^-2;",
    ] {
        roundtrip(text);
    }
}

#[test]
fn reactions_keep_participant_separator_arrows_and_boundary_marks() {
    for text in [
        "S=2;P=0;k=1;J:S=>P;k*S;",
        "S=2;P=0;k=1;J:$S->P;k*S;",
        "S=2;k=1;J:=>S;k;K:S=>;k*S;",
        "S=2;P=0;k=1;J:2 S=>0.5 P;k*S;",
    ] {
        roundtrip(text);
    }
}

#[test]
fn original_unit_aliases_defaults_and_generated_order_survive() {
    for text in [
        "unit base=mole/litre;unit conc=base;species S;S=2;S has conc;",
        "unit zed=second;species B,A;B=2;A=3;B has mole;A has dimensionless;",
        "unit volume=vol;unit vol=0.001 litre;species S;S=2;",
        "unit custom=litre*mole;species S;S=2;S has mole;",
        "unit small=0.001 second;unit square=small^2;p=7;p has square;",
    ] {
        roundtrip(text);
    }
}

#[test]
fn first_symbol_occurrence_in_references_is_not_sorted_away() {
    let out = roundtrip("z=a*2;a=b+1;b=3;");
    assert_eq!(
        compile(&out)
            .initial_assignments()
            .iter()
            .map(|x| x.symbol())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    roundtrip("x=A*B;species B,A;B=2;A=3;B has mole;A has dimensionless;");
}

#[test]
fn lexical_comment_adjacency_cannot_create_new_tokens() {
    for text in [
        "a/*x*/=/*x*/2;b=a/*x*/+/*x*/1;",
        "unit u=1/*x*//second;p=2;p has u;",
        "S=2;P=0;k=1;J/*x*/:S/*x*/=>/*x*/P; k/*x*/*/*x*/S;",
        "model/*x*/m(/*x*/)\na=2\nend",
    ] {
        roundtrip(text);
    }
}

#[test]
fn parseable_but_invalid_models_never_format_successfully() {
    for text in [
        "a=missing+1/0;",
        "a=b;b=a;",
        "a=1/0;",
        "unit u=v;unit v=u;a=2;",
        "unit volume=dimensionless;species S;S=2;S has mole;",
        "unit mole_litre=second;species S;S=2;S has mole;",
    ] {
        let error = format_with(text, &Limits::default()).expect_err(text);
        assert!(!error.labels().is_empty());
        for label in error.labels() {
            assert_eq!(label.span().source_id(), SourceId::new(93));
            assert!(text.get(label.span().range()).is_some());
        }
    }
}

#[test]
fn output_expansion_budget_error_uses_original_utf8_source_spans() {
    let text = "/*β*/a=2+3;";
    let limits = Limits::default().with_limit(LimitKind::SourceBytes, text.len());
    // Removing comments can offset whitespace growth: use a separate compact case.
    let compact = "a=2+3;";
    let error = format_with(
        compact,
        &Limits::default().with_limit(LimitKind::SourceBytes, compact.len()),
    )
    .unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    for label in error.labels() {
        assert_eq!(label.span().source_id(), SourceId::new(93));
        assert!(compact.get(label.span().range()).is_some());
    }
    match format_with(text, &limits) {
        Ok(out) => {
            assert!(out.len() <= text.len());
        }
        Err(error) => {
            assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
            for label in error.labels() {
                assert!(text.get(label.span().range()).is_some());
            }
        }
    }
}

#[test]
fn concurrent_formatting_is_repeatable_without_shared_state() {
    let threads: Vec<_> = (0..4)
        .map(|thread| {
            std::thread::spawn(move || {
                for n in 1..=12 {
                    roundtrip(&format!("model m{thread}()\na={n};b=a+1;\nend"));
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn generated_token_budget_error_maps_to_original_source_not_generated_bytes() {
    // The original has five tokens including EOF. Canonical LF adds a token.
    let text = "a=2;";
    let limits = Limits::default().with_limit(LimitKind::Tokens, 5);
    let error = format_with(text, &limits).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::ResourceLimit);
    assert!(!error.labels().is_empty());
    for label in error.labels() {
        assert_eq!(label.span().source_id(), SourceId::new(93));
        assert!(text.get(label.span().range()).is_some());
    }
}

/// Original live-reference adversarial inputs; freeze token/layout semantics.
#[test]
fn frozen_format_adversarial_inputs() {
    for (name, text) in [
        ("named_semicolons", "model m();a=2;b=a+1;end"),
        ("crlf", "model m()\r\na=2;\r\nb=a+1\r\nend\r\n"),
        ("empty_separators", ";;\n a=2;;;\n\n b=a+1;;;"),
        ("eof_reaction_rate", "S=2;P=0;k=1;J:S=>P;k*S"),
        (
            "comment_operators",
            "S=2;P=0;k=1;J/*x*/:S/*x*/=>/*x*/P; k/*x*/*/*x*/S;",
        ),
        ("comment_reciprocal", "unit u=1/*x*//second;p=2;p has u;"),
        ("negative_power_spaces", "a=2;b=a ^ - 2;"),
        ("ia_wrapped_signed", "a=-2;b=+2;c=(2);d=(-2);e=2;f=2+0;"),
        ("leading_zero_literal", "a=0002.50;b=a+1;"),
        ("large_scalar", "a=1e308;"),
        (
            "unit_juxtaposition",
            "unit small=0.002mole;p=2;p has small;",
        ),
        (
            "unit_alias_plural",
            "unit vol=liters;compartment c=2;c has vol;species S in c;S=2;S has mole;",
        ),
        ("boundary_const", "const species S;S=2;P=0;k=1;J:S->P;k*S;"),
        ("unspaced_source_arrow", "S=2;k=1;J:=>S;k;"),
        (
            "unicode_trivia",
            "/*β😀\nmodel end;*/model m()\na=2;//β\nb=a+1;\nend",
        ),
        (
            "raw_unit_default_alias",
            "unit base=mole/litre;unit conc=base;unit volume=0.001 litre;species S;S=2;S has conc;",
        ),
        (
            "first_order_references",
            "x=A*B;species B,A;B=2;A=3;B has mole;A has dimensionless;",
        ),
    ] {
        let output = roundtrip(text);
        assert!(!output.is_empty(), "{name}");
    }
}

#[test]
fn scalar_export_envelope_rejects_without_clamping_across_roles() {
    for text in [
        "a=1e-308;",
        "a=-1e-308;",
        "a=2.2250738585072014e-308;",
        "a=1e-999;",
        "a=-1e-999;",
        "a=1.1e308;",
        "a=1.234567890123456789;",
        "a=1.0000000000000002;",
        "compartment c=1.234567890123456;",
        "species S;S=1.234567890123456;",
        "S=2;P=0;J:1.234567890123456 S=>P;S;",
        "compartment c=1e-308;",
        "species S;S=1e-308;",
        "species S;S=1e-999;",
        "S=2;P=0;J:1e-308 S=>P;S;",
        "S=2;P=0;J:S=>1e-308 P;S;",
        "a=1e-999+1;",
        "S=2;P=0;J:S=>P;1e-999*S;",
    ] {
        let error = format_with(text, &Limits::default()).expect_err(text);
        assert!(!error.labels().is_empty());
        for label in error.labels() {
            assert_eq!(label.span().source_id(), SourceId::new(93));
            assert!(text.get(label.span().range()).is_some());
        }
    }
}

#[test]
fn verified_scalar_endpoints_and_exact_zero_still_roundtrip() {
    for text in [
        "a=1e-307;",
        "a=-1e-307;",
        "a=1e308;",
        "a=-1e308;",
        "a=0;",
        "a=-0;",
        "a=0e-999;",
        "a=1.23456789012345;",
        "a=1.00000000000000;",
    ] {
        roundtrip(text);
    }
}
