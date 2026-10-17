//! Discrete-event trigger evaluation, delayed-event queue, and assignment execution.

use super::evaluator::{ModelRuntime, eval_bool, eval_num};
use crate::Diagnostic;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct PendingDelayedEvent {
    pub event_idx: usize,
    pub exec_time: f64,
    pub trigger_time: f64,
    pub snapshot_rhs: Option<Vec<(usize, f64, Option<f64>)>>,
    pub seq: usize,
}

pub(super) fn any_trigger_crossing(
    rt: &ModelRuntime<'_>,
    prev_trig: &[bool],
    vals_after: &[f64],
    t_after: f64,
) -> Result<bool, Diagnostic> {
    for (ev_i, ev) in rt.model.events().iter().enumerate() {
        let cur_trig = eval_bool(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            vals_after,
            t_after,
            ev.trigger().expression(),
            &BTreeMap::new(),
        )?;
        if (!prev_trig[ev_i] && cur_trig)
            || (prev_trig[ev_i] && !cur_trig && !ev.trigger().persistent())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn process_events_at_current_time(
    rt: &mut ModelRuntime<'_>,
    prev_trig: &mut [bool],
    pending_delayed: &mut Vec<PendingDelayedEvent>,
    event_seq: &mut usize,
    t: f64,
) -> Result<(), Diagnostic> {
    if rt.model.events().is_empty() {
        return Ok(());
    }
    let n_spec = rt.n_spec;

    for _cascade in 0..64 {
        let mut state_changed = false;

        // 1. Cancel any non-persistent pending delayed events whose trigger is now false.
        let mut cur_trigs = Vec::with_capacity(rt.model.events().len());
        for ev in rt.model.events() {
            cur_trigs.push(eval_bool(
                rt.model,
                &rt.sym_map,
                &rt.fn_map,
                &rt.vals,
                t,
                ev.trigger().expression(),
                &BTreeMap::new(),
            )?);
        }
        pending_delayed.retain(|p| {
            let ev = &rt.model.events()[p.event_idx];
            ev.trigger().persistent() || cur_trigs[p.event_idx]
        });

        // 2. Check for newly rising-edge (`false -> true`) triggers at time `t`.
        let mut immediate_candidates = Vec::<(usize, f64, usize)>::new();
        for (ev_i, ev) in rt.model.events().iter().enumerate() {
            let cur = cur_trigs[ev_i];
            if !prev_trig[ev_i] && cur {
                prev_trig[ev_i] = true;
                let prio = match ev.priority() {
                    Some(p_expr) => eval_num(
                        rt.model,
                        &rt.sym_map,
                        &rt.fn_map,
                        &rt.vals,
                        t,
                        p_expr,
                        &BTreeMap::new(),
                    )?,
                    None => 0.0,
                };
                if let Some(d_expr) = ev.delay() {
                    let d_val = eval_num(
                        rt.model,
                        &rt.sym_map,
                        &rt.fn_map,
                        &rt.vals,
                        t,
                        d_expr,
                        &BTreeMap::new(),
                    )?;
                    if d_val > 0.0 {
                        let snapshot_rhs = if ev.use_values_from_trigger_time() {
                            Some(evaluate_event_rhs(rt, t, ev_i)?)
                        } else {
                            None
                        };
                        *event_seq += 1;
                        pending_delayed.push(PendingDelayedEvent {
                            event_idx: ev_i,
                            exec_time: t + d_val,
                            trigger_time: t,
                            snapshot_rhs,
                            seq: *event_seq,
                        });
                    } else {
                        *event_seq += 1;
                        immediate_candidates.push((ev_i, prio, *event_seq));
                    }
                } else {
                    *event_seq += 1;
                    immediate_candidates.push((ev_i, prio, *event_seq));
                }
            } else {
                prev_trig[ev_i] = cur;
            }
        }

        // 3. Collect any due delayed events (`exec_time <= t + 1e-12`).
        let mut due_delayed = Vec::<(PendingDelayedEvent, f64)>::new();
        let mut remaining_delayed = Vec::new();
        for p in pending_delayed.drain(..) {
            if p.exec_time <= t + 1e-12 {
                let ev = &rt.model.events()[p.event_idx];
                let prio = match ev.priority() {
                    Some(p_expr) => eval_num(
                        rt.model,
                        &rt.sym_map,
                        &rt.fn_map,
                        &rt.vals,
                        t,
                        p_expr,
                        &BTreeMap::new(),
                    )?,
                    None => 0.0,
                };
                due_delayed.push((p, prio));
            } else {
                remaining_delayed.push(p);
            }
        }
        *pending_delayed = remaining_delayed;

        if immediate_candidates.is_empty() && due_delayed.is_empty() {
            break;
        }

        immediate_candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
        });
        let mut pre_snapshots = BTreeMap::<usize, Vec<(usize, f64, Option<f64>)>>::new();
        for &(ev_i, _, _) in &immediate_candidates {
            if rt.model.events()[ev_i].use_values_from_trigger_time() {
                pre_snapshots.insert(ev_i, evaluate_event_rhs(rt, t, ev_i)?);
            }
        }

        for (ev_i, _, _) in immediate_candidates {
            let rhs_vals = match pre_snapshots.remove(&ev_i) {
                Some(snap) => snap,
                None => evaluate_event_rhs(rt, t, ev_i)?,
            };
            apply_event_assignments(rt, n_spec, t, &rhs_vals)?;
            state_changed = true;
        }

        due_delayed.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.seq.cmp(&b.0.seq))
        });
        for (p, _) in due_delayed {
            let rhs_vals = match p.snapshot_rhs {
                Some(snap) => snap,
                None => evaluate_event_rhs(rt, p.trigger_time.max(t), p.event_idx)?,
            };
            apply_event_assignments(rt, n_spec, t, &rhs_vals)?;
            state_changed = true;
        }

        if !state_changed {
            break;
        }
    }
    Ok(())
}

fn evaluate_event_rhs(
    rt: &ModelRuntime<'_>,
    t: f64,
    ev_idx: usize,
) -> Result<Vec<(usize, f64, Option<f64>)>, Diagnostic> {
    let n_spec = rt.n_spec;
    let ev = &rt.model.events()[ev_idx];
    let mut out = Vec::with_capacity(ev.assignments().len());
    for ea in ev.assignments() {
        let target_idx = rt.sym_map[ea.variable()];
        let val = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            &rt.vals,
            t,
            ea.expression(),
            &BTreeMap::new(),
        )?;
        let comp_vol_at_eval = if target_idx < n_spec && !rt.sp_subst_only[target_idx] {
            Some(rt.vals[rt.sp_comp_sym[target_idx]])
        } else {
            None
        };
        out.push((target_idx, val, comp_vol_at_eval));
    }
    Ok(out)
}

fn apply_event_assignments(
    rt: &mut ModelRuntime<'_>,
    n_spec: usize,
    t: f64,
    rhs_vals: &[(usize, f64, Option<f64>)],
) -> Result<(), Diagnostic> {
    let mut compartment_resized = false;
    for &(target_idx, val, comp_vol_at_eval) in rhs_vals {
        rt.vals[target_idx] = val;
        if target_idx < n_spec {
            rt.sp_amounts[target_idx] = match comp_vol_at_eval {
                Some(v_c) => val * v_c,
                None => val,
            };
        } else if target_idx < n_spec + rt.n_comp {
            compartment_resized = true;
        }
    }
    if compartment_resized {
        for sp_i in 0..n_spec {
            if !rt.sp_subst_only[sp_i] {
                let v_c = rt.vals[rt.sp_comp_sym[sp_i]];
                rt.vals[sp_i] = rt.sp_amounts[sp_i] / v_c;
            }
        }
    }
    for &(target_idx, expr) in &rt.assign_rules {
        rt.vals[target_idx] = eval_num(
            rt.model,
            &rt.sym_map,
            &rt.fn_map,
            &rt.vals,
            t,
            expr,
            &BTreeMap::new(),
        )?;
        if target_idx < n_spec {
            rt.sp_amounts[target_idx] = if rt.sp_subst_only[target_idx] {
                rt.vals[target_idx]
            } else {
                rt.vals[target_idx] * rt.vals[rt.sp_comp_sym[target_idx]]
            };
        }
    }
    Ok(())
}
