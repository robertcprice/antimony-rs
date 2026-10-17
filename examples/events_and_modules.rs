//! Ordinary-user example demonstrating Phases 6–8 (`modular-core-v0.8-slice`):
//! - `substanceOnly` species, dynamic compartments (`=`, `:=`, `'=`), and reaction rate overrides
//! - discrete `at` events with `delay after`, `t0`, `persistent`, `fromTrigger`, and `priority`
//! - reusable modular submodels with ports, `M.x` member overrides, and `is` aliases

use antimony::{Limits, SourceFile, SourceId, analyze, format_antimony, parse};

fn main() {
    let source_text = "\
function mm(S, Vm, Km)
  Vm * S / (Km + S)
end

model phosphorylation_module(Sub, Prod, k_cat)
  compartment c = 1.0;
  species Sub in c = 10.0;
  species Prod in c = 0.0;
  k_cat = 0.5;
  Km = 2.0;
  J_phos: Sub -> Prod; mm(Sub, k_cat, Km);
  E_boost: at 0.5 after (time >= 4.0), t0 = false, persistent = true, fromTrigger = true, priority = 1: k_cat = 1.2;
end

model *signaling_pathway()
  k_vol = 0.05;
  compartment cytosol = 2.0;
  cytosol' = k_vol * cytosol;
  species MAPK in cytosol = 15.0;
  species MAPKp in cytosol = 0.0;
  species MAPKpp in cytosol = 0.0;
  substanceOnly species DrugBolus in cytosol = 5.0;

  k_fast = 0.8;
  Stage1: phosphorylation_module(MAPK, MAPKp, k_fast);
  Stage2: phosphorylation_module(MAPKp, MAPKpp, k_slow);
  Stage2.Km = 0.75;
  km_stage1 is Stage1.Km;
  km_stage1 = 1.5;

  J_clear: DrugBolus -> ; 0.1 * DrugBolus;
  J_clear := 0.25 * DrugBolus;
  E_washout: at (time >= 8.0): DrugBolus = 0.0;
end
";

    let limits = Limits::default();
    let source = SourceFile::new(
        SourceId::new(1),
        "signaling_pathway.ant",
        source_text,
        &limits,
    )
    .expect("valid source");
    let parsed = parse(&source, &limits).expect("parse succeeded");
    let formatted = format_antimony(&parsed).expect("format succeeded");
    let model = analyze(&parsed).expect("semantic analysis succeeded");

    println!("Profile: {}", model.profile_id());
    println!("Model ID: {}", model.id());
    println!("Compartments: {}", model.compartments().len());
    println!("Species: {}", model.species().len());
    println!("Parameters: {}", model.parameters().len());
    println!("Reactions: {}", model.reactions().len());
    println!("Rules: {}", model.rules().len());
    println!("Events: {}", model.events().len());
    println!("\nCanonical Antimony:\n{formatted}");

    #[cfg(feature = "sbml")]
    {
        let sbml = antimony::write_sbml(&model).expect("SBML export succeeded");
        println!("Exported SBML bytes: {}", sbml.xml().len());
    }
}
