//! Phase 8 module flattening, port binding, dotted member access, and `is` alias resolution.
//!
//! Flattens multi-module documents and `is` aliases into a single flat statement stream
//! before expression normalization and symbol resolution, while preserving source-ordered
//! first-seen semantics, per-module `_J` / `_E` / `_sys` counters, and caller-over-submodel
//! initialization and compartment precedence.

use crate::syntax::{
    Expr, ExprKind, InitialValue, Name, ParsedDocument, ParsedEventAssignment, ParsedModule,
    Participant, Statement, error,
};
use crate::{Diagnostic, DiagnosticCode, Span};
use std::collections::{BTreeMap, BTreeSet};

const MAX_MODULE_NESTING: usize = 32;

#[derive(Clone)]
struct TaggedStatement {
    statement: Statement,
    from_submodel: bool,
}

#[derive(Clone)]
struct FlattenedModule {
    ports: Vec<Name>,
    statements: Vec<TaggedStatement>,
    member_map: BTreeMap<String, String>,
    symbols: BTreeSet<String>,
}

pub(super) fn flatten(source: &ParsedDocument) -> Result<ParsedDocument, Diagnostic> {
    let has_special_stmt = source.statements.iter().any(|s| {
        matches!(
            s,
            Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. }
        )
    });
    let has_dot = source.expressions.iter().any(|e| {
        matches!(
            &e.kind,
            ExprKind::Symbol(n) | ExprKind::UserCall { name: n, .. } if n.text.contains('.')
        )
    }) || source.statements.iter().any(statement_has_dot);
    if source.modules.is_empty() && !has_special_stmt && !has_dot {
        return Ok(source.clone());
    }

    let mut expressions = source.expressions.clone();
    let fn_names: BTreeSet<String> = source
        .functions
        .iter()
        .map(|f| f.name.text.clone())
        .collect();

    let mut available_modules = BTreeMap::<String, FlattenedModule>::new();
    let mut seen_module_names = BTreeSet::<String>::new();
    for m in &source.modules {
        if !seen_module_names.insert(m.name.text.clone()) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                m.name.span,
                format!("duplicate module definition {:?}", m.name.text),
            ));
        }
        if fn_names.contains(&m.name.text) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                m.name.span,
                format!(
                    "module name {:?} conflicts with a function definition",
                    m.name.text
                ),
            ));
        }
    }

    for module in &source.modules {
        let stack = vec![module.name.text.clone()];
        let flat = flatten_module(
            module,
            &available_modules,
            &fn_names,
            &mut expressions,
            &stack,
        )?;
        available_modules.insert(module.name.text.clone(), flat);
    }

    let origin = source.source.span(0, 0).expect("origin span");
    let main_module = ParsedModule {
        name: Name {
            text: source.name.clone(),
            span: origin,
        },
        ports: Vec::new(),
        statements: source.statements.clone(),
        span: origin,
    };
    let stack = vec![source.name.clone()];
    let main_flat = flatten_module(
        &main_module,
        &available_modules,
        &fn_names,
        &mut expressions,
        &stack,
    )?;

    let mut out = source.clone();
    out.modules.clear();
    out.expressions = expressions;
    out.statements = main_flat
        .statements
        .into_iter()
        .map(|t| t.statement)
        .collect();
    Ok(out)
}

fn flatten_module(
    module: &ParsedModule,
    available_modules: &BTreeMap<String, FlattenedModule>,
    fn_names: &BTreeSet<String>,
    expressions: &mut Vec<Expr>,
    stack: &[String],
) -> Result<FlattenedModule, Diagnostic> {
    if stack.len() > MAX_MODULE_NESTING {
        return Err(error(
            DiagnosticCode::ResourceLimit,
            module.span,
            "submodel nesting depth exceeds limit",
        ));
    }
    let mut port_set = BTreeSet::new();
    for port in &module.ports {
        if port.text.contains('.') {
            return Err(error(
                DiagnosticCode::Syntax,
                port.span,
                "module port names cannot be dotted member references",
            ));
        }
        if fn_names.contains(&port.text) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                port.span,
                format!("port {:?} conflicts with a function name", port.text),
            ));
        }
        if !port_set.insert(port.text.clone()) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                port.span,
                format!(
                    "duplicate port {:?} in module {:?}",
                    port.text, module.name.text
                ),
            ));
        }
    }

    let mut emitted: Vec<TaggedStatement> = Vec::new();
    let mut submodels: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut submodel_symbols: BTreeSet<String> = BTreeSet::new();
    let mut deletable_submodel_symbols: BTreeSet<String> = BTreeSet::new();
    let mut deleted_symbols: BTreeMap<String, Span> = BTreeMap::new();
    let mut conv_factors: Vec<Name> = Vec::new();
    let mut known_symbols: BTreeSet<String> = port_set.clone();
    let mut reserved_units: BTreeSet<String> = BTreeSet::new();
    let mut next_sys = 0usize;
    let mut next_rxn = 0usize;
    let mut next_evt = 0usize;

    // Emit initial port placeholders so formal ports preserve signature first-seen order.
    for port in &module.ports {
        emitted.push(TaggedStatement {
            statement: Statement::Parameter {
                name: port.clone(),
                value: None,
                qualifier: None,
            },
            from_submodel: false,
        });
    }

    for raw_stmt in &module.statements {
        match raw_stmt {
            Statement::Submodel {
                instance,
                module: sub_name,
                args,
                timeconv,
                extentconv,
                span,
            } => {
                if sub_name.text == module.name.text || stack.contains(&sub_name.text) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        *span,
                        format!(
                            "cyclic submodel instantiation involving {:?}",
                            sub_name.text
                        ),
                    ));
                }
                let Some(sub_flat) = available_modules.get(&sub_name.text) else {
                    if fn_names.contains(&sub_name.text)
                        || crate::Builtin::from_name(&sub_name.text).is_some()
                    {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            sub_name.span,
                            format!(
                                "{:?} is a function, not a module, and cannot be instantiated as a statement",
                                sub_name.text
                            ),
                        ));
                    }
                    return Err(error(
                        DiagnosticCode::Syntax,
                        sub_name.span,
                        format!(
                            "unknown module {:?} in submodel instantiation",
                            sub_name.text
                        ),
                    ));
                };
                if args.len() != sub_flat.ports.len() {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        *span,
                        format!(
                            "module {:?} expects {} port argument(s), found {}",
                            sub_name.text,
                            sub_flat.ports.len(),
                            args.len()
                        ),
                    ));
                }
                let inst_id = if !instance.text.is_empty() {
                    if instance.text.contains('.') {
                        return Err(error(
                            DiagnosticCode::Syntax,
                            instance.span,
                            "submodel instance name cannot contain '.'",
                        ));
                    }
                    if submodels.contains_key(&instance.text)
                        || known_symbols.contains(&instance.text)
                        || reserved_units.contains(&instance.text)
                        || fn_names.contains(&instance.text)
                    {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            instance.span,
                            format!(
                                "duplicate or conflicting submodel instance {:?}",
                                instance.text
                            ),
                        ));
                    }
                    instance.text.clone()
                } else {
                    loop {
                        let candidate = format!("_sys{next_sys}");
                        next_sys += 1;
                        if !submodels.contains_key(&candidate)
                            && !known_symbols.contains(&candidate)
                            && !reserved_units.contains(&candidate)
                        {
                            break candidate;
                        }
                    }
                };

                let mut resolved_args = Vec::with_capacity(args.len());
                for arg in args {
                    let resolved = resolve_name(arg, &submodels, &aliases)?;
                    known_symbols.insert(resolved.text.clone());
                    resolved_args.push(resolved);
                }
                let resolved_timeconv = match timeconv {
                    Some(tc) => {
                        let r = resolve_name(tc, &submodels, &aliases)?;
                        if fn_names.contains(&r.text) || available_modules.contains_key(&r.text) {
                            return Err(error(
                                DiagnosticCode::DeclarationConflict,
                                tc.span,
                                format!(
                                    "conversion factor {:?} conflicts with a function or module",
                                    r.text
                                ),
                            ));
                        }
                        conv_factors.push(r.clone());
                        Some(r)
                    }
                    None => None,
                };
                let resolved_extentconv = match extentconv {
                    Some(ec) => {
                        let r = resolve_name(ec, &submodels, &aliases)?;
                        if fn_names.contains(&r.text) || available_modules.contains_key(&r.text) {
                            return Err(error(
                                DiagnosticCode::DeclarationConflict,
                                ec.span,
                                format!(
                                    "conversion factor {:?} conflicts with a function or module",
                                    r.text
                                ),
                            ));
                        }
                        conv_factors.push(r.clone());
                        Some(r)
                    }
                    None => None,
                };

                // Check for unit definitions inside instantiated submodels.
                for tagged in &sub_flat.statements {
                    if let Statement::UnitDefinition { name, .. } = &tagged.statement {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            name.span,
                            "unit definitions inside instantiated submodels are outside this slice; declare units in the top-level model",
                        ));
                    }
                }

                let mut port_map = BTreeMap::<String, Name>::new();
                for (port, actual) in sub_flat.ports.iter().zip(&resolved_args) {
                    port_map.insert(port.text.clone(), actual.clone());
                }

                let map_sub_symbol = |sym: &str, sym_span: Span| -> Name {
                    if let Some(bound) = port_map.get(sym) {
                        Name {
                            text: bound.text.clone(),
                            span: bound.span,
                        }
                    } else {
                        Name {
                            text: format!("{inst_id}_{sym}"),
                            span: sym_span,
                        }
                    }
                };

                let mut inst_members = BTreeMap::<String, String>::new();
                for (orig_member, canonical_in_sub) in &sub_flat.member_map {
                    let mapped = map_sub_symbol(canonical_in_sub, *span).text;
                    inst_members.insert(orig_member.clone(), mapped);
                }
                for sym in &sub_flat.symbols {
                    let mapped = map_sub_symbol(sym, *span).text;
                    if !port_map.contains_key(sym) && !known_symbols.contains(&mapped) {
                        submodel_symbols.insert(mapped.clone());
                        deletable_submodel_symbols.insert(mapped.clone());
                    }
                    known_symbols.insert(mapped.clone());
                    inst_members.entry(sym.clone()).or_insert(mapped);
                }
                submodels.insert(inst_id.clone(), inst_members);

                // Register actual port arguments in caller argument order before internal submodel statements.
                for actual in &resolved_args {
                    emitted.push(TaggedStatement {
                        statement: Statement::Parameter {
                            name: actual.clone(),
                            value: None,
                            qualifier: None,
                        },
                        from_submodel: true,
                    });
                }

                for tagged in &sub_flat.statements {
                    let rewritten = rewrite_statement_from_submodel(
                        &tagged.statement,
                        &map_sub_symbol,
                        resolved_timeconv.as_ref(),
                        resolved_extentconv.as_ref(),
                        expressions,
                    )?;
                    emitted.push(TaggedStatement {
                        statement: rewritten,
                        from_submodel: true,
                    });
                }
                for conv in [resolved_timeconv, resolved_extentconv]
                    .into_iter()
                    .flatten()
                {
                    known_symbols.insert(conv.text.clone());
                    emitted.push(TaggedStatement {
                        statement: Statement::Parameter {
                            name: conv,
                            value: None,
                            qualifier: None,
                        },
                        from_submodel: false,
                    });
                }
            }
            Statement::Delete { targets, .. } => {
                for target in targets {
                    let resolved = resolve_name(target, &submodels, &aliases)?;
                    let del_sym = resolved.text;
                    if !deletable_submodel_symbols.contains(&del_sym) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            target.span,
                            format!(
                                "cannot delete {:?}: deletions are valid only for unsynchronized submodel elements",
                                target.text
                            ),
                        ));
                    }
                    deletable_submodel_symbols.remove(&del_sym);
                    submodel_symbols.remove(&del_sym);
                    known_symbols.remove(&del_sym);
                    for members in submodels.values_mut() {
                        members.retain(|_, v| *v != del_sym);
                    }
                    aliases.retain(|k, v| *k != del_sym && *v != del_sym);
                    deleted_symbols.insert(del_sym.clone(), target.span);
                    apply_deletion(&mut emitted, &del_sym, target.span)?;
                }
            }
            Statement::Alias { left, right, span } => {
                let left_raw_base = left.text.split('.').next().unwrap_or(&left.text);
                let right_raw_base = right.text.split('.').next().unwrap_or(&right.text);
                if port_set.contains(left_raw_base)
                    && !left.text.contains('.')
                    && !port_set.contains(right_raw_base)
                {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        *span,
                        "aliasing a module port onto a local symbol triggers an upstream libAntimony crash; use `local is port`",
                    ));
                }
                let l_res = resolve_name(left, &submodels, &aliases)?;
                let r_res = resolve_name(right, &submodels, &aliases)?;
                known_symbols.insert(l_res.text.clone());
                known_symbols.insert(r_res.text.clone());
                if l_res.text == r_res.text {
                    continue;
                }
                let l_is_sub = submodel_symbols.contains(&l_res.text);
                let r_is_sub = submodel_symbols.contains(&r_res.text);
                let (alias, canonical) = if r_is_sub && !l_is_sub {
                    (r_res, l_res)
                } else {
                    (l_res, r_res)
                };
                if !l_is_sub || !r_is_sub {
                    submodel_symbols.remove(&canonical.text);
                    deletable_submodel_symbols.remove(&canonical.text);
                    deletable_submodel_symbols.remove(&alias.text);
                }
                if !statement_mentions(&emitted, expressions, &canonical.text)
                    && !statement_mentions(&emitted, expressions, &alias.text)
                {
                    emitted.push(TaggedStatement {
                        statement: Statement::Parameter {
                            name: canonical.clone(),
                            value: None,
                            qualifier: None,
                        },
                        from_submodel: false,
                    });
                }
                let canonical_has_caller_init = has_initializer(&emitted, &canonical.text, false);
                let canonical_has_any_init = has_initializer(&emitted, &canonical.text, true);
                let alias_has_caller_init = has_initializer(&emitted, &alias.text, false);
                if canonical_has_caller_init || (canonical_has_any_init && !alias_has_caller_init) {
                    strip_initializers(&mut emitted, &alias.text, true);
                } else if alias_has_caller_init && !canonical_has_caller_init {
                    strip_initializers(&mut emitted, &canonical.text, true);
                }

                let canonical_has_caller_comp = has_compartment(&emitted, &canonical.text, false);
                let canonical_has_any_comp = has_compartment(&emitted, &canonical.text, true);
                let alias_has_caller_comp = has_compartment(&emitted, &alias.text, false);
                if canonical_has_caller_comp || (canonical_has_any_comp && !alias_has_caller_comp) {
                    strip_compartments(&mut emitted, &alias.text, true);
                } else if alias_has_caller_comp && !canonical_has_caller_comp {
                    strip_compartments(&mut emitted, &canonical.text, true);
                }

                for target in aliases.values_mut() {
                    if *target == alias.text {
                        *target = canonical.text.clone();
                    }
                }
                aliases.insert(alias.text.clone(), canonical.text.clone());
                for members in submodels.values_mut() {
                    for target in members.values_mut() {
                        if *target == alias.text {
                            *target = canonical.text.clone();
                        }
                    }
                }
                for tagged in &mut emitted {
                    rewrite_symbol_in_statement(
                        &mut tagged.statement,
                        expressions,
                        &alias.text,
                        &canonical.text,
                    );
                }
            }
            other => {
                let resolved_stmt = resolve_statement_names(
                    other,
                    &submodels,
                    &aliases,
                    &mut known_symbols,
                    &mut reserved_units,
                    &mut next_rxn,
                    &mut next_evt,
                    expressions,
                )?;
                collect_statement_symbols(&resolved_stmt, expressions, &mut known_symbols);
                if let Some(sym) = initialized_target(&resolved_stmt) {
                    submodel_symbols.remove(&sym);
                }
                emitted.push(TaggedStatement {
                    statement: resolved_stmt,
                    from_submodel: false,
                });
            }
        }
    }

    for (del_sym, del_span) in &deleted_symbols {
        if statement_mentions(&emitted, expressions, del_sym) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                *del_span,
                format!(
                    "deleted submodel element {del_sym:?} is still referenced by retained statements or expressions; delete or override the dependent reaction, rule, or compartment user as well"
                ),
            ));
        }
    }

    for cf in &conv_factors {
        let cf_sym = resolve_alias(&aliases, &cf.text);
        for tagged in &emitted {
            let bad_role = match &tagged.statement {
                Statement::Compartment { name, .. } if name.text == cf_sym => Some("compartment"),
                Statement::Species { name, .. } if name.text == cf_sym => Some("species"),
                Statement::Reaction {
                    name,
                    reactants,
                    products,
                    ..
                } => {
                    if name.text == cf_sym {
                        Some("reaction")
                    } else if reactants
                        .iter()
                        .chain(products)
                        .any(|p| p.name.text == cf_sym)
                    {
                        Some("species")
                    } else {
                        None
                    }
                }
                Statement::Event { name, .. } if name.text == cf_sym => Some("event"),
                _ => None,
            };
            if let Some(role) = bad_role {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    cf.span,
                    format!(
                        "conversion factor {:?} cannot be a {role}; only parameters (formulas) may be used as conversion factors",
                        cf.text
                    ),
                ));
            }
        }
    }

    let mut caller_inits = BTreeSet::new();
    let mut caller_comps = BTreeSet::new();
    for tagged in &emitted {
        if !tagged.from_submodel {
            if let Some(sym) = initialized_target(&tagged.statement) {
                caller_inits.insert(sym);
            }
            if let Statement::Species {
                name,
                compartment: Some(_),
                ..
            } = &tagged.statement
            {
                caller_comps.insert(name.text.clone());
            }
        }
    }
    for sym in &caller_inits {
        strip_initializers(&mut emitted, sym, false);
    }
    for sym in &caller_comps {
        strip_compartments(&mut emitted, sym, false);
    }

    let mut member_map = BTreeMap::new();
    for sym in &known_symbols {
        let resolved = resolve_alias(&aliases, sym);
        member_map.insert(sym.clone(), resolved);
    }
    for (alias, canonical) in &aliases {
        member_map.insert(alias.clone(), canonical.clone());
    }
    let final_symbols = member_map.values().cloned().collect::<BTreeSet<_>>();

    Ok(FlattenedModule {
        ports: module.ports.clone(),
        statements: emitted,
        member_map,
        symbols: final_symbols,
    })
}

fn statement_has_dot(statement: &Statement) -> bool {
    match statement {
        Statement::UnitDefinition { name, factors } => {
            name.text.contains('.') || factors.iter().any(|f| f.name.text.contains('.'))
        }
        Statement::UnitAnnotation { name, unit } => {
            name.text.contains('.') || unit.text.contains('.')
        }
        Statement::Compartment { name, .. }
        | Statement::Parameter { name, .. }
        | Statement::Initial { name, .. }
        | Statement::Rule { name, .. } => name.text.contains('.'),
        Statement::Species {
            name, compartment, ..
        } => name.text.contains('.') || compartment.as_ref().is_some_and(|c| c.text.contains('.')),
        Statement::Reaction {
            name,
            reactants,
            products,
            ..
        } => {
            name.text.contains('.')
                || reactants.iter().any(|p| p.name.text.contains('.'))
                || products.iter().any(|p| p.name.text.contains('.'))
        }
        Statement::Event {
            name, assignments, ..
        } => name.text.contains('.') || assignments.iter().any(|a| a.variable.text.contains('.')),
        Statement::Alias { left, right, .. } => left.text.contains('.') || right.text.contains('.'),
        Statement::Submodel {
            instance,
            module,
            args,
            timeconv,
            extentconv,
            ..
        } => {
            instance.text.contains('.')
                || module.text.contains('.')
                || args.iter().any(|a| a.text.contains('.'))
                || timeconv.as_ref().is_some_and(|t| t.text.contains('.'))
                || extentconv.as_ref().is_some_and(|e| e.text.contains('.'))
        }
        Statement::Delete { targets, .. } => targets.iter().any(|t| t.text.contains('.')),
    }
}

fn resolve_alias(aliases: &BTreeMap<String, String>, name: &str) -> String {
    let mut current = name;
    let mut steps = 0;
    while let Some(next) = aliases.get(current) {
        current = next;
        steps += 1;
        if steps > 1024 {
            break;
        }
    }
    current.to_owned()
}

fn resolve_name(
    name: &Name,
    submodels: &BTreeMap<String, BTreeMap<String, String>>,
    aliases: &BTreeMap<String, String>,
) -> Result<Name, Diagnostic> {
    if let Some((inst, member)) = name.text.split_once('.') {
        let curr_sub = submodels.get(inst).ok_or_else(|| {
            error(
                DiagnosticCode::UnresolvedSymbol,
                name.span,
                format!("submodel instance {inst:?} is not declared before member access"),
            )
        })?;
        if member.contains('.') {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                name.span,
                "chained multi-dot member access is outside this slice; access direct submodel members or bind ports",
            ));
        }
        let target = curr_sub.get(member).ok_or_else(|| {
            error(
                DiagnosticCode::UnresolvedSymbol,
                name.span,
                format!("member {member:?} does not exist in submodel {inst:?}"),
            )
        })?;
        Ok(Name {
            text: resolve_alias(aliases, target),
            span: name.span,
        })
    } else {
        Ok(Name {
            text: resolve_alias(aliases, &name.text),
            span: name.span,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_statement_names(
    statement: &Statement,
    submodels: &BTreeMap<String, BTreeMap<String, String>>,
    aliases: &BTreeMap<String, String>,
    known_symbols: &mut BTreeSet<String>,
    reserved_units: &mut BTreeSet<String>,
    next_rxn: &mut usize,
    next_evt: &mut usize,
    expressions: &mut Vec<Expr>,
) -> Result<Statement, Diagnostic> {
    Ok(match statement {
        Statement::UnitDefinition { name, factors } => {
            reserved_units.insert(name.text.clone());
            for f in factors {
                reserved_units.insert(f.name.text.clone());
            }
            Statement::UnitDefinition {
                name: name.clone(),
                factors: factors.clone(),
            }
        }
        Statement::UnitAnnotation { name, unit } => {
            reserved_units.insert(unit.text.clone());
            let name = if crate::model::units::ModelUnit::from_name(&name.text).is_some() {
                name.clone()
            } else {
                resolve_name(name, submodels, aliases)?
            };
            Statement::UnitAnnotation {
                name,
                unit: unit.clone(),
            }
        }
        Statement::Compartment {
            name,
            value,
            qualifier,
        } => {
            let name = resolve_name(name, submodels, aliases)?;
            let value = match value {
                Some(InitialValue::Scalar(v)) => Some(InitialValue::Scalar(*v)),
                Some(InitialValue::Expression(root)) => Some(InitialValue::Expression(
                    clone_and_resolve_expr(*root, submodels, aliases, expressions)?,
                )),
                None => None,
            };
            Statement::Compartment {
                name,
                value,
                qualifier: *qualifier,
            }
        }
        Statement::Species {
            name,
            compartment,
            boundary,
            substance_only,
            qualifier,
        } => {
            let name = resolve_name(name, submodels, aliases)?;
            let compartment = match compartment {
                Some(c) => Some(resolve_name(c, submodels, aliases)?),
                None => None,
            };
            Statement::Species {
                name,
                compartment,
                boundary: *boundary,
                substance_only: *substance_only,
                qualifier: *qualifier,
            }
        }
        Statement::Parameter {
            name,
            value,
            qualifier,
        } => {
            let name = resolve_name(name, submodels, aliases)?;
            let value = match value {
                Some(InitialValue::Scalar(v)) => Some(InitialValue::Scalar(*v)),
                Some(InitialValue::Expression(root)) => Some(InitialValue::Expression(
                    clone_and_resolve_expr(*root, submodels, aliases, expressions)?,
                )),
                None => None,
            };
            Statement::Parameter {
                name,
                value,
                qualifier: *qualifier,
            }
        }
        Statement::Initial { name, value } => {
            let name = resolve_name(name, submodels, aliases)?;
            let value =
                match value {
                    InitialValue::Scalar(v) => InitialValue::Scalar(*v),
                    InitialValue::Expression(root) => InitialValue::Expression(
                        clone_and_resolve_expr(*root, submodels, aliases, expressions)?,
                    ),
                };
            Statement::Initial { name, value }
        }
        Statement::Rule {
            kind,
            name,
            expression,
            lexical_scalar,
            qualifier,
            span,
        } => {
            let name = resolve_name(name, submodels, aliases)?;
            let expression = clone_and_resolve_expr(*expression, submodels, aliases, expressions)?;
            Statement::Rule {
                kind: *kind,
                name,
                expression,
                lexical_scalar: *lexical_scalar,
                qualifier: *qualifier,
                span: *span,
            }
        }
        Statement::Reaction {
            name,
            start,
            reactants,
            products,
            reversible,
            rate,
        } => {
            let mut resolved_reactants = Vec::with_capacity(reactants.len());
            for p in reactants {
                let p_name = resolve_name(&p.name, submodels, aliases)?;
                known_symbols.insert(p_name.text.clone());
                resolved_reactants.push(Participant {
                    stoichiometry: p.stoichiometry,
                    boundary: p.boundary,
                    name: p_name,
                });
            }
            let mut resolved_products = Vec::with_capacity(products.len());
            for p in products {
                let p_name = resolve_name(&p.name, submodels, aliases)?;
                known_symbols.insert(p_name.text.clone());
                resolved_products.push(Participant {
                    stoichiometry: p.stoichiometry,
                    boundary: p.boundary,
                    name: p_name,
                });
            }
            let rate = clone_and_resolve_expr(*rate, submodels, aliases, expressions)?;
            collect_expr_symbols(rate, expressions, known_symbols);
            let name = if name.text.is_empty() {
                let generated = loop {
                    let candidate = format!("_J{}", *next_rxn);
                    *next_rxn += 1;
                    if !known_symbols.contains(&candidate) && !reserved_units.contains(&candidate) {
                        break candidate;
                    }
                };
                Name {
                    text: generated,
                    span: name.span,
                }
            } else {
                resolve_name(name, submodels, aliases)?
            };
            known_symbols.insert(name.text.clone());
            Statement::Reaction {
                name,
                start: *start,
                reactants: resolved_reactants,
                products: resolved_products,
                reversible: *reversible,
                rate,
            }
        }
        Statement::Event {
            name,
            start,
            trigger,
            initial_value,
            persistent,
            use_values_from_trigger_time,
            delay,
            priority,
            assignments,
            span,
        } => {
            let delay = match delay {
                Some(d) => {
                    let d = clone_and_resolve_expr(*d, submodels, aliases, expressions)?;
                    collect_expr_symbols(d, expressions, known_symbols);
                    Some(d)
                }
                None => None,
            };
            let trigger = clone_and_resolve_expr(*trigger, submodels, aliases, expressions)?;
            collect_expr_symbols(trigger, expressions, known_symbols);
            let priority = match priority {
                Some(p) => {
                    let p = clone_and_resolve_expr(*p, submodels, aliases, expressions)?;
                    collect_expr_symbols(p, expressions, known_symbols);
                    Some(p)
                }
                None => None,
            };
            let mut resolved_assignments = Vec::with_capacity(assignments.len());
            for a in assignments {
                let variable = resolve_name(&a.variable, submodels, aliases)?;
                known_symbols.insert(variable.text.clone());
                let expression =
                    clone_and_resolve_expr(a.expression, submodels, aliases, expressions)?;
                collect_expr_symbols(expression, expressions, known_symbols);
                resolved_assignments.push(ParsedEventAssignment {
                    variable,
                    expression,
                    span: a.span,
                });
            }
            let name = if name.text.is_empty() {
                let generated = loop {
                    let candidate = format!("_E{}", *next_evt);
                    *next_evt += 1;
                    if !known_symbols.contains(&candidate) && !reserved_units.contains(&candidate) {
                        break candidate;
                    }
                };
                Name {
                    text: generated,
                    span: *span,
                }
            } else {
                resolve_name(name, submodels, aliases)?
            };
            known_symbols.insert(name.text.clone());
            Statement::Event {
                name,
                start: *start,
                trigger,
                initial_value: *initial_value,
                persistent: *persistent,
                use_values_from_trigger_time: *use_values_from_trigger_time,
                delay,
                priority,
                assignments: resolved_assignments,
                span: *span,
            }
        }
        Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => {
            unreachable!("handled by caller")
        }
    })
}

fn clone_and_resolve_expr(
    root: usize,
    submodels: &BTreeMap<String, BTreeMap<String, String>>,
    aliases: &BTreeMap<String, String>,
    expressions: &mut Vec<Expr>,
) -> Result<usize, Diagnostic> {
    clone_expr_with(root, expressions, None, &|name| {
        resolve_name(name, submodels, aliases)
    })
}

fn wrap_binary_symbol(
    expressions: &mut Vec<Expr>,
    left: usize,
    op: crate::BinaryOp,
    sym: &Name,
) -> usize {
    let span = expressions[left].span;
    let depth = expressions[left].depth;
    let right = expressions.len();
    expressions.push(Expr {
        kind: ExprKind::Symbol(sym.clone()),
        span: sym.span,
        depth: 1,
    });
    let wrapped = expressions.len();
    expressions.push(Expr {
        kind: ExprKind::Binary { op, left, right },
        span,
        depth: depth + 1,
    });
    wrapped
}

fn rewrite_statement_from_submodel(
    statement: &Statement,
    map_sym: &impl Fn(&str, Span) -> Name,
    timeconv: Option<&Name>,
    extentconv: Option<&Name>,
    expressions: &mut Vec<Expr>,
) -> Result<Statement, Diagnostic> {
    let map_expr = |root: usize, exprs: &mut Vec<Expr>| -> Result<usize, Diagnostic> {
        clone_expr_with(root, exprs, timeconv, &|name| {
            Ok(map_sym(&name.text, name.span))
        })
    };
    Ok(match statement {
        Statement::UnitDefinition { name, factors } => Statement::UnitDefinition {
            name: name.clone(),
            factors: factors.clone(),
        },
        Statement::UnitAnnotation { name, unit } => {
            let name = if crate::model::units::ModelUnit::from_name(&name.text).is_some() {
                name.clone()
            } else {
                map_sym(&name.text, name.span)
            };
            Statement::UnitAnnotation {
                name,
                unit: unit.clone(),
            }
        }
        Statement::Compartment {
            name,
            value,
            qualifier,
        } => Statement::Compartment {
            name: map_sym(&name.text, name.span),
            value: match value {
                Some(InitialValue::Scalar(v)) => Some(InitialValue::Scalar(*v)),
                Some(InitialValue::Expression(r)) => {
                    Some(InitialValue::Expression(map_expr(*r, expressions)?))
                }
                None => None,
            },
            qualifier: *qualifier,
        },
        Statement::Species {
            name,
            compartment,
            boundary,
            substance_only,
            qualifier,
        } => Statement::Species {
            name: map_sym(&name.text, name.span),
            compartment: compartment.as_ref().map(|c| map_sym(&c.text, c.span)),
            boundary: *boundary,
            substance_only: *substance_only,
            qualifier: *qualifier,
        },
        Statement::Parameter {
            name,
            value,
            qualifier,
        } => Statement::Parameter {
            name: map_sym(&name.text, name.span),
            value: match value {
                Some(InitialValue::Scalar(v)) => Some(InitialValue::Scalar(*v)),
                Some(InitialValue::Expression(r)) => {
                    Some(InitialValue::Expression(map_expr(*r, expressions)?))
                }
                None => None,
            },
            qualifier: *qualifier,
        },
        Statement::Initial { name, value } => Statement::Initial {
            name: map_sym(&name.text, name.span),
            value: match value {
                InitialValue::Scalar(v) => InitialValue::Scalar(*v),
                InitialValue::Expression(r) => InitialValue::Expression(map_expr(*r, expressions)?),
            },
        },
        Statement::Rule {
            kind,
            name,
            expression,
            lexical_scalar,
            qualifier,
            span,
        } => {
            let mut expression = map_expr(*expression, expressions)?;
            let mut lexical_scalar = *lexical_scalar;
            if *kind == crate::RuleKind::Rate
                && let Some(tc) = timeconv
            {
                expression =
                    wrap_binary_symbol(expressions, expression, crate::BinaryOp::Divide, tc);
                lexical_scalar = None;
            }
            Statement::Rule {
                kind: *kind,
                name: map_sym(&name.text, name.span),
                expression,
                lexical_scalar,
                qualifier: *qualifier,
                span: *span,
            }
        }
        Statement::Reaction {
            name,
            start,
            reactants,
            products,
            reversible,
            rate,
        } => {
            let mut rate = map_expr(*rate, expressions)?;
            if let Some(tc) = timeconv {
                rate = wrap_binary_symbol(expressions, rate, crate::BinaryOp::Divide, tc);
            }
            if let Some(ec) = extentconv {
                rate = wrap_binary_symbol(expressions, rate, crate::BinaryOp::Multiply, ec);
            }
            Statement::Reaction {
                name: map_sym(&name.text, name.span),
                start: *start,
                reactants: reactants
                    .iter()
                    .map(|p| Participant {
                        stoichiometry: p.stoichiometry,
                        boundary: p.boundary,
                        name: map_sym(&p.name.text, p.name.span),
                    })
                    .collect(),
                products: products
                    .iter()
                    .map(|p| Participant {
                        stoichiometry: p.stoichiometry,
                        boundary: p.boundary,
                        name: map_sym(&p.name.text, p.name.span),
                    })
                    .collect(),
                reversible: *reversible,
                rate,
            }
        }
        Statement::Event {
            name,
            start,
            trigger,
            initial_value,
            persistent,
            use_values_from_trigger_time,
            delay,
            priority,
            assignments,
            span,
        } => Statement::Event {
            name: map_sym(&name.text, name.span),
            start: *start,
            trigger: map_expr(*trigger, expressions)?,
            initial_value: *initial_value,
            persistent: *persistent,
            use_values_from_trigger_time: *use_values_from_trigger_time,
            delay: match delay {
                Some(d) => {
                    let mut d = map_expr(*d, expressions)?;
                    if let Some(tc) = timeconv {
                        d = wrap_binary_symbol(expressions, d, crate::BinaryOp::Multiply, tc);
                    }
                    Some(d)
                }
                None => None,
            },
            priority: match priority {
                Some(p) => Some(map_expr(*p, expressions)?),
                None => None,
            },
            assignments: assignments
                .iter()
                .map(|a| {
                    Ok(ParsedEventAssignment {
                        variable: map_sym(&a.variable.text, a.variable.span),
                        expression: map_expr(a.expression, expressions)?,
                        span: a.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?,
            span: *span,
        },
        Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => {
            unreachable!("already flattened in submodel")
        }
    })
}

fn clone_expr_with(
    root: usize,
    expressions: &mut Vec<Expr>,
    timeconv: Option<&Name>,
    map_sym: &impl Fn(&Name) -> Result<Name, Diagnostic>,
) -> Result<usize, Diagnostic> {
    let mut stack = vec![(root, false)];
    let mut mapped = BTreeMap::<usize, usize>::new();
    while let Some((id, visited)) = stack.pop() {
        if mapped.contains_key(&id) {
            continue;
        }
        if !visited {
            stack.push((id, true));
            let children: Vec<usize> = expressions[id].kind.children().collect();
            for child in children.into_iter().rev() {
                stack.push((child, false));
            }
            continue;
        }
        let orig = expressions[id].clone();
        if matches!(orig.kind, ExprKind::Time)
            && let Some(tc) = timeconv
        {
            let time_id = expressions.len();
            expressions.push(Expr {
                kind: ExprKind::Time,
                span: orig.span,
                depth: 1,
            });
            let div_id = wrap_binary_symbol(expressions, time_id, crate::BinaryOp::Divide, tc);
            mapped.insert(id, div_id);
            continue;
        }
        let kind = match orig.kind {
            ExprKind::Symbol(name) => ExprKind::Symbol(map_sym(&name)?),
            ExprKind::UserCall { name, args } => {
                if name.text.contains('.') {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        name.span,
                        "dotted function calls are outside this slice",
                    ));
                }
                ExprKind::UserCall {
                    name,
                    args: args.into_iter().map(|c| mapped[&c]).collect(),
                }
            }
            other => other.map_children(|c| mapped[&c]),
        };
        let new_id = expressions.len();
        expressions.push(Expr {
            kind,
            span: orig.span,
            depth: orig.depth,
        });
        mapped.insert(id, new_id);
    }
    Ok(mapped[&root])
}

fn apply_deletion(
    emitted: &mut Vec<TaggedStatement>,
    del_sym: &str,
    del_span: Span,
) -> Result<(), Diagnostic> {
    let mut retained = Vec::with_capacity(emitted.len());
    for mut tagged in std::mem::take(emitted) {
        let remove = match &mut tagged.statement {
            Statement::UnitDefinition { .. } => false,
            Statement::UnitAnnotation { name, .. } => {
                crate::model::units::ModelUnit::from_name(&name.text).is_none()
                    && name.text == del_sym
            }
            Statement::Compartment { name, .. }
            | Statement::Parameter { name, .. }
            | Statement::Initial { name, .. }
            | Statement::Rule { name, .. } => name.text == del_sym,
            Statement::Species { name, .. } => name.text == del_sym,
            Statement::Reaction {
                name,
                reactants,
                products,
                ..
            } => {
                if name.text == del_sym {
                    true
                } else {
                    let had_participants = !reactants.is_empty() || !products.is_empty();
                    reactants.retain(|p| p.name.text != del_sym);
                    products.retain(|p| p.name.text != del_sym);
                    if had_participants && reactants.is_empty() && products.is_empty() {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            del_span,
                            format!(
                                "deleting {del_sym:?} leaves reaction {:?} with no participants; delete the reaction as well",
                                name.text
                            ),
                        ));
                    }
                    false
                }
            }
            Statement::Event {
                name, assignments, ..
            } => {
                if name.text == del_sym {
                    true
                } else {
                    assignments.retain(|a| a.variable.text != del_sym);
                    if assignments.is_empty() {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            del_span,
                            format!(
                                "deleting {del_sym:?} leaves event {:?} with no event assignments; delete the event as well",
                                name.text
                            ),
                        ));
                    }
                    false
                }
            }
            Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => {
                false
            }
        };
        if !remove {
            retained.push(tagged);
        }
    }
    *emitted = retained;
    Ok(())
}

fn collect_statement_symbols(
    statement: &Statement,
    expressions: &[Expr],
    known_symbols: &mut BTreeSet<String>,
) {
    match statement {
        Statement::UnitDefinition { .. } => (),
        Statement::UnitAnnotation { name, .. } => {
            if crate::model::units::ModelUnit::from_name(&name.text).is_none() {
                known_symbols.insert(name.text.clone());
            }
        }
        Statement::Compartment { name, value, .. } | Statement::Parameter { name, value, .. } => {
            known_symbols.insert(name.text.clone());
            if let Some(InitialValue::Expression(root)) = value {
                collect_expr_symbols(*root, expressions, known_symbols);
            }
        }
        Statement::Species {
            name, compartment, ..
        } => {
            known_symbols.insert(name.text.clone());
            if let Some(c) = compartment {
                known_symbols.insert(c.text.clone());
            }
        }
        Statement::Initial { name, value } => {
            known_symbols.insert(name.text.clone());
            if let InitialValue::Expression(root) = value {
                collect_expr_symbols(*root, expressions, known_symbols);
            }
        }
        Statement::Rule {
            name, expression, ..
        } => {
            known_symbols.insert(name.text.clone());
            collect_expr_symbols(*expression, expressions, known_symbols);
        }
        Statement::Reaction {
            name,
            reactants,
            products,
            rate,
            ..
        } => {
            if !name.text.is_empty() {
                known_symbols.insert(name.text.clone());
            }
            for p in reactants.iter().chain(products) {
                known_symbols.insert(p.name.text.clone());
            }
            collect_expr_symbols(*rate, expressions, known_symbols);
        }
        Statement::Event {
            name,
            trigger,
            delay,
            priority,
            assignments,
            ..
        } => {
            if !name.text.is_empty() {
                known_symbols.insert(name.text.clone());
            }
            collect_expr_symbols(*trigger, expressions, known_symbols);
            if let Some(d) = delay {
                collect_expr_symbols(*d, expressions, known_symbols);
            }
            if let Some(p) = priority {
                collect_expr_symbols(*p, expressions, known_symbols);
            }
            for a in assignments {
                known_symbols.insert(a.variable.text.clone());
                collect_expr_symbols(a.expression, expressions, known_symbols);
            }
        }
        Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => (),
    }
}

fn collect_expr_symbols(root: usize, expressions: &[Expr], known_symbols: &mut BTreeSet<String>) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let kind = &expressions[id].kind;
        if let ExprKind::Symbol(name) = kind {
            known_symbols.insert(name.text.clone());
        }
        stack.extend(kind.children());
    }
}

fn statement_mentions(emitted: &[TaggedStatement], expressions: &[Expr], target: &str) -> bool {
    let mut syms = BTreeSet::new();
    for tagged in emitted {
        collect_statement_symbols(&tagged.statement, expressions, &mut syms);
        if syms.contains(target) {
            return true;
        }
    }
    false
}

fn initialized_target(statement: &Statement) -> Option<String> {
    match statement {
        Statement::Compartment {
            name,
            value: Some(_),
            ..
        }
        | Statement::Parameter {
            name,
            value: Some(_),
            ..
        }
        | Statement::Initial { name, .. }
        | Statement::Rule {
            name,
            kind: crate::RuleKind::Assignment,
            ..
        } => Some(name.text.clone()),
        _ => None,
    }
}

fn has_initializer(emitted: &[TaggedStatement], sym: &str, include_submodel: bool) -> bool {
    emitted.iter().any(|t| {
        (include_submodel || !t.from_submodel)
            && initialized_target(&t.statement).as_deref() == Some(sym)
    })
}

fn strip_initializers(emitted: &mut [TaggedStatement], sym: &str, include_caller: bool) {
    for tagged in emitted.iter_mut() {
        if (!include_caller && !tagged.from_submodel)
            || initialized_target(&tagged.statement).as_deref() != Some(sym)
        {
            continue;
        }
        match &mut tagged.statement {
            Statement::Compartment { value, .. } | Statement::Parameter { value, .. } => {
                *value = None;
            }
            Statement::Initial { name, .. } => {
                tagged.statement = Statement::Parameter {
                    name: name.clone(),
                    value: None,
                    qualifier: None,
                };
            }
            Statement::Rule {
                name,
                qualifier,
                kind: crate::RuleKind::Assignment,
                ..
            } => {
                tagged.statement = Statement::Parameter {
                    name: name.clone(),
                    value: None,
                    qualifier: *qualifier,
                };
            }
            _ => (),
        }
    }
}

fn has_compartment(emitted: &[TaggedStatement], sym: &str, include_submodel: bool) -> bool {
    emitted.iter().any(|t| {
        (include_submodel || !t.from_submodel)
            && matches!(
                &t.statement,
                Statement::Species {
                    name,
                    compartment: Some(_),
                    ..
                } if name.text == sym
            )
    })
}

fn strip_compartments(emitted: &mut [TaggedStatement], sym: &str, include_caller: bool) {
    for tagged in emitted.iter_mut() {
        if !include_caller && !tagged.from_submodel {
            continue;
        }
        if let Statement::Species {
            name, compartment, ..
        } = &mut tagged.statement
            && name.text == sym
        {
            *compartment = None;
        }
    }
}

fn rewrite_symbol_in_statement(
    statement: &mut Statement,
    expressions: &mut [Expr],
    from: &str,
    to: &str,
) {
    let rename = |name: &mut Name| {
        if name.text == from {
            name.text = to.to_owned();
        }
    };
    match statement {
        Statement::UnitDefinition { .. } => (),
        Statement::UnitAnnotation { name, .. } => {
            if crate::model::units::ModelUnit::from_name(&name.text).is_none() {
                rename(name);
            }
        }
        Statement::Compartment { name, value, .. } | Statement::Parameter { name, value, .. } => {
            rename(name);
            if let Some(InitialValue::Expression(root)) = value {
                rewrite_symbol_in_expr(*root, expressions, from, to);
            }
        }
        Statement::Species {
            name, compartment, ..
        } => {
            rename(name);
            if let Some(c) = compartment {
                rename(c);
            }
        }
        Statement::Initial { name, value } => {
            rename(name);
            if let InitialValue::Expression(root) = value {
                rewrite_symbol_in_expr(*root, expressions, from, to);
            }
        }
        Statement::Rule {
            name, expression, ..
        } => {
            rename(name);
            rewrite_symbol_in_expr(*expression, expressions, from, to);
        }
        Statement::Reaction {
            name,
            reactants,
            products,
            rate,
            ..
        } => {
            rename(name);
            for p in reactants.iter_mut().chain(products) {
                rename(&mut p.name);
            }
            rewrite_symbol_in_expr(*rate, expressions, from, to);
        }
        Statement::Event {
            name,
            trigger,
            delay,
            priority,
            assignments,
            ..
        } => {
            if !name.text.is_empty() {
                rename(name);
            }
            rewrite_symbol_in_expr(*trigger, expressions, from, to);
            if let Some(d) = delay {
                rewrite_symbol_in_expr(*d, expressions, from, to);
            }
            if let Some(p) = priority {
                rewrite_symbol_in_expr(*p, expressions, from, to);
            }
            for a in assignments {
                rename(&mut a.variable);
                rewrite_symbol_in_expr(a.expression, expressions, from, to);
            }
        }
        Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => (),
    }
}

fn rewrite_symbol_in_expr(root: usize, expressions: &mut [Expr], from: &str, to: &str) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let ExprKind::Symbol(name) = &mut expressions[id].kind
            && name.text == from
        {
            name.text = to.to_owned();
        }
        stack.extend(expressions[id].kind.children());
    }
}
