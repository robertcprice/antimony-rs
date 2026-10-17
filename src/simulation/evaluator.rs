//! Runtime expression evaluation, topological equation sorting, and initial state setup.

use crate::builtins::{Builtin, NumericConstant};
use crate::conditional::{ComparisonOp, LogicalOp};
use crate::model::{ExpressionId, ExpressionKind, RuleKind, SymbolKind, ValidatedModel};
use crate::syntax::{BinaryOp, UnaryOp, error};
use crate::{Diagnostic, DiagnosticCode};
use std::collections::{BTreeMap, BTreeSet};

/// Pre-indexed runtime metadata and initial state for a [`ValidatedModel`].
pub(super) struct ModelRuntime<'a> {
    pub model: &'a ValidatedModel,
    pub n_spec: usize,
    pub n_comp: usize,
    pub total_syms: usize,
    pub sym_map: BTreeMap<String, usize>,
    pub sym_names: Vec<String>,
    pub sp_comp_sym: Vec<usize>,
    pub sp_subst_only: Vec<bool>,
    pub fn_map: BTreeMap<&'a str, &'a crate::FunctionDefinition>,
    pub assign_rules: Vec<(usize, ExpressionId)>,
    pub rate_rules: Vec<(usize, ExpressionId)>,
    pub rxn_governed_species: Vec<bool>,
    pub rxn_net_stoich: Vec<Vec<(usize, f64)>>,
    pub vals: Vec<f64>,
    pub sp_amounts: Vec<f64>,
}

impl<'a> ModelRuntime<'a> {
    /// Initialize runtime symbol tables and evaluate initial assignments + assignment rules at `t0`.
    pub fn initialize(model: &'a ValidatedModel, t0: f64) -> Result<Self, Diagnostic> {
        let origin = model.source().span(0, 0).expect("origin span");
        let n_spec = model.species().len();
        let n_comp = model.compartments().len();
        let n_param = model.parameters().len();
        let total_syms = n_spec + n_comp + n_param;

        let mut sym_map = BTreeMap::<String, usize>::new();
        let mut sym_names = Vec::with_capacity(total_syms);
        for (i, sp) in model.species().iter().enumerate() {
            sym_map.insert(sp.id().to_owned(), i);
            sym_names.push(sp.id().to_owned());
        }
        for (i, c) in model.compartments().iter().enumerate() {
            sym_map.insert(c.id().to_owned(), n_spec + i);
            sym_names.push(c.id().to_owned());
        }
        for (i, p) in model.parameters().iter().enumerate() {
            sym_map.insert(p.id().to_owned(), n_spec + n_comp + i);
            sym_names.push(p.id().to_owned());
        }

        let mut sp_comp_sym = Vec::with_capacity(n_spec);
        let mut sp_subst_only = Vec::with_capacity(n_spec);
        let mut sp_boundary = Vec::with_capacity(n_spec);
        for sp in model.species() {
            let c_idx = *sym_map
                .get(sp.compartment())
                .expect("validated species compartment exists");
            sp_comp_sym.push(c_idx);
            sp_subst_only.push(sp.has_only_substance_units());
            sp_boundary.push(sp.boundary_condition());
        }

        let fn_map: BTreeMap<&str, &crate::FunctionDefinition> = model
            .function_definitions()
            .iter()
            .map(|f| (f.id(), f))
            .collect();

        let mut assign_rules_raw = Vec::<(usize, ExpressionId)>::new();
        let mut rate_rules = Vec::<(usize, ExpressionId)>::new();
        let mut is_rule_target = vec![false; total_syms];
        let mut is_assign_rule_target = vec![false; total_syms];

        for r in model.rules() {
            let target_idx = *sym_map.get(r.target()).expect("validated rule target");
            is_rule_target[target_idx] = true;
            match r.kind() {
                RuleKind::Assignment => {
                    is_assign_rule_target[target_idx] = true;
                    assign_rules_raw.push((target_idx, r.expression()));
                }
                RuleKind::Rate => {
                    rate_rules.push((target_idx, r.expression()));
                }
            }
        }

        let assign_rules = topo_sort_equations(model, &sym_map, &assign_rules_raw);

        let mut vals = vec![0.0_f64; total_syms];
        let mut has_init = vec![false; total_syms];

        for (i, sp) in model.species().iter().enumerate() {
            if let Some(v) = sp.initial_concentration().or(sp.initial_amount()) {
                vals[i] = v;
                has_init[i] = true;
            }
        }
        for (i, c) in model.compartments().iter().enumerate() {
            if let Some(v) = c.size() {
                vals[n_spec + i] = v;
                has_init[n_spec + i] = true;
            }
        }
        for (i, p) in model.parameters().iter().enumerate() {
            if let Some(v) = p.value() {
                vals[n_spec + n_comp + i] = v;
                has_init[n_spec + n_comp + i] = true;
            }
        }

        let mut init_eqs_raw = Vec::<(usize, ExpressionId)>::new();
        for ia in model.initial_assignments() {
            let target_idx = *sym_map.get(ia.symbol()).expect("validated IA target");
            if !is_assign_rule_target[target_idx] {
                init_eqs_raw.push((target_idx, ia.expression()));
                has_init[target_idx] = true;
            }
        }
        for &(target_idx, expr) in &assign_rules {
            init_eqs_raw.push((target_idx, expr));
            has_init[target_idx] = true;
        }

        for (idx, &ok) in has_init.iter().enumerate() {
            if !ok {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!(
                        "cannot simulate incomplete model: symbol {:?} has no initial value",
                        sym_names[idx]
                    ),
                ));
            }
        }

        let init_eqs = topo_sort_equations(model, &sym_map, &init_eqs_raw);
        for &(target_idx, expr) in &init_eqs {
            vals[target_idx] =
                eval_num(model, &sym_map, &fn_map, &vals, t0, expr, &BTreeMap::new())?;
        }

        let mut rxn_governed_species = vec![false; n_spec];
        let mut sp_amounts = vec![0.0_f64; n_spec];
        for sp_i in 0..n_spec {
            if !sp_boundary[sp_i] && !is_rule_target[sp_i] {
                rxn_governed_species[sp_i] = true;
            }
            let comp_v = vals[sp_comp_sym[sp_i]];
            sp_amounts[sp_i] = if sp_subst_only[sp_i] {
                vals[sp_i]
            } else {
                vals[sp_i] * comp_v
            };
        }

        let n_rxn = model.reactions().len();
        let mut rxn_net_stoich: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_rxn];
        for (r_idx, rxn) in model.reactions().iter().enumerate() {
            let mut net = BTreeMap::<usize, f64>::new();
            for react in rxn.reactants() {
                let sp_i = sym_map[react.species()];
                if rxn_governed_species[sp_i] {
                    *net.entry(sp_i).or_insert(0.0) -= react.stoichiometry();
                }
            }
            for prod in rxn.products() {
                let sp_i = sym_map[prod.species()];
                if rxn_governed_species[sp_i] {
                    *net.entry(sp_i).or_insert(0.0) += prod.stoichiometry();
                }
            }
            rxn_net_stoich[r_idx] = net.into_iter().filter(|&(_, s)| s != 0.0).collect();
        }

        Ok(Self {
            model,
            n_spec,
            n_comp,
            total_syms,
            sym_map,
            sym_names,
            sp_comp_sym,
            sp_subst_only,
            fn_map,
            assign_rules,
            rate_rules,
            rxn_governed_species,
            rxn_net_stoich,
            vals,
            sp_amounts,
        })
    }
}

pub(super) fn topo_sort_equations(
    model: &ValidatedModel,
    sym_map: &BTreeMap<String, usize>,
    eqs: &[(usize, ExpressionId)],
) -> Vec<(usize, ExpressionId)> {
    if eqs.len() <= 1 {
        return eqs.to_vec();
    }
    let defined: BTreeMap<usize, usize> = eqs
        .iter()
        .enumerate()
        .map(|(i, &(sym, _))| (sym, i))
        .collect();
    let mut deps: Vec<BTreeSet<usize>> = Vec::with_capacity(eqs.len());
    for &(_, expr) in eqs {
        let mut set = BTreeSet::new();
        collect_expr_syms(model, sym_map, expr, &mut set);
        deps.push(
            set.into_iter()
                .filter_map(|s| defined.get(&s).copied())
                .collect(),
        );
    }

    let mut visited = vec![false; eqs.len()];
    let mut order = Vec::with_capacity(eqs.len());
    fn dfs(u: usize, deps: &[BTreeSet<usize>], visited: &mut [bool], order: &mut Vec<usize>) {
        if visited[u] {
            return;
        }
        visited[u] = true;
        for &v in &deps[u] {
            dfs(v, deps, visited, order);
        }
        order.push(u);
    }
    for i in 0..eqs.len() {
        dfs(i, &deps, &mut visited, &mut order);
    }
    order.into_iter().map(|i| eqs[i]).collect()
}

fn collect_expr_syms(
    model: &ValidatedModel,
    sym_map: &BTreeMap<String, usize>,
    id: ExpressionId,
    out: &mut BTreeSet<usize>,
) {
    let Some(node) = model.expression(id) else {
        return;
    };
    match node.kind() {
        ExpressionKind::Symbol { name, kind } => {
            if *kind != SymbolKind::FunctionParameter
                && let Some(&idx) = sym_map.get(name)
            {
                out.insert(idx);
            }
        }
        ExpressionKind::Unary { child, .. } | ExpressionKind::Not { child } => {
            collect_expr_syms(model, sym_map, *child, out);
        }
        ExpressionKind::Binary { left, right, .. } => {
            collect_expr_syms(model, sym_map, *left, out);
            collect_expr_syms(model, sym_map, *right, out);
        }
        ExpressionKind::Call { args, .. }
        | ExpressionKind::UserCall { args, .. }
        | ExpressionKind::Compare { args, .. }
        | ExpressionKind::Logical { args, .. } => {
            for &a in args {
                collect_expr_syms(model, sym_map, a, out);
            }
        }
        ExpressionKind::Piecewise {
            branches,
            otherwise,
        } => {
            for b in branches {
                collect_expr_syms(model, sym_map, b.value, out);
                collect_expr_syms(model, sym_map, b.condition, out);
            }
            if let Some(o) = otherwise {
                collect_expr_syms(model, sym_map, *o, out);
            }
        }
        ExpressionKind::Boolean(_)
        | ExpressionKind::Rational { .. }
        | ExpressionKind::Time
        | ExpressionKind::Constant(_)
        | ExpressionKind::Number { .. } => {}
    }
}

pub(super) fn eval_num(
    model: &ValidatedModel,
    sym_map: &BTreeMap<String, usize>,
    fn_map: &BTreeMap<&str, &crate::FunctionDefinition>,
    vals: &[f64],
    t: f64,
    id: ExpressionId,
    fn_locals: &BTreeMap<&str, f64>,
) -> Result<f64, Diagnostic> {
    let node = model.expression(id).expect("valid expression ID");
    match node.kind() {
        ExpressionKind::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        ExpressionKind::Not { .. }
        | ExpressionKind::Compare { .. }
        | ExpressionKind::Logical { .. } => {
            let b = eval_bool(model, sym_map, fn_map, vals, t, id, fn_locals)?;
            Ok(if b { 1.0 } else { 0.0 })
        }
        ExpressionKind::Piecewise {
            branches,
            otherwise,
        } => {
            for b in branches {
                if eval_bool(model, sym_map, fn_map, vals, t, b.condition, fn_locals)? {
                    return eval_num(model, sym_map, fn_map, vals, t, b.value, fn_locals);
                }
            }
            match otherwise {
                Some(o) => eval_num(model, sym_map, fn_map, vals, t, *o, fn_locals),
                None => Ok(0.0),
            }
        }
        ExpressionKind::Rational {
            numerator,
            denominator,
        } => Ok((*numerator as f64) / (*denominator as f64)),
        ExpressionKind::Time => Ok(t),
        ExpressionKind::Constant(c) => Ok(match c {
            NumericConstant::Pi => std::f64::consts::PI,
            NumericConstant::ExponentialE => std::f64::consts::E,
        }),
        ExpressionKind::Number { value, .. } => Ok(*value),
        ExpressionKind::Symbol { name, kind } => {
            if *kind == SymbolKind::FunctionParameter
                && let Some(&v) = fn_locals.get(name.as_str())
            {
                return Ok(v);
            }
            Ok(vals[sym_map[name.as_str()]])
        }
        ExpressionKind::Unary { op, child } => {
            let v = eval_num(model, sym_map, fn_map, vals, t, *child, fn_locals)?;
            Ok(match op {
                UnaryOp::Plus => v,
                UnaryOp::Minus => -v,
            })
        }
        ExpressionKind::Binary { op, left, right } => {
            let l = eval_num(model, sym_map, fn_map, vals, t, *left, fn_locals)?;
            let r = eval_num(model, sym_map, fn_map, vals, t, *right, fn_locals)?;
            Ok(match op {
                BinaryOp::Add => l + r,
                BinaryOp::Subtract => l - r,
                BinaryOp::Multiply => l * r,
                BinaryOp::Divide => l / r,
                BinaryOp::Power => l.powf(r),
            })
        }
        ExpressionKind::Call { function, args } => {
            let mut eval_args = Vec::with_capacity(args.len());
            for &a in args {
                eval_args.push(eval_num(model, sym_map, fn_map, vals, t, a, fn_locals)?);
            }
            Ok(match function {
                Builtin::Sin => eval_args[0].sin(),
                Builtin::Cos => eval_args[0].cos(),
                Builtin::Tan => eval_args[0].tan(),
                Builtin::Exp => eval_args[0].exp(),
                Builtin::Ln => eval_args[0].ln(),
                Builtin::Log10 => eval_args[0].log10(),
                Builtin::Log => eval_args[1].log(eval_args[0]),
                Builtin::Sqrt => eval_args[0].sqrt(),
                Builtin::Abs => eval_args[0].abs(),
                Builtin::Floor => eval_args[0].floor(),
                Builtin::Ceiling => eval_args[0].ceil(),
                Builtin::Power => eval_args[0].powf(eval_args[1]),
                Builtin::Min => eval_args.iter().copied().fold(f64::INFINITY, f64::min),
                Builtin::Max => eval_args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        ExpressionKind::UserCall { name, args } => {
            let func = fn_map[name.as_str()];
            let mut sub_locals = BTreeMap::new();
            for (param_name, &arg_id) in func.parameters().iter().zip(args.iter()) {
                let v = eval_num(model, sym_map, fn_map, vals, t, arg_id, fn_locals)?;
                sub_locals.insert(param_name.as_str(), v);
            }
            eval_num(model, sym_map, fn_map, vals, t, func.body(), &sub_locals)
        }
    }
}

pub(super) fn eval_bool(
    model: &ValidatedModel,
    sym_map: &BTreeMap<String, usize>,
    fn_map: &BTreeMap<&str, &crate::FunctionDefinition>,
    vals: &[f64],
    t: f64,
    id: ExpressionId,
    fn_locals: &BTreeMap<&str, f64>,
) -> Result<bool, Diagnostic> {
    let node = model.expression(id).expect("valid expression ID");
    match node.kind() {
        ExpressionKind::Boolean(b) => Ok(*b),
        ExpressionKind::Not { child } => Ok(!eval_bool(
            model, sym_map, fn_map, vals, t, *child, fn_locals,
        )?),
        ExpressionKind::Logical { op, args } => match op {
            LogicalOp::And => {
                for &a in args {
                    if !eval_bool(model, sym_map, fn_map, vals, t, a, fn_locals)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            LogicalOp::Or => {
                for &a in args {
                    if eval_bool(model, sym_map, fn_map, vals, t, a, fn_locals)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        },
        ExpressionKind::Compare { op, args } => {
            let mut prev = eval_num(model, sym_map, fn_map, vals, t, args[0], fn_locals)?;
            for &a in &args[1..] {
                let curr = eval_num(model, sym_map, fn_map, vals, t, a, fn_locals)?;
                let ok = match op {
                    ComparisonOp::Equal => prev == curr,
                    ComparisonOp::NotEqual => prev != curr,
                    ComparisonOp::Less => prev < curr,
                    ComparisonOp::LessEqual => prev <= curr,
                    ComparisonOp::Greater => prev > curr,
                    ComparisonOp::GreaterEqual => prev >= curr,
                };
                if !ok {
                    return Ok(false);
                }
                prev = curr;
            }
            Ok(true)
        }
        _ => {
            let v = eval_num(model, sym_map, fn_map, vals, t, id, fn_locals)?;
            Ok(v != 0.0)
        }
    }
}
