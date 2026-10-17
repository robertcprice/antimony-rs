//! Bounded deterministic hardening, not a fuzz-coverage or performance claim.
use antimony::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, Severity, SourceFile, SourceId, ValidatedModel,
    analyze, parse,
};

fn compile(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source = SourceFile::new(
        SourceId::new(73),
        "$(not-a-command)",
        text,
        &Limits::default(),
    )
    .expect("bounded test input");
    analyze(&parse(&source, limits)?)
}

fn exercise(text: &str) {
    let limits = Limits::default()
        .with_limit(LimitKind::Tokens, 256)
        .with_limit(LimitKind::ExpressionDepth, 24)
        .with_limit(LimitKind::ModelEntities, 32);
    match compile(text, &limits) {
        Ok(model) => {
            assert_eq!(model.source().text(), text);
            assert!(
                model
                    .species()
                    .iter()
                    .all(|s| s.initial_concentration().is_none_or(f64::is_finite))
            );
            assert!(
                model
                    .parameters()
                    .iter()
                    .all(|p| p.value().is_none_or(f64::is_finite))
            );
            #[cfg(feature = "sbml")]
            {
                let first = antimony::write_sbml(&model).unwrap();
                let second = antimony::write_sbml(&model).unwrap();
                assert_eq!(first.xml(), second.xml());
                assert_eq!(first.warnings(), model.warnings());
            }
        }
        Err(diagnostic) => {
            assert_eq!(diagnostic.severity(), Severity::Error);
            assert!(!diagnostic.labels().is_empty());
            for label in diagnostic.labels() {
                assert_eq!(label.span().source_id(), SourceId::new(73));
                assert!(text.get(label.span().range()).is_some());
            }
        }
    }
}

#[test]
fn generated_numeric_models_preserve_independent_initial_values() {
    for case in 1..=64 {
        let text = format!(
            "compartment c={}; species S in c,P in c; S={case}; P=0; k=0.25; J:S=>P;k*S*c;",
            case % 7 + 1
        );
        let model = compile(&text, &Limits::default()).unwrap();
        assert_eq!(
            model
                .species()
                .iter()
                .find(|s| s.id() == "S")
                .unwrap()
                .initial_concentration()
                .expect("explicit initialized test model"),
            f64::from(case)
        );
        assert_eq!(
            model.compartments()[0]
                .size()
                .expect("explicit initialized test model"),
            f64::from(case % 7 + 1)
        );
        assert_eq!(
            model.parameters()[0]
                .value()
                .expect("explicit initialized test model"),
            0.25
        );
        exercise(&text);
    }
}

#[test]
fn every_single_scalar_mutation_keeps_diagnostics_and_export_safe() {
    let seed = "/* β😀 */ S=2;P=0;k=0.25;J:S=>P;k*S;";
    let boundaries: Vec<usize> = seed
        .char_indices()
        .map(|(i, _)| i)
        .chain([seed.len()])
        .collect();
    for &offset in &boundaries {
        for inserted in [";", "'", "β", "(", "\0", "=", "/*"] {
            exercise(&format!(
                "{}{}{}",
                &seed[..offset],
                inserted,
                &seed[offset..]
            ));
        }
    }
    for pair in boundaries.windows(2) {
        exercise(&format!("{}{}", &seed[..pair[0]], &seed[pair[1]..]));
    }
}

#[test]
fn seeded_short_inputs_are_bounded_and_repeatable() {
    const CHARS: &[char] = &[
        'S', '1', 'e', '+', '-', '^', '/', '*', ';', ':', '=', '$', '(', ')', '\n', 'β',
    ];
    let mut state = 0x8f31_c42a_u32;
    for _ in 0..256 {
        let mut text = String::new();
        for _ in 0..48 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            text.push(CHARS[((state >> 16) as usize) % CHARS.len()]);
        }
        exercise(&text);
        exercise(&text);
    }
}

#[test]
fn utf8_boundary_truncations_never_escape_source_spans() {
    for seed in [
        "model m()\nS=1;P=0;J:S=>P;S;\nend",
        "/* β😀 */ S=1;J:S=>;S;",
        "a=b+2;b=3;S=1;J:S=>;a*S;",
        "import \"not-read\";",
    ] {
        for offset in seed.char_indices().map(|(i, _)| i).chain([seed.len()]) {
            exercise(&seed[..offset]);
        }
    }
}

#[test]
fn repeated_concurrent_models_do_not_leak_state() {
    let threads = (1..=8)
        .map(|worker| {
            std::thread::spawn(move || {
                for iteration in 0..16 {
                    let value = worker * 100 + iteration;
                    let text = format!("S={value};P=0;J:S=>P;S;");
                    let first = compile(&text, &Limits::default()).unwrap();
                    let second = compile(&text, &Limits::default()).unwrap();
                    assert_eq!(
                        first
                            .species()
                            .iter()
                            .find(|s| s.id() == "S")
                            .unwrap()
                            .initial_concentration()
                            .expect("explicit initialized test model"),
                        f64::from(value)
                    );
                    assert_eq!(first.source().text(), second.source().text());
                    #[cfg(feature = "sbml")]
                    assert_eq!(
                        antimony::write_sbml(&first).unwrap().xml(),
                        antimony::write_sbml(&second).unwrap().xml()
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn growing_input_stops_at_configured_limits_without_partial_model() {
    let text = (0..64).map(|i| format!("p{i}=1;\n")).collect::<String>();
    for limits in [
        Limits::default().with_limit(LimitKind::SourceBytes, 32),
        Limits::default().with_limit(LimitKind::Tokens, 32),
        Limits::default().with_limit(LimitKind::ModelEntities, 16),
        Limits::default().with_limit(LimitKind::Diagnostics, 0),
    ] {
        assert_eq!(
            compile(&text, &limits).unwrap_err().code(),
            DiagnosticCode::ResourceLimit
        );
    }
    let rate = std::iter::repeat_n("S", 64).collect::<Vec<_>>().join("+");
    assert_eq!(
        compile(
            &format!("S=1;J:S=>;{rate};"),
            &Limits::default().with_limit(LimitKind::ExpressionDepth, 16)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn generated_forward_initialization_chain_is_order_independent_and_bounded() {
    let statements = (0..48)
        .map(|i| {
            if i == 0 {
                "p0=1;".to_owned()
            } else {
                format!("p{i}=p{}+1;", i - 1)
            }
        })
        .collect::<Vec<_>>();
    let forward = statements.join("\n");
    let reversed = statements
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let one = compile(&forward, &Limits::default()).unwrap();
    let two = compile(&reversed, &Limits::default()).unwrap();
    let values = |m: &ValidatedModel| {
        m.parameters()
            .iter()
            .map(|p| {
                (
                    p.id().to_owned(),
                    p.value().expect("explicit initialized test model"),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(values(&one), values(&two));
    assert_eq!(
        one.parameters()
            .iter()
            .find(|p| p.id() == "p47")
            .unwrap()
            .value()
            .expect("explicit initialized test model"),
        48.0
    );
    assert_eq!(one.initial_assignments().len(), 47);
    assert_eq!(two.initial_assignments().len(), 47);
    assert_eq!(
        compile(
            &reversed,
            &Limits::default().with_limit(LimitKind::ModelEntities, 24)
        )
        .unwrap_err()
        .code(),
        DiagnosticCode::ResourceLimit
    );
    let cycle = forward.replacen("p0=1;", "p0=p47+1;", 1);
    assert_eq!(
        compile(&cycle, &Limits::default()).unwrap_err().code(),
        DiagnosticCode::InitializationCycle
    );
    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&one).unwrap();
        assert_eq!(xml.xml().matches("<initialAssignment ").count(), 47);
        assert!(!xml.xml().contains("<assignmentRule"));
    }
}

#[test]
fn frozen_initialization_review_probe_acceptance() {
    // Exact bounded independent differential inputs, retained for repeatable review.
    let probes = [
        ("alias", "a=b;b=2;", true),
        ("diamond", "a=b+c;b=d+1;c=d+2;d=3;", true),
        ("self_cycle", "a=a+1;", false),
        ("cycle_downstream", "down=a+2;a=b;b=a;", false),
        ("negative_fraction", "a=(-2)^0.5;", false),
        ("zero_zero", "a=0^0;", false),
        ("zero_neg", "a=0^-1;", false),
        ("negative_integer", "a=(-2)^3;", true),
        ("div_negzero", "z=-0;a=1/z;", false),
        ("finite_after_overflow", "big=1e308;a=(big*big)/big;", false),
        (
            "forward_mixed_volume",
            "compartment c=3;species S in c,P in c;P=S*2;S=k/c;k=6;J:S=>P;k*S;",
            true,
        ),
        (
            "boundary_chain",
            "const species S;S=k+1;k=2;a=S;b=a+1;",
            true,
        ),
        ("normal_chain", "species S;S=k+1;k=2;a=S;b=a+1;", true),
        (
            "legal_reaction_cycle",
            "S=k+1;P=0;k=2;J:S=>P;S;K:P=>S;P;",
            true,
        ),
        (
            "forward_declarations",
            "a=S+1;S=k/c;k=6;species S in c;compartment c=3;",
            true,
        ),
        ("missing_unconsumed", "a=missing+1;", true),
        (
            "volume_expression",
            "compartment c=k*2;k=3;species S in c;S=2;",
            true,
        ),
        ("negative_species", "species S;S=1-2;", false),
    ];
    for (name, text, accepted) in probes {
        assert_eq!(
            compile(text, &Limits::default()).is_ok(),
            accepted,
            "{name}"
        );
        exercise(text);
    }
}
