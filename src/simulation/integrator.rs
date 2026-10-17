//! 4th-order Runge-Kutta (RK4) ODE and bisection discrete-event time-course integrator.

use super::evaluator::{ModelRuntime, eval_num};
use super::events::{PendingDelayedEvent, any_trigger_crossing, process_events_at_current_time};
use crate::model::ValidatedModel;
use crate::syntax::error;
use crate::{Diagnostic, DiagnosticCode};
use std::collections::BTreeMap;

/// Configuration for pure-Rust ODE + discrete-event time-course simulation.
#[derive(Clone, Copy, Debug)]
pub struct SimulationConfig {
    /// Initial simulation time $t_0$ (default `0.0`).
    pub t_start: f64,
    /// Final simulation time $t_{\text{end}}$ (default `10.0`).
    pub t_end: f64,
    /// Number of uniform output intervals (`steps + 1` output rows; default `50`).
    pub steps: usize,
    /// Maximum internal RK4 integration sub-step size (default `1e-3`).
    pub max_dt: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            t_start: 0.0,
            t_end: 10.0,
            steps: 50,
            max_dt: 1e-3,
        }
    }
}

impl SimulationConfig {
    /// Create a simulation configuration for $[t_{\text{start}}, t_{\text{end}}]$ with `steps` output intervals.
    pub fn new(t_start: f64, t_end: f64, steps: usize) -> Self {
        Self {
            t_start,
            t_end,
            steps,
            max_dt: 1e-3,
        }
    }

    /// Override the maximum internal RK4 sub-step size.
    pub fn with_max_dt(mut self, max_dt: f64) -> Self {
        self.max_dt = max_dt;
        self
    }
}

/// Time-course trajectory produced by [`simulate`].
#[derive(Clone, Debug)]
pub struct Trajectory {
    columns: Vec<String>,
    species_ids: Vec<String>,
    compartment_ids: Vec<String>,
    parameter_ids: Vec<String>,
    times: Vec<f64>,
    rows: Vec<Vec<f64>>,
    col_index: BTreeMap<String, usize>,
}

impl Trajectory {
    /// Column headers (`["time", ...species..., ...compartments..., ...parameters...]`).
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Species column names in model order.
    pub fn species_ids(&self) -> &[String] {
        &self.species_ids
    }

    /// Compartment column names in model order.
    pub fn compartment_ids(&self) -> &[String] {
        &self.compartment_ids
    }

    /// Parameter column names in model order.
    pub fn parameter_ids(&self) -> &[String] {
        &self.parameter_ids
    }

    /// Sampled output time points ($t_0, \dots, t_{\text{end}}$).
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Output rows, each matching [`Self::columns`].
    pub fn rows(&self) -> &[Vec<f64>] {
        &self.rows
    }

    /// Extract the full time-series vector for `"time"` or any species, compartment, or parameter ID.
    pub fn series(&self, symbol: &str) -> Option<Vec<f64>> {
        let &col = self.col_index.get(symbol)?;
        Some(self.rows.iter().map(|r| r[col]).collect())
    }

    /// Lookup the value of `symbol` at output row `step_index`.
    pub fn value_at(&self, step_index: usize, symbol: &str) -> Option<f64> {
        let &col = self.col_index.get(symbol)?;
        self.rows.get(step_index).map(|r| r[col])
    }

    /// Format the trajectory as a deterministic CSV table.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.columns.join(","));
        out.push('\n');
        for row in &self.rows {
            let formatted: Vec<String> = row.iter().map(|v| format!("{v:.10}")).collect();
            out.push_str(&formatted.join(","));
            out.push('\n');
        }
        out
    }
}

impl ValidatedModel {
    /// Run a pure-Rust ODE and discrete-event simulation on this model.
    pub fn simulate(&self, config: &SimulationConfig) -> Result<Trajectory, Diagnostic> {
        simulate(self, config)
    }
}

/// Simulate a complete [`ValidatedModel`] over $[t_{\text{start}}, t_{\text{end}}]$ using a
/// pure-Rust RK4 integrator with bisection root-finding for discrete `at` events and delays.
pub fn simulate(
    model: &ValidatedModel,
    config: &SimulationConfig,
) -> Result<Trajectory, Diagnostic> {
    let origin = model.source().span(0, 0).expect("origin span");
    if !config.t_start.is_finite()
        || !config.t_end.is_finite()
        || config.t_end < config.t_start
        || config.steps == 0
        || !config.max_dt.is_finite()
        || config.max_dt <= 0.0
    {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "invalid simulation configuration: require finite t_start <= t_end, steps >= 1, and max_dt > 0",
        ));
    }

    let mut rt = ModelRuntime::initialize(model, config.t_start)?;
    let mut t = config.t_start;

    let n_events = model.events().len();
    let mut prev_trig: Vec<bool> = model
        .events()
        .iter()
        .map(|ev| ev.trigger().initial_value())
        .collect();
    let mut pending_delayed = Vec::<PendingDelayedEvent>::new();
    let mut event_seq = 0usize;

    process_events_at_current_time(
        &mut rt,
        &mut prev_trig,
        &mut pending_delayed,
        &mut event_seq,
        t,
    )?;

    let species_ids: Vec<String> = model.species().iter().map(|s| s.id().to_owned()).collect();
    let compartment_ids: Vec<String> = model
        .compartments()
        .iter()
        .map(|c| c.id().to_owned())
        .collect();
    let parameter_ids: Vec<String> = model
        .parameters()
        .iter()
        .map(|p| p.id().to_owned())
        .collect();

    let mut columns = Vec::with_capacity(1 + rt.total_syms);
    columns.push("time".to_owned());
    columns.extend(species_ids.iter().cloned());
    columns.extend(compartment_ids.iter().cloned());
    columns.extend(parameter_ids.iter().cloned());

    let col_index: BTreeMap<String, usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.clone(), i))
        .collect();

    let mut times = Vec::with_capacity(config.steps + 1);
    let mut rows = Vec::with_capacity(config.steps + 1);

    let record_row = |t_now: f64, vals_now: &[f64]| -> Vec<f64> {
        let mut row = Vec::with_capacity(1 + vals_now.len());
        row.push(t_now);
        row.extend_from_slice(vals_now);
        row
    };

    times.push(t);
    rows.push(record_row(t, &rt.vals));

    let total_span = config.t_end - config.t_start;
    if total_span == 0.0 {
        return Ok(Trajectory {
            columns,
            species_ids,
            compartment_ids,
            parameter_ids,
            times,
            rows,
            col_index,
        });
    }

    for step_idx in 1..=config.steps {
        let target_t = if step_idx == config.steps {
            config.t_end
        } else {
            config.t_start + (total_span * (step_idx as f64)) / (config.steps as f64)
        };

        while t < target_t - 1e-14 {
            let next_delayed_t = pending_delayed
                .iter()
                .map(|p| p.exec_time)
                .fold(f64::INFINITY, f64::min);
            let step_limit = target_t.min(next_delayed_t);
            let raw_dt = (step_limit - t).min(config.max_dt);
            let dt = raw_dt.max(1e-12).min(step_limit - t);

            let vals_before = rt.vals.clone();
            let amounts_before = rt.sp_amounts.clone();
            let t_before = t;

            let (mut vals_after, mut amounts_after) =
                rk4_step(&rt, &vals_before, &amounts_before, t_before, dt)?;
            let mut t_after = t_before + dt;

            if n_events > 0
                && any_trigger_crossing(&rt, &prev_trig, &vals_after, t_after)?
                && dt > 1e-9
            {
                let mut lo = 0.0_f64;
                let mut hi = dt;
                for _ in 0..32 {
                    let mid = 0.5 * (lo + hi);
                    let (v_mid, _) = rk4_step(&rt, &vals_before, &amounts_before, t_before, mid)?;
                    let t_mid = t_before + mid;
                    if any_trigger_crossing(&rt, &prev_trig, &v_mid, t_mid)? {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let (v_hi, a_hi) = rk4_step(&rt, &vals_before, &amounts_before, t_before, hi)?;
                vals_after = v_hi;
                amounts_after = a_hi;
                t_after = t_before + hi;
            }

            rt.vals = vals_after;
            rt.sp_amounts = amounts_after;
            t = t_after;

            process_events_at_current_time(
                &mut rt,
                &mut prev_trig,
                &mut pending_delayed,
                &mut event_seq,
                t,
            )?;
        }

        times.push(target_t);
        rows.push(record_row(target_t, &rt.vals));
    }

    Ok(Trajectory {
        columns,
        species_ids,
        compartment_ids,
        parameter_ids,
        times,
        rows,
        col_index,
    })
}

pub(super) fn rk4_step(
    rt: &ModelRuntime<'_>,
    vals0: &[f64],
    amounts0: &[f64],
    t0: f64,
    dt: f64,
) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let (d_vals1, d_amts1) = compute_derivatives(rt, vals0, t0)?;

    let (vals2, _amts2) = advance_stage(
        rt,
        vals0,
        amounts0,
        &d_vals1,
        &d_amts1,
        t0 + 0.5 * dt,
        0.5 * dt,
    )?;
    let (d_vals2, d_amts2) = compute_derivatives(rt, &vals2, t0 + 0.5 * dt)?;

    let (vals3, _amts3) = advance_stage(
        rt,
        vals0,
        amounts0,
        &d_vals2,
        &d_amts2,
        t0 + 0.5 * dt,
        0.5 * dt,
    )?;
    let (d_vals3, d_amts3) = compute_derivatives(rt, &vals3, t0 + 0.5 * dt)?;

    let (vals4, _amts4) = advance_stage(rt, vals0, amounts0, &d_vals3, &d_amts3, t0 + dt, dt)?;
    let (d_vals4, d_amts4) = compute_derivatives(rt, &vals4, t0 + dt)?;

    let mut d_vals_comb = vec![0.0_f64; vals0.len()];
    for &(target_idx, _) in &rt.rate_rules {
        d_vals_comb[target_idx] = (d_vals1[target_idx]
            + 2.0 * d_vals2[target_idx]
            + 2.0 * d_vals3[target_idx]
            + d_vals4[target_idx])
            / 6.0;
    }
    let mut d_amts_comb = vec![0.0_f64; amounts0.len()];
    for i in 0..amounts0.len() {
        if rt.rxn_governed_species[i] {
            d_amts_comb[i] = (d_amts1[i] + 2.0 * d_amts2[i] + 2.0 * d_amts3[i] + d_amts4[i]) / 6.0;
        }
    }

    advance_stage(rt, vals0, amounts0, &d_vals_comb, &d_amts_comb, t0 + dt, dt)
}

pub(super) fn compute_derivatives(
    rt: &ModelRuntime<'_>,
    vals: &[f64],
    t: f64,
) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let mut d_vals = vec![0.0_f64; vals.len()];
    for &(target_idx, expr) in &rt.rate_rules {
        d_vals[target_idx] = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            vals,
            t,
            expr,
            &BTreeMap::new(),
        )?;
    }

    let mut d_amts = vec![0.0_f64; rt.n_spec];
    for (r_idx, rxn) in rt.model.reactions().iter().enumerate() {
        if rt.rxn_net_stoich[r_idx].is_empty() {
            continue;
        }
        let flux = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            vals,
            t,
            rxn.rate(),
            &BTreeMap::new(),
        )?;
        for &(sp_i, stoich) in &rt.rxn_net_stoich[r_idx] {
            d_amts[sp_i] += stoich * flux;
        }
    }

    Ok((d_vals, d_amts))
}

pub(super) fn advance_stage(
    rt: &ModelRuntime<'_>,
    vals0: &[f64],
    amounts0: &[f64],
    d_vals: &[f64],
    d_amts: &[f64],
    t_stage: f64,
    h: f64,
) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let mut vals = vals0.to_vec();
    let mut amts = amounts0.to_vec();
    let n_spec = rt.n_spec;

    for &(target_idx, _) in &rt.rate_rules {
        vals[target_idx] = vals0[target_idx] + h * d_vals[target_idx];
        if target_idx < n_spec {
            amts[target_idx] = if rt.sp_subst_only[target_idx] {
                vals[target_idx]
            } else {
                vals[target_idx] * vals[rt.sp_comp_sym[target_idx]]
            };
        }
    }

    for sp_i in 0..n_spec {
        if rt.rxn_governed_species[sp_i] {
            amts[sp_i] = amounts0[sp_i] + h * d_amts[sp_i];
        }
    }

    for &(target_idx, expr) in &rt.assign_rules {
        if target_idx >= n_spec && target_idx < n_spec + rt.n_comp {
            vals[target_idx] = eval_num(
                rt.model,
                &rt.sym_map,
                &rt.fn_map,
                &vals,
                t_stage,
                expr,
                &BTreeMap::new(),
            )?;
        }
    }

    for sp_i in 0..n_spec {
        if rt.rxn_governed_species[sp_i] {
            vals[sp_i] = if rt.sp_subst_only[sp_i] {
                amts[sp_i]
            } else {
                amts[sp_i] / vals[rt.sp_comp_sym[sp_i]]
            };
        }
    }

    for &(target_idx, expr) in &rt.assign_rules {
        vals[target_idx] = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            &vals,
            t_stage,
            expr,
            &BTreeMap::new(),
        )?;
        if target_idx < n_spec {
            amts[target_idx] = if rt.sp_subst_only[target_idx] {
                vals[target_idx]
            } else {
                vals[target_idx] * vals[rt.sp_comp_sym[target_idx]]
            };
        }
    }

    for sp_i in 0..n_spec {
        if rt.rxn_governed_species[sp_i] && !rt.sp_subst_only[sp_i] {
            vals[sp_i] = amts[sp_i] / vals[rt.sp_comp_sym[sp_i]];
        }
    }

    Ok((vals, amts))
}
