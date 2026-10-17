//! Integration tests for pure-Rust stoichiometry matrix extraction (`StoichiometryMatrix`),
//! RK4 ODE + discrete-event simulation (`simulate`, `SimulationConfig`, `Trajectory`),
//! steady-state solving (`steady_state`, `SteadyStateConfig`, `SteadyState`), and
//! Metabolic Control Analysis (MCA) elasticity matrices (`ElasticityMatrix`).

use antimony::{
    DiagnosticCode, Limits, SimulationConfig, SourceFile, SourceId, SteadyStateConfig, analyze,
    parse, simulate,
};
use std::fs;
use std::path::Path;

fn compile_str(src: &str) -> antimony::ValidatedModel {
    let limits = Limits::default();
    let sf = SourceFile::new(SourceId::new(1), "test.ant", src, &limits).expect("source");
    let parsed = parse(&sf, &limits).expect("parse");
    analyze(&parsed).expect("analyze")
}

#[test]
fn pure_rust_simulator_matches_closed_form_analytical_solutions() {
    // 1. Basic event bolus: S(t) = exp(-0.3*t) for t <= 4, and (exp(-1.2) + 5)*exp(-0.3*(t-4)) for t > 4
    let m1 = compile_str(include_str!(
        "../fixtures/modular/cases/event_unnamed_basic_bolus.ant"
    ));
    let t1 = m1.simulate(&SimulationConfig::new(0.0, 10.0, 10)).unwrap();
    let s_at_4 = t1.value_at(4, "S").unwrap();
    assert!((s_at_4 - (-1.2_f64).exp()).abs() < 1e-6);
    let s_at_10 = t1.value_at(10, "S").unwrap();
    let expected_10 = ((-1.2_f64).exp() + 5.0) * (-1.8_f64).exp();
    assert!((s_at_10 - expected_10).abs() < 1e-6);

    // 2. Dynamic compartment rate rule growth: C(t) = exp(0.05*t), [S](t) = 10*exp(-0.15*t)
    let m2 = compile_str(include_str!(
        "../fixtures/modular/cases/compartment_rate_rule_growth.ant"
    ));
    let t2 = m2.simulate(&SimulationConfig::new(0.0, 10.0, 10)).unwrap();
    for step in 0..=10 {
        let time = step as f64;
        let c_val = t2.value_at(step, "C").unwrap();
        let s_val = t2.value_at(step, "S").unwrap();
        assert!((c_val - (0.05 * time).exp()).abs() < 1e-6);
        assert!((s_val - 10.0 * (-0.15 * time).exp()).abs() < 1e-5);
    }

    // 3. Delayed event with fromTrigger=true (`event_delay_after_trigger.ant`)
    let m3 = compile_str(include_str!(
        "../fixtures/modular/cases/event_delay_after_trigger.ant"
    ));
    let t3 = m3.simulate(&SimulationConfig::new(0.0, 10.0, 10)).unwrap();
    let s3_at_4 = t3.value_at(4, "S").unwrap();
    let expected_s3_at_4 = (5.0 * (-0.4_f64).exp() + 3.0) * (-0.1_f64).exp();
    assert!(
        (s3_at_4 - expected_s3_at_4).abs() < 1e-5,
        "got {s3_at_4}, expected {expected_s3_at_4}"
    );

    // 4. Event targeting both compartment and species (`event_targeting_compartment_and_species.ant`)
    let m4 = compile_str(include_str!(
        "../fixtures/modular/cases/event_targeting_compartment_and_species.ant"
    ));
    let t4 = m4.simulate(&SimulationConfig::new(0.0, 10.0, 10)).unwrap();
    let s4_at_4 = t4.value_at(4, "S").unwrap();
    let expected_s4_at_4 = (4.0 * (-0.6_f64).exp() / 4.0) * (-0.2_f64).exp();
    assert!(
        (s4_at_4 - expected_s4_at_4).abs() < 1e-5,
        "got {s4_at_4}, expected {expected_s4_at_4}"
    );
}

#[test]
fn steady_state_and_mca_elasticity_matrix_match_analytical_values() {
    // Open two-step pathway with conserved moiety cycle:
    //   $X0 -> S1; k0 * X0;
    //   S1 -> S2; Vm * S1 / (Km + S1);
    //   S2 -> $X1; k2 * S2;
    // At steady state:
    //   v0 = v1 = v2 = k0 * X0 = 0.2 * 5.0 = 1.0
    //   Vm * S1* / (Km + S1*) = 1.0  =>  2.0 * S1* / (1.5 + S1*) = 1.0  =>  S1* = 1.5
    //   k2 * S2* = 1.0               =>  0.5 * S2* = 1.0                 =>  S2* = 2.0
    // Scaled Michaelis-Menten elasticity at S1* = 1.5, Km = 1.5:
    //   e_{S1}^{J1} = Km / (Km + S1*) = 1.5 / (1.5 + 1.5) = 0.5
    // Unscaled elasticity at S1* = 1.5:
    //   dv1 / dS1 = Vm * Km / (Km + S1*)^2 = 2.0 * 1.5 / 9.0 = 1/3
    let src = "\
model pathway()
  species $X0 = 5.0, S1 = 0.2, S2 = 0.1, $X1 = 0.0;
  k0 = 0.2;
  Vm = 2.0;
  Km = 1.5;
  k2 = 0.5;
  J0: $X0 -> S1; k0 * X0;
  J1: S1 -> S2; Vm * S1 / (Km + S1);
  J2: S2 -> $X1; k2 * S2;
end
";
    let model = compile_str(src);
    let ss = model
        .steady_state(&SteadyStateConfig::default())
        .expect("steady_state should succeed");
    assert!(ss.converged(), "residual = {}", ss.max_rate_residual());
    assert!((ss.value("S1").unwrap() - 1.5).abs() < 1e-7);
    assert!((ss.value("S2").unwrap() - 2.0).abs() < 1e-7);
    assert!((ss.reaction_rate("J0").unwrap() - 1.0).abs() < 1e-7);
    assert!((ss.reaction_rate("J1").unwrap() - 1.0).abs() < 1e-7);
    assert!((ss.reaction_rate("J2").unwrap() - 1.0).abs() < 1e-7);

    let elast = ss.elasticity_matrix(&model).expect("elasticity_matrix");
    assert_eq!(elast.reactions(), &["J0", "J1", "J2"]);
    assert_eq!(elast.floating_species(), &["S1", "S2"]);
    assert!((elast.scaled("J1", "S1").unwrap() - 0.5).abs() < 1e-6);
    assert!((elast.unscaled("J1", "S1").unwrap() - (1.0 / 3.0)).abs() < 1e-6);
    assert!((elast.scaled("J2", "S2").unwrap() - 1.0).abs() < 1e-6);
    assert!((elast.unscaled("J2", "S2").unwrap() - 0.5).abs() < 1e-6);
    assert!(elast.scaled("J1", "S2").unwrap().abs() < 1e-10);
}

#[test]
fn simulate_all_complete_modular_and_collaboration_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut simulated_count = 0usize;

    for dir_rel in ["fixtures/collaboration/cases", "fixtures/modular/cases"] {
        let dir = root.join(dir_rel);
        let mut entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("ant") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.starts_with("neg_") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let model = compile_str(&text);
            let config = SimulationConfig::new(0.0, 10.0, 20);
            if name.contains("incomplete") {
                assert_eq!(
                    simulate(&model, &config).unwrap_err().code(),
                    DiagnosticCode::UnsupportedFeature,
                    "incomplete fixture {name} must be rejected by simulate()"
                );
            } else {
                let traj = simulate(&model, &config)
                    .unwrap_or_else(|d| panic!("simulate failed on {name}: {}", d.message()));
                assert_eq!(traj.rows().len(), 21);
                for row in traj.rows() {
                    assert!(
                        row.iter().all(|v| v.is_finite()),
                        "non-finite value in trajectory for {name}"
                    );
                }
                let stoich = model.stoichiometry_matrix();
                assert_eq!(stoich.num_cols(), model.reactions().len());
                simulated_count += 1;
            }
        }
    }
    assert_eq!(
        simulated_count, 49,
        "expected all 49 complete modular + collaboration models simulated, got {simulated_count}"
    );
}
