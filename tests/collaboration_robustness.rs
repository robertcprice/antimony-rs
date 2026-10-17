//! Collaboration & robustness verification suite covering the combined Phase 1-5
//! flat-model language slice (`functions-core-v0.5-slice`).
//!
//! Exercises realistic hand-authored pathway models, incomplete-to-complete
//! editing workflows, format roundtrip byte-idempotence, explicit deferred-scope
//! diagnostics, and bounded resource/mutation hardening across user-defined
//! functions, assignment/rate rules, conditionals/piecewise, builtins/time,
//! exact rationals, source-ordered symbols, and unit checks.

use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionKind, LimitKind, Limits,
    ParsedDocument, RuleKind, Severity, SourceFile, SourceId, ValidatedModel, analyze,
    format_antimony, parse,
};

fn parse_with_limits(text: &str, limits: &Limits) -> Result<ParsedDocument, Diagnostic> {
    let source = SourceFile::new(
        SourceId::new(101),
        "collaboration.ant",
        text,
        &Limits::default(),
    )
    .expect("bounded test input");
    parse(&source, limits)
}

fn compile_with_limits(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let parsed = parse_with_limits(text, limits)?;
    analyze(&parsed)
}

fn compile(text: &str) -> Result<ValidatedModel, Diagnostic> {
    compile_with_limits(text, &Limits::default())
}

fn assert_idempotent_roundtrip(text: &str) -> ValidatedModel {
    let limits = Limits::default();
    let doc = parse_with_limits(text, &limits)
        .unwrap_or_else(|e| panic!("expected valid parse, got: {e:?}"));
    let model =
        analyze(&doc).unwrap_or_else(|e| panic!("expected valid model analysis, got: {e:?}"));
    let formatted_once = format_antimony(&doc)
        .unwrap_or_else(|e| panic!("expected format_antimony success, got: {e:?}"));

    let doc_re = parse_with_limits(&formatted_once, &limits)
        .unwrap_or_else(|e| panic!("formatted output failed to re-parse: {e:?}"));
    let model_re =
        analyze(&doc_re).unwrap_or_else(|e| panic!("formatted output failed to re-analyze: {e:?}"));
    let formatted_twice = format_antimony(&doc_re)
        .unwrap_or_else(|e| panic!("formatted output failed to re-format: {e:?}"));
    assert_eq!(
        formatted_once, formatted_twice,
        "format_antimony must be byte-idempotent"
    );
    assert_eq!(model.id(), model_re.id());
    assert_eq!(model.rules().len(), model_re.rules().len());
    #[cfg(feature = "sbml")]
    {
        let sbml_orig = antimony::write_sbml(&model).expect("SBML export on original");
        let sbml_fmt = antimony::write_sbml(&model_re).expect("SBML export on formatted");
        assert_eq!(
            sbml_orig.xml(),
            sbml_fmt.xml(),
            "SBML export must be identical before and after canonical formatting"
        );
    }
    model
}

#[test]
fn unnamed_conversion_chain_with_reusable_saturating_rates_and_conserved_total() {
    let src = "\
function mm(S, Vm, Km)
  Vm * S / (Km + S)
end

function hill(S, Vm, K, n)
  Vm * (S ^ n) / ((K ^ n) + (S ^ n))
end

model linear_cascade()
  compartment cell = 1.5;
  species S1 in cell, S2 in cell, S3 in cell;
  S1 = 10;
  S2 = 0;
  S3 = 0;

  v_in = 0.8;
  Vm1 = 2.5;
  Km1 = 1.2;
  Vm2 = 1.8;
  K2 = 0.9;
  n2 = 2;
  k_out = 0.15;

  -> S1; v_in;
  S1 -> S2; mm(S1, Vm1, Km1);
  S2 -> S3; hill(S2, Vm2, K2, n2);
  S3 -> ; k_out * S3;

  S_total := S1 + S2 + S3;
  active_fraction := S3 / (S_total + 1e-9);
end
";
    let model = assert_idempotent_roundtrip(src);
    assert_eq!(model.function_definitions().len(), 2);
    assert_eq!(model.reactions().len(), 4);
    assert_eq!(model.reactions()[0].id(), "_J0");
    assert_eq!(model.reactions()[1].id(), "_J1");
    assert_eq!(model.reactions()[2].id(), "_J2");
    assert_eq!(model.reactions()[3].id(), "_J3");
    assert_eq!(model.rules().len(), 2);
    assert_eq!(model.rules()[0].target(), "S_total");
    assert_eq!(model.rules()[0].kind(), RuleKind::Assignment);
    assert_eq!(model.rules()[1].target(), "active_fraction");
}

#[test]
fn small_rate_constants_and_exact_rationals_in_covalent_modification_cycle() {
    let src = "\
function rev_mm(S, P, Vf, Vr, KmS, KmP)
  (Vf * (S / KmS) - Vr * (P / KmP)) / (1 + (S / KmS) + (P / KmP))
end

compartment c = 1;
species W in c, Wp in c;
W = 5e-6;
Wp = 1e-9;

Vf_kin = 2.5e-6;
Vr_kin = 1.25e-8;
Km_W = 1e-6;
Km_Wp = 2e-6;
k_basal = 1e-9;

J_phos: W -> Wp; rev_mm(W, Wp, Vf_kin, Vr_kin, Km_W, Km_Wp);
J_dephos: Wp -> W; (1 / 2) * Vf_kin * Wp / (Km_Wp + Wp) + k_basal * Wp;

W_tot := W + Wp;
scaled_ratio := (3 / 4) * (Wp / W_tot);
";
    let model = assert_idempotent_roundtrip(src);
    assert_eq!(model.function_definitions().len(), 1);
    let w = model.species().iter().find(|s| s.id() == "W").unwrap();
    assert_eq!(w.initial_concentration(), Some(5e-6));
    let wp = model.species().iter().find(|s| s.id() == "Wp").unwrap();
    assert_eq!(wp.initial_concentration(), Some(1e-9));
    assert_eq!(model.rules().len(), 2);
}

#[test]
fn time_dependent_input_piecewise_stimulus_and_rate_controlled_state() {
    let src = "\
function dose_response(u, ec50)
  u / (ec50 + u)
end

species X, Y;
X = 1.0;
Y = 0.2;

v_low = 0.1;
v_high = 1.5;
k_clear = 0.3;
ec50 = 0.5;
omega = 0.5;

stimulus := piecewise(v_high, (time >= 2) && (time < 6), v_low) * (1 + 0.25 * sin(omega * time));
X' = stimulus - k_clear * X;

J_act: -> Y; dose_response(X, ec50);
J_deg: Y -> ; 0.4 * Y;
";
    let model = assert_idempotent_roundtrip(src);
    assert_eq!(model.rules().len(), 2);
    let stim_rule = model
        .rules()
        .iter()
        .find(|r| r.target() == "stimulus")
        .unwrap();
    assert_eq!(stim_rule.kind(), RuleKind::Assignment);
    let x_rate = model.rules().iter().find(|r| r.target() == "X").unwrap();
    assert_eq!(x_rate.kind(), RuleKind::Rate);
    let x_spec = model.species().iter().find(|s| s.id() == "X").unwrap();
    assert_eq!(x_spec.initial_concentration(), Some(1.0));
    assert!(!x_spec.boundary_condition());
}

#[test]
fn incomplete_model_later_filled_in_and_edited_preserves_contract() {
    // Stage 1: structurally valid incomplete draft (unset species and parameters).
    let incomplete_src = "\
function mm(S, Vm, Km)
  Vm * S / (Km + S)
end

compartment cell;
species S in cell, P in cell;
const Vm, Km;

J1: S -> P; mm(S, Vm, Km);
total_pool := S + P;
";
    let draft = assert_idempotent_roundtrip(incomplete_src);
    assert_eq!(draft.compartments()[0].size(), None);
    for s in draft.species() {
        assert_eq!(
            s.initial_concentration(),
            None,
            "unset species {} must not default to 0.0",
            s.id()
        );
    }
    for p in draft.parameters() {
        assert_eq!(
            p.value(),
            None,
            "unset or rule-derived parameter {} must have None scalar value",
            p.id()
        );
    }
    #[cfg(feature = "sbml")]
    {
        let xml = antimony::write_sbml(&draft).unwrap().xml().to_owned();
        assert!(!xml.contains("initialConcentration=\"0\""));
        assert!(!xml.contains("value=\"0\""));
    }

    // Stage 2: user fills in initial concentrations, compartment size, and kinetic constants.
    let completed_src =
        format!("{incomplete_src}\ncell = 2.0;\nS = 8.0;\nP = 0.5;\nVm = 3.0;\nKm = 1.5;\n");
    let filled = assert_idempotent_roundtrip(&completed_src);
    assert_eq!(filled.compartments()[0].size(), Some(2.0));
    assert_eq!(
        filled
            .species()
            .iter()
            .find(|s| s.id() == "S")
            .unwrap()
            .initial_concentration(),
        Some(8.0)
    );
    assert_eq!(
        filled
            .species()
            .iter()
            .find(|s| s.id() == "P")
            .unwrap()
            .initial_concentration(),
        Some(0.5)
    );
    assert_eq!(
        filled
            .parameters()
            .iter()
            .find(|p| p.id() == "Vm")
            .unwrap()
            .value(),
        Some(3.0)
    );
    assert_eq!(
        filled
            .parameters()
            .iter()
            .find(|p| p.id() == "total_pool")
            .unwrap()
            .value(),
        None,
        "compound assignment rule target must still expose None for static scalar value()"
    );
}

#[test]
fn unit_annotated_pathway_with_functions_and_rules_validates_cleanly() {
    let src = "\
function first_order(k, c)
  k * c
end

function weighted_sum(a, b)
  a + b
end

unit mM = 1e-3 mole / litre;
unit per_sec = 1 / second;

S = 4;
S has mM;
P = 1;
P has mM;
k = 0.25;
k has per_sec;

S' = -first_order(k, S);
P' = first_order(k, S);
total_conc := weighted_sum(S, P);
total_conc has mM;
";
    let model = assert_idempotent_roundtrip(src);
    assert_eq!(model.function_definitions().len(), 2);
    assert_eq!(model.rules().len(), 3);
    assert!(matches!(
        model.symbol_dimensions("total_conc"),
        Some(DimensionalStatus::Known(_))
    ));
}

#[test]
fn source_order_boundary_promotion_and_rule_replacement_in_pathway() {
    let src = "\
// S_bound is assigned a rule BEFORE appearing as a reaction participant -> promoted to boundary
S_bound := 2 + sin(time);
// S_free is declared as a species BEFORE getting a rate rule (not modified by reactions) -> stays non-boundary
species S_free = 5;
species P_out = 0;
k = 0.3;
J1: S_bound -> P_out; k * S_bound * S_free;
S_free' = -0.1 * S_free;

// Repeated assignment rule replaces formula while preserving earlier inferred symbol u_old
obs := u_old + 1;
obs := 2 * S_free;
";
    let model = assert_idempotent_roundtrip(src);
    let s_bound = model
        .species()
        .iter()
        .find(|s| s.id() == "S_bound")
        .unwrap();
    assert!(
        s_bound.boundary_condition(),
        "rule target promoted to species by later reaction must become boundaryCondition=true"
    );
    let s_free = model.species().iter().find(|s| s.id() == "S_free").unwrap();
    assert!(
        !s_free.boundary_condition(),
        "already-declared species given a rate rule without $ must stay boundaryCondition=false"
    );
    assert!(
        model.parameters().iter().any(|p| p.id() == "u_old"),
        "symbol inferred from overwritten rule expression must remain in parameter table"
    );
    let obs_rule = model.rules().iter().find(|r| r.target() == "obs").unwrap();
    assert!(matches!(
        model.expression(obs_rule.expression()).unwrap().kind(),
        ExpressionKind::Binary { .. }
    ));
}

#[test]
fn explicit_deferred_scope_constructs_and_negative_near_neighbors_reject_deterministically() {
    for newly_supported in [
        "S = 10; k = 0.1; S -> ; k * S; at (time > 5): S = 0;",
        "compartment c = 1; c := 1 + 0.1 * time;",
        "compartment c = 1; c' = 0.05;",
        "species S = 1, P = 0; k1 = 0.2; k2 = 0.4; J0: S -> P; k1 * S; J0 := k2 * S;",
        "model sub()\n  S = 1;\nend\nmodel main()\n  A: sub();\nend\n",
    ] {
        assert!(compile(newly_supported).is_ok(), "{newly_supported}");
    }

    let cases: &[(&str, &str, DiagnosticCode)] = &[
        (
            "reaction_id_rate_rule_override",
            "species S = 1, P = 0; k1 = 0.2; J0: S -> P; k1 * S; J0' = 0.1;",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "algebraic_rule_zero_lhs",
            "S = 1; P = 2; 0 = S + P - 3;",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "import_statement",
            "import \"external_submodel\"; S = 1;",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "is_annotation",
            "species S = 1; S is \"glucose\";",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "rate_of_call",
            "species S = 1; k = 0.2; S' = -k * S; r := rateOf(S);",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "forward_user_function_call",
            "a = f(2);\nfunction f(x)\n  x + 1\nend\n",
            DiagnosticCode::UnsupportedFeature,
        ),
        (
            "direct_recursive_function",
            "function rec(x)\n  piecewise(1, x <= 0, rec(x - 1))\nend\na = 1;\n",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "function_capturing_global_parameter",
            "k = 2;\nfunction scale(x)\n  k * x\nend\na = scale(3);\n",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "function_capturing_time",
            "function temporal(x)\n  x + time\nend\na := temporal(1);\n",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "transient_assignment_rule_cycle",
            "a := b; b := a; a := 1;",
            DiagnosticCode::InitializationCycle,
        ),
        (
            "mixed_assign_and_rate_rule",
            "x := 2; x' = 0.5;",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "initial_after_assign_rule",
            "x := 2 + 1; x = 5;",
            DiagnosticCode::DeclarationConflict,
        ),
        (
            "unit_mismatch_through_user_function",
            "function add(x, y)\n  x + y\nend\na = 1; a has second;\nb = 2; b has metre;\nc = add(a, b);\n",
            DiagnosticCode::UnitsInconsistency,
        ),
        (
            "rate_rule_unit_mismatch",
            "unit time_unit = second;\nx = 1; x has metre;\nk = 2; k has second;\nx' = k;\n",
            DiagnosticCode::UnitsInconsistency,
        ),
    ];

    for &(name, src, expected_code) in cases {
        let err = compile(src).expect_err(&format!("case `{name}` must be rejected"));
        assert_eq!(
            err.code(),
            expected_code,
            "case `{name}` diagnostic code mismatch: got {err:?}"
        );
        assert_eq!(err.severity(), Severity::Error);
        assert!(!err.labels().is_empty());
        for label in err.labels() {
            assert!(
                src.get(label.span().range()).is_some(),
                "case `{name}` span out of bounds"
            );
        }
    }
}

#[test]
fn deep_function_chain_and_interleaved_rules_respect_configured_limits() {
    let mut lines = Vec::new();
    lines.push("function f0(x)\n  x + 1\nend".to_owned());
    for i in 1..16 {
        lines.push(format!("function f{i}(x)\n  f{}(x) + 1\nend", i - 1));
    }
    lines.push("a = f15(0);".to_owned());
    lines.push("r := f15(a);".to_owned());
    let src = lines.join("\n");

    let model = assert_idempotent_roundtrip(&src);
    assert_eq!(model.function_definitions().len(), 16);
    assert_eq!(
        model
            .parameters()
            .iter()
            .find(|p| p.id() == "a")
            .unwrap()
            .value(),
        Some(16.0)
    );

    // Enforcing a tighter ModelEntities limit cleanly rejects with ResourceLimit.
    let tight_entities = Limits::default().with_limit(LimitKind::ModelEntities, 8);
    assert_eq!(
        compile_with_limits(&src, &tight_entities)
            .unwrap_err()
            .code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn mutations_and_utf8_truncations_on_combined_phase5_pathway_never_panic() {
    let seed = "\
function mm(S, Vm, Km)
  Vm * S / (Km + S)
end
species S = 4, P = 0;
Vm = 2; Km = 1;
J0: S -> P; mm(S, Vm, Km) * piecewise(1, time < 5, 1 / 2);
tot := S + P;
P' = -0.05 * P;
";
    let bounded = Limits::default()
        .with_limit(LimitKind::Tokens, 256)
        .with_limit(LimitKind::ExpressionDepth, 24)
        .with_limit(LimitKind::ModelEntities, 32);

    let boundaries: Vec<usize> = seed
        .char_indices()
        .map(|(i, _)| i)
        .chain([seed.len()])
        .collect();

    // Every UTF-8 prefix truncation must either validate or return an in-bounds diagnostic.
    for &offset in &boundaries {
        let prefix = &seed[..offset];
        match parse_with_limits(prefix, &bounded) {
            Ok(doc) => match analyze(&doc) {
                Ok(_m) => {
                    let _ = format_antimony(&doc);
                    #[cfg(feature = "sbml")]
                    let _ = antimony::write_sbml(&_m).unwrap();
                }
                Err(diag) => {
                    for label in diag.labels() {
                        assert!(prefix.get(label.span().range()).is_some());
                    }
                }
            },
            Err(diag) => {
                for label in diag.labels() {
                    assert!(prefix.get(label.span().range()).is_some());
                }
            }
        }
    }

    // Targeted token insertions at every 4th boundary.
    for &offset in boundaries.iter().step_by(4) {
        for tok in [":=", "'=", "end", "function", "(", ")", ";", "/"] {
            let mutated = format!("{}{}{}", &seed[..offset], tok, &seed[offset..]);
            match parse_with_limits(&mutated, &bounded) {
                Ok(doc) => match analyze(&doc) {
                    Ok(_m) => {
                        let _ = format_antimony(&doc);
                        #[cfg(feature = "sbml")]
                        let _ = antimony::write_sbml(&_m).unwrap();
                    }
                    Err(diag) => {
                        for label in diag.labels() {
                            assert!(mutated.get(label.span().range()).is_some());
                        }
                    }
                },
                Err(diag) => {
                    for label in diag.labels() {
                        assert!(mutated.get(label.span().range()).is_some());
                    }
                }
            }
        }
    }
}
