//! Pure-Rust stoichiometry matrix extraction, ODE + discrete-event integration,
//! steady-state solving, and Metabolic Control Analysis (MCA) elasticity matrices.
//!
//! This module has zero external C/C++ or Python dependencies and is available across
//! all Cargo feature configurations (including `--no-default-features`).

mod evaluator;
mod events;
mod integrator;
mod steady_state;
mod stoichiometry;

pub use integrator::{SimulationConfig, Trajectory, simulate};
pub use steady_state::{ElasticityMatrix, SteadyState, SteadyStateConfig, steady_state};
pub use stoichiometry::StoichiometryMatrix;
