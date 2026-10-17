//! Pure-Rust ODE and discrete-event trajectory CSV exporter for Antimony models.
//!
//! Usage:
//!   cargo run --example ode_trajectory_csv -- INPUT.ant [T_END] [STEPS] [OUTPUT.csv]
use antimony::{Diagnostic, Limits, SimulationConfig, load_file};
use std::{
    env, fs,
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

fn diagnostic(stage: &str, d: Diagnostic) -> ExitCode {
    eprintln!(
        "stage={stage} code={} message={}",
        d.code().as_str(),
        d.message()
    );
    for label in d.labels() {
        eprintln!(
            "source={} bytes={}..{} {}",
            label.span().source_id().get(),
            label.span().start(),
            label.span().end(),
            label.message()
        );
    }
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if !(1..=4).contains(&args.len()) {
        eprintln!(
            "stage=input code=usage message=usage: ode_trajectory_csv INPUT.ant [T_END] [STEPS] [OUTPUT.csv]"
        );
        return ExitCode::from(1);
    }
    let t_end: f64 = match args.get(1) {
        Some(s) => match s.parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("stage=input code=usage message=invalid T_END {s:?}");
                return ExitCode::from(1);
            }
        },
        None => 10.0,
    };
    let steps: usize = match args.get(2) {
        Some(s) => match s.parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("stage=input code=usage message=invalid STEPS {s:?}");
                return ExitCode::from(1);
            }
        },
        None => 50,
    };

    let limits = Limits::default();
    let model = match load_file(Path::new(&args[0]), &limits) {
        Ok(m) => m,
        Err(d) => return diagnostic("load", d),
    };
    let stoich = model.stoichiometry_matrix();
    eprintln!(
        "model={} floating_species={} boundary_species={} reactions={} events={}",
        model.id(),
        stoich.num_rows(),
        stoich.boundary_species().len(),
        stoich.num_cols(),
        model.events().len()
    );

    let config = SimulationConfig::new(0.0, t_end, steps);
    let traj = match model.simulate(&config) {
        Ok(t) => t,
        Err(d) => return diagnostic("simulate", d),
    };
    let csv = traj.to_csv();
    let result = if let Some(out_path) = args.get(3) {
        fs::write(out_path, &csv)
    } else {
        io::stdout().lock().write_all(csv.as_bytes())
    };
    if let Err(e) = result {
        eprintln!("stage=output code=io message={e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
