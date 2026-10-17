//! Steady-state solver ($\frac{d\mathbf{x}}{dt} = \mathbf{0}$) and Metabolic Control Analysis (MCA)
//! unscaled and scaled elasticity matrices ($\frac{\partial v_j}{\partial S_i}$ and $\varepsilon_{S_i}^{v_j}$).

use super::evaluator::{ModelRuntime, eval_num};
use super::integrator::{advance_stage, compute_derivatives, rk4_step};
use crate::model::ValidatedModel;
use crate::syntax::error;
use crate::{Diagnostic, DiagnosticCode};
use std::collections::BTreeMap;

/// Configuration for the steady-state solver (`model.steady_state(&config)`).
#[derive(Clone, Copy, Debug)]
pub struct SteadyStateConfig {
    /// Maximum absolute rate-of-change residual $\max_i |dx_i/dt|$ required for convergence (default `1e-9`).
    pub tolerance: f64,
    /// Maximum pseudo-transient relaxation horizon before Newton refinement (default `1000.0`).
    pub max_time: f64,
    /// Internal pseudo-transient step size (default `0.02`).
    pub dt: f64,
}

impl Default for SteadyStateConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-9,
            max_time: 1000.0,
            dt: 0.02,
        }
    }
}

impl SteadyStateConfig {
    /// Construct a steady-state solver configuration with custom residual tolerance.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Override the maximum pseudo-transient relaxation time horizon.
    pub fn with_max_time(mut self, max_time: f64) -> Self {
        self.max_time = max_time;
        self
    }
}

/// Steady-state solution and Metabolic Control Analysis (MCA) flux report.
#[derive(Clone, Debug)]
pub struct SteadyState {
    symbol_values: BTreeMap<String, f64>,
    floating_species: Vec<(String, f64)>,
    reaction_rates: Vec<(String, f64)>,
    max_rate_residual: f64,
    converged: bool,
}

impl SteadyState {
    /// Whether the maximum rate-of-change residual reached `config.tolerance`.
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// Maximum absolute derivative $\max_i |dx_i / dt|$ at the returned state.
    pub fn max_rate_residual(&self) -> f64 {
        self.max_rate_residual
    }

    /// Lookup the steady-state value of any species, compartment, or parameter by ID.
    pub fn value(&self, symbol: &str) -> Option<f64> {
        self.symbol_values.get(symbol).copied()
    }

    /// Steady-state concentrations (or amounts for `substanceOnly` species) of all floating species.
    pub fn floating_species(&self) -> &[(String, f64)] {
        &self.floating_species
    }

    /// Steady-state reaction fluxes $v_j$ in model reaction order.
    pub fn reaction_rates(&self) -> &[(String, f64)] {
        &self.reaction_rates
    }

    /// Lookup the steady-state flux of a specific reaction by ID.
    pub fn reaction_rate(&self, reaction_id: &str) -> Option<f64> {
        self.reaction_rates
            .iter()
            .find(|(id, _)| id == reaction_id)
            .map(|(_, v)| *v)
    }

    /// Compute the Metabolic Control Analysis (MCA) elasticity matrix at this steady state.
    pub fn elasticity_matrix(
        &self,
        model: &ValidatedModel,
    ) -> Result<ElasticityMatrix, Diagnostic> {
        ElasticityMatrix::at_state(model, Some(&self.symbol_values))
    }
}

/// Metabolic Control Analysis (MCA) unscaled ($\frac{\partial v_j}{\partial S_i}$) and scaled
/// ($\varepsilon_{S_i}^{v_j} = \frac{\partial v_j}{\partial S_i} \frac{S_i}{v_j}$) elasticity matrices.
///
/// Rows correspond to reactions $v_j$ in model order; columns correspond to floating species $S_i$ in model order.
#[derive(Clone, Debug, PartialEq)]
pub struct ElasticityMatrix {
    reactions: Vec<String>,
    floating_species: Vec<String>,
    unscaled: Vec<Vec<f64>>,
    scaled: Vec<Vec<f64>>,
}

impl ElasticityMatrix {
    /// Compute the unscaled and scaled elasticity matrices at the model's initial state (or at `state_overrides`).
    pub fn at_state(
        model: &ValidatedModel,
        state_overrides: Option<&BTreeMap<String, f64>>,
    ) -> Result<Self, Diagnostic> {
        let mut rt = ModelRuntime::initialize(model, 0.0)?;
        if let Some(overrides) = state_overrides {
            for (name, &val) in overrides {
                if let Some(&idx) = rt.sym_map.get(name) {
                    rt.vals[idx] = val;
                }
            }
            for &(target_idx, expr) in &rt.assign_rules {
                rt.vals[target_idx] = eval_num(
                    rt.model,
                    &rt.sym_map,
                    &rt.fn_map,
                    &rt.vals,
                    0.0,
                    expr,
                    &BTreeMap::new(),
                )?;
            }
        }

        let mut floating_indices = Vec::new();
        let mut floating_species = Vec::new();
        for (i, sp) in model.species().iter().enumerate() {
            if !sp.boundary_condition() {
                floating_indices.push(i);
                floating_species.push(sp.id().to_owned());
            }
        }

        let reactions: Vec<String> = model
            .reactions()
            .iter()
            .map(|r| r.id().to_owned())
            .collect();

        let n_rxn = reactions.len();
        let n_flt = floating_species.len();
        let mut unscaled = vec![vec![0.0_f64; n_flt]; n_rxn];
        let mut scaled = vec![vec![0.0_f64; n_flt]; n_rxn];

        let mut base_fluxes = Vec::with_capacity(n_rxn);
        for rxn in model.reactions() {
            base_fluxes.push(eval_num(
                rt.model,
                &rt.sym_map,
                &rt.fn_map,
                &rt.vals,
                0.0,
                rxn.rate(),
                &BTreeMap::new(),
            )?);
        }

        for (col_idx, &sp_idx) in floating_indices.iter().enumerate() {
            let s0 = rt.vals[sp_idx];
            let h = 1e-7 * s0.abs().max(1.0);

            let eval_perturbed = |s_val: f64| -> Result<Vec<f64>, Diagnostic> {
                let mut v_pert = rt.vals.clone();
                v_pert[sp_idx] = s_val;
                for &(target_idx, expr) in &rt.assign_rules {
                    if target_idx != sp_idx {
                        v_pert[target_idx] = eval_num(
                            rt.model,
                            &rt.sym_map,
                            &rt.fn_map,
                            &v_pert,
                            0.0,
                            expr,
                            &BTreeMap::new(),
                        )?;
                    }
                }
                let mut fluxes = Vec::with_capacity(n_rxn);
                for rxn in model.reactions() {
                    fluxes.push(eval_num(
                        rt.model,
                        &rt.sym_map,
                        &rt.fn_map,
                        &v_pert,
                        0.0,
                        rxn.rate(),
                        &BTreeMap::new(),
                    )?);
                }
                Ok(fluxes)
            };

            let (derivs, step_used) = if s0 >= h {
                let f_plus = eval_perturbed(s0 + h)?;
                let f_minus = eval_perturbed(s0 - h)?;
                (
                    f_plus
                        .into_iter()
                        .zip(f_minus)
                        .map(|(fp, fm)| fp - fm)
                        .collect::<Vec<_>>(),
                    2.0 * h,
                )
            } else {
                let f_plus = eval_perturbed(s0 + h)?;
                (
                    f_plus
                        .into_iter()
                        .zip(base_fluxes.iter().copied())
                        .map(|(fp, f0)| fp - f0)
                        .collect::<Vec<_>>(),
                    h,
                )
            };

            for row_idx in 0..n_rxn {
                let dv_ds = derivs[row_idx] / step_used;
                unscaled[row_idx][col_idx] = dv_ds;
                let v0 = base_fluxes[row_idx];
                scaled[row_idx][col_idx] = if v0.abs() > 1e-15 {
                    dv_ds * (s0 / v0)
                } else {
                    0.0
                };
            }
        }

        Ok(Self {
            reactions,
            floating_species,
            unscaled,
            scaled,
        })
    }

    /// Reaction identifiers corresponding to matrix rows.
    pub fn reactions(&self) -> &[String] {
        &self.reactions
    }

    /// Floating species identifiers corresponding to matrix columns.
    pub fn floating_species(&self) -> &[String] {
        &self.floating_species
    }

    /// Borrow all rows of the unscaled elasticity matrix $\frac{\partial v_j}{\partial S_i}$.
    pub fn unscaled_rows(&self) -> &[Vec<f64>] {
        &self.unscaled
    }

    /// Borrow all rows of the scaled elasticity matrix $\varepsilon_{S_i}^{v_j} = \frac{\partial v_j}{\partial S_i} \frac{S_i}{v_j}$.
    pub fn scaled_rows(&self) -> &[Vec<f64>] {
        &self.scaled
    }

    /// Lookup the unscaled elasticity $\frac{\partial v_j}{\partial S_i}$ for a named reaction and floating species.
    pub fn unscaled(&self, reaction_id: &str, species_id: &str) -> Option<f64> {
        let row = self.reactions.iter().position(|r| r == reaction_id)?;
        let col = self.floating_species.iter().position(|s| s == species_id)?;
        Some(self.unscaled[row][col])
    }

    /// Lookup the scaled elasticity $\varepsilon_{S_i}^{v_j}$ for a named reaction and floating species.
    pub fn scaled(&self, reaction_id: &str, species_id: &str) -> Option<f64> {
        let row = self.reactions.iter().position(|r| r == reaction_id)?;
        let col = self.floating_species.iter().position(|s| s == species_id)?;
        Some(self.scaled[row][col])
    }
}

impl ValidatedModel {
    /// Solve for the model's steady state ($\frac{d\mathbf{x}}{dt} = \mathbf{0}$) while preserving moiety conservation laws.
    pub fn steady_state(&self, config: &SteadyStateConfig) -> Result<SteadyState, Diagnostic> {
        steady_state(self, config)
    }

    /// Compute the Metabolic Control Analysis (MCA) unscaled and scaled elasticity matrices at the initial state.
    pub fn elasticity_matrix(&self) -> Result<ElasticityMatrix, Diagnostic> {
        ElasticityMatrix::at_state(self, None)
    }
}

/// Compute the steady state ($\frac{d\mathbf{x}}{dt} = \mathbf{0}$) of a complete [`ValidatedModel`],
/// preserving linear moiety conservation laws via pseudo-transient RK4 continuation followed by
/// conserved-subspace Newton-Raphson polishing.
pub fn steady_state(
    model: &ValidatedModel,
    config: &SteadyStateConfig,
) -> Result<SteadyState, Diagnostic> {
    let origin = model.source().span(0, 0).expect("origin span");
    if !config.tolerance.is_finite()
        || config.tolerance <= 0.0
        || !config.max_time.is_finite()
        || config.max_time <= 0.0
        || !config.dt.is_finite()
        || config.dt <= 0.0
    {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "invalid steady-state configuration: require positive finite tolerance, max_time, and dt",
        ));
    }

    let mut rt = ModelRuntime::initialize(model, 0.0)?;
    let mut t = 0.0_f64;
    let max_steps = ((config.max_time / config.dt).ceil() as usize).clamp(100, 100_000);

    let max_residual =
        |rt_ref: &ModelRuntime<'_>, vals: &[f64], t_now: f64| -> Result<f64, Diagnostic> {
            let (d_vals, d_amts) = compute_derivatives(rt_ref, vals, t_now)?;
            let mut max_r = 0.0_f64;
            for &(target_idx, _) in &rt_ref.rate_rules {
                max_r = max_r.max(d_vals[target_idx].abs());
            }
            for sp_i in 0..rt_ref.n_spec {
                if rt_ref.rxn_governed_species[sp_i] {
                    let rate = if rt_ref.sp_subst_only[sp_i] {
                        d_amts[sp_i]
                    } else {
                        d_amts[sp_i] / vals[rt_ref.sp_comp_sym[sp_i]]
                    };
                    max_r = max_r.max(rate.abs());
                }
            }
            Ok(max_r)
        };

    let mut res = max_residual(&rt, &rt.vals, t)?;
    for _ in 0..max_steps {
        if res <= config.tolerance {
            break;
        }
        let (v_next, a_next) = rk4_step(&rt, &rt.vals, &rt.sp_amounts, t, config.dt)?;
        rt.vals = v_next;
        rt.sp_amounts = a_next;
        t += config.dt;
        res = max_residual(&rt, &rt.vals, t)?;

        if res <= config.tolerance * 1e4 {
            // Attempt a damped Newton-Raphson step along the reaction-flux / rate-rule manifold
            if let Ok((v_pol, a_pol)) =
                damped_newton_polish(&rt, &rt.vals, &rt.sp_amounts, t, config.tolerance)
            {
                let pol_res = max_residual(&rt, &v_pol, t)?;
                if pol_res < res {
                    rt.vals = v_pol;
                    rt.sp_amounts = a_pol;
                    res = pol_res;
                }
            }
        }
    }

    let mut symbol_values = BTreeMap::new();
    for (idx, name) in rt.sym_names.iter().enumerate() {
        symbol_values.insert(name.clone(), rt.vals[idx]);
    }

    let mut floating_species = Vec::new();
    for (i, sp) in model.species().iter().enumerate() {
        if !sp.boundary_condition() {
            floating_species.push((sp.id().to_owned(), rt.vals[i]));
        }
    }

    let mut reaction_rates = Vec::with_capacity(model.reactions().len());
    for rxn in model.reactions() {
        let flux = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            &rt.vals,
            t,
            rxn.rate(),
            &BTreeMap::new(),
        )?;
        reaction_rates.push((rxn.id().to_owned(), flux));
    }

    Ok(SteadyState {
        symbol_values,
        floating_species,
        reaction_rates,
        max_rate_residual: res,
        converged: res <= config.tolerance,
    })
}

/// Perform damped Newton-Raphson refinement in the active ODE state variables.
fn damped_newton_polish(
    rt: &ModelRuntime<'_>,
    vals0: &[f64],
    amts0: &[f64],
    t: f64,
    tol: f64,
) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let mut active_vars = Vec::<usize>::new();
    for i in 0..rt.n_spec {
        if rt.rxn_governed_species[i] {
            active_vars.push(i);
        }
    }
    for &(target_idx, _) in &rt.rate_rules {
        if !active_vars.contains(&target_idx) {
            active_vars.push(target_idx);
        }
    }
    let n = active_vars.len();
    if n == 0 || n > 32 {
        return Ok((vals0.to_vec(), amts0.to_vec()));
    }

    let mut vals = vals0.to_vec();
    let mut amts = amts0.to_vec();

    let eval_f = |v: &[f64]| -> Result<Vec<f64>, Diagnostic> {
        let (d_vals, d_amts) = compute_derivatives(rt, v, t)?;
        let mut f = Vec::with_capacity(n);
        for &idx in &active_vars {
            if idx < rt.n_spec && rt.rxn_governed_species[idx] {
                f.push(d_amts[idx]);
            } else {
                f.push(d_vals[idx]);
            }
        }
        Ok(f)
    };

    for _iter in 0..12 {
        let f0 = eval_f(&vals)?;
        let norm0 = f0.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        if norm0 <= tol {
            break;
        }

        // Finite-difference Jacobian J_{i,j} = df_i / du_j (with tiny Tikhonov regularization for conserved moieties)
        let mut jac = vec![vec![0.0_f64; n]; n];
        let zero_vec = vec![0.0_f64; vals.len()];
        let zero_amts = vec![0.0_f64; amts.len()];
        for (j, &var_idx) in active_vars.iter().enumerate() {
            let base_u = if var_idx < rt.n_spec && rt.rxn_governed_species[var_idx] {
                amts[var_idx]
            } else {
                vals[var_idx]
            };
            let eps = 1e-7 * base_u.abs().max(1.0);
            let mut d_v = zero_vec.clone();
            let mut d_a = zero_amts.clone();
            if var_idx < rt.n_spec && rt.rxn_governed_species[var_idx] {
                d_a[var_idx] = 1.0;
            } else {
                d_v[var_idx] = 1.0;
            }
            let (v_pert, _) = advance_stage(rt, &vals, &amts, &d_v, &d_a, t, eps)?;
            let f_pert = eval_f(&v_pert)?;
            for i in 0..n {
                jac[i][j] = (f_pert[i] - f0[i]) / eps;
            }
            jac[j][j] -= 1e-9;
        }

        let rhs: Vec<f64> = f0.iter().map(|&x| -x).collect();
        let Some(delta) = solve_linear_system(jac, rhs) else {
            break;
        };

        let mut d_v = vec![0.0_f64; vals.len()];
        let mut d_a = vec![0.0_f64; amts.len()];
        for (j, &var_idx) in active_vars.iter().enumerate() {
            if var_idx < rt.n_spec && rt.rxn_governed_species[var_idx] {
                d_a[var_idx] = delta[j];
            } else {
                d_v[var_idx] = delta[j];
            }
        }

        let mut accepted = false;
        let mut alpha = 1.0_f64;
        for _ls in 0..8 {
            let (v_cand, a_cand) = advance_stage(rt, &vals, &amts, &d_v, &d_a, t, alpha)?;
            if a_cand.iter().all(|&a| a >= -1e-12) && v_cand.iter().all(|&v| v.is_finite()) {
                let f_cand = eval_f(&v_cand)?;
                let norm_cand = f_cand.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
                if norm_cand < norm0 {
                    vals = v_cand;
                    amts = a_cand;
                    accepted = true;
                    break;
                }
            }
            alpha *= 0.5;
        }
        if !accepted {
            break;
        }
    }

    Ok((vals, amts))
}

fn solve_linear_system(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        let mut pivot_row = i;
        let mut max_val = a[i][i].abs();
        for (r, row) in a.iter().enumerate().skip(i + 1) {
            if row[i].abs() > max_val {
                max_val = row[i].abs();
                pivot_row = r;
            }
        }
        if max_val < 1e-15 {
            return None;
        }
        a.swap(i, pivot_row);
        b.swap(i, pivot_row);

        let piv = a[i][i];
        let (top, bottom) = a.split_at_mut(i + 1);
        let piv_row = &top[i][i..n];
        for (r_offset, row) in bottom.iter_mut().enumerate() {
            let r = i + 1 + r_offset;
            let factor = row[i] / piv;
            if factor == 0.0 {
                continue;
            }
            for (dst, &src) in row[i..n].iter_mut().zip(piv_row.iter()) {
                *dst -= factor * src;
            }
            b[r] -= factor * b[i];
        }
    }

    let mut x = vec![0.0_f64; n];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for (c, &x_c) in x.iter().enumerate().skip(i + 1) {
            sum -= a[i][c] * x_c;
        }
        x[i] = sum / a[i][i];
    }
    Some(x)
}
