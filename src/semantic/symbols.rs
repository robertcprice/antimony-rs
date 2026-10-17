//! Source-ordered symbol roles, qualifiers and initialization ownership.
//! Formula roles may be promoted; generated reaction IDs reserve only names
//! encountered so far, including operands in the reaction being declared.
use super::initialization::Initials;
use super::{Entry, Symbol, declare};
use crate::syntax::{
    Expr, ExprKind, InitialValue, Name, ParsedDocument, Statement, UnaryOp, error,
};
use crate::{Diagnostic, DiagnosticCode, Label};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct ResolvedRule {
    pub kind: crate::RuleKind,
    pub expression: usize,
    pub lexical_scalar: Option<f64>,
    pub span: crate::Span,
}
pub(super) type Rules = BTreeMap<String, ResolvedRule>;

#[derive(Clone, Debug)]
pub(super) struct ResolvedEvent {
    pub id: String,
    pub trigger: usize,
    pub initial_value: bool,
    pub persistent: bool,
    pub from_trigger: bool,
    pub delay: Option<usize>,
    pub priority: Option<usize>,
    pub assignments: Vec<crate::syntax::ParsedEventAssignment>,
    pub span: crate::Span,
}

#[allow(clippy::type_complexity)]
pub(super) fn resolve(
    document: &mut ParsedDocument,
) -> Result<
    (
        BTreeMap<String, Entry>,
        Initials,
        Rules,
        Vec<ResolvedEvent>,
        BTreeSet<String>,
    ),
    Diagnostic,
> {
    let mut symbols = BTreeMap::new();
    let mut initials = Initials::new();
    let mut rules = Rules::new();
    let mut events = Vec::<ResolvedEvent>::new();
    let mut reactions = BTreeMap::<String, usize>::new();
    let mut next_reaction = 0;
    let mut next_event = 0;
    let mut next_seen = 0usize;
    let mut reserved_units = BTreeSet::new();
    for index in 0..document.statements.len() {
        let statement = document.statements[index].clone();
        match statement {
            Statement::UnitDefinition { name, factors } => {
                reserved_units.insert(name.text);
                reserved_units.extend(factors.into_iter().map(|f| f.name.text));
            }
            Statement::UnitAnnotation { name, unit } => {
                reserved_units.insert(unit.text);
                // Model-default annotations are not numeric symbols.
                if crate::model::units::ModelUnit::from_name(&name.text).is_none() {
                    merge(
                        document,
                        &mut symbols,
                        &name,
                        Symbol::Parameter,
                        None,
                        &rules,
                        &mut next_seen,
                    )?;
                }
            }
            Statement::Compartment {
                name,
                value,
                qualifier,
            } => {
                merge(
                    document,
                    &mut symbols,
                    &name,
                    Symbol::Compartment {
                        size: None,
                        implicit: false,
                    },
                    qualifier,
                    &rules,
                    &mut next_seen,
                )?;
                if let Some(value) = value {
                    assign(
                        document,
                        &mut symbols,
                        &mut initials,
                        &rules,
                        &mut reactions,
                        &name,
                        value,
                        &mut next_seen,
                    )?;
                }
            }
            Statement::Species {
                name,
                compartment,
                boundary,
                substance_only,
                qualifier,
            } => {
                merge(
                    document,
                    &mut symbols,
                    &name,
                    Symbol::Species {
                        compartment: compartment.as_ref().map(|n| n.text.clone()),
                        boundary,
                        substance_only: Some(substance_only),
                    },
                    qualifier,
                    &rules,
                    &mut next_seen,
                )?;
                if let Some(location) = compartment {
                    merge(
                        document,
                        &mut symbols,
                        &location,
                        Symbol::Compartment {
                            size: None,
                            implicit: false,
                        },
                        None,
                        &rules,
                        &mut next_seen,
                    )?;
                }
            }
            Statement::Parameter {
                name,
                value,
                qualifier,
            } => {
                merge(
                    document,
                    &mut symbols,
                    &name,
                    Symbol::Parameter,
                    qualifier,
                    &rules,
                    &mut next_seen,
                )?;
                if let Some(value) = value {
                    assign(
                        document,
                        &mut symbols,
                        &mut initials,
                        &rules,
                        &mut reactions,
                        &name,
                        value,
                        &mut next_seen,
                    )?;
                }
            }
            Statement::Initial { name, value } => {
                merge(
                    document,
                    &mut symbols,
                    &name,
                    Symbol::Parameter,
                    None,
                    &rules,
                    &mut next_seen,
                )?;
                assign(
                    document,
                    &mut symbols,
                    &mut initials,
                    &rules,
                    &mut reactions,
                    &name,
                    value,
                    &mut next_seen,
                )?;
            }
            Statement::Rule {
                kind,
                name,
                expression,
                lexical_scalar,
                qualifier,
                span,
            } => {
                if let Some(&rxn_index) = reactions.get(&name.text) {
                    if kind == crate::RuleKind::Rate || qualifier.is_some() {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            span,
                            "reaction rules are not supported as species/parameter rules",
                        ));
                    }
                    infer_expression(document, &mut symbols, expression, &rules, &mut next_seen)?;
                    if let Statement::Reaction { rate, .. } = &mut document.statements[rxn_index] {
                        *rate = expression;
                    }
                    continue;
                }
                merge(
                    document,
                    &mut symbols,
                    &name,
                    Symbol::Parameter,
                    qualifier,
                    &rules,
                    &mut next_seen,
                )?;
                infer_expression(document, &mut symbols, expression, &rules, &mut next_seen)?;
                if let Some(existing) = rules.get(&name.text)
                    && existing.kind != kind
                {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        span,
                        "conflicting assignment and rate rules on the same symbol",
                    ));
                }
                if kind == crate::RuleKind::Assignment {
                    check_transient_cycle(document, &initials, &rules, &name.text)?;
                    initials.remove(&name.text);
                }
                rules.insert(
                    name.text,
                    ResolvedRule {
                        kind,
                        expression,
                        lexical_scalar,
                        span,
                    },
                );
            }
            Statement::Reaction {
                mut name,
                reactants,
                products,
                rate,
                ..
            } => {
                // Named IDs are encountered before their participants, but anonymous
                // IDs are generated only after the complete reaction is parsed.
                if !name.text.is_empty() {
                    merge(
                        document,
                        &mut symbols,
                        &name,
                        Symbol::Reaction,
                        None,
                        &rules,
                        &mut next_seen,
                    )?;
                }
                for participant in reactants.iter().chain(&products) {
                    merge(
                        document,
                        &mut symbols,
                        &participant.name,
                        Symbol::Species {
                            compartment: None,
                            boundary: participant.boundary,
                            substance_only: None,
                        },
                        participant.boundary.then_some(true),
                        &rules,
                        &mut next_seen,
                    )?;
                }
                infer_expression(document, &mut symbols, rate, &rules, &mut next_seen)?;
                if name.text.is_empty() {
                    loop {
                        name.text = format!("_J{next_reaction}");
                        next_reaction += 1;
                        if !symbols.contains_key(&name.text) && !reserved_units.contains(&name.text)
                        {
                            break;
                        }
                    }
                    merge(
                        document,
                        &mut symbols,
                        &name,
                        Symbol::Reaction,
                        None,
                        &rules,
                        &mut next_seen,
                    )?;
                    if let Statement::Reaction { name: target, .. } =
                        &mut document.statements[index]
                    {
                        *target = name.clone();
                    }
                }
                initials.remove(&name.text);
                if rules
                    .get(&name.text)
                    .is_some_and(|r| r.kind == crate::RuleKind::Assignment)
                {
                    rules.remove(&name.text);
                }
                reactions.insert(name.text, index);
            }
            Statement::Event {
                name,
                trigger,
                initial_value,
                persistent,
                use_values_from_trigger_time,
                delay,
                priority,
                assignments,
                span,
                ..
            } => {
                if !is_boolean_expr(document, trigger) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        document.expressions[trigger].span,
                        "event trigger expression must evaluate to a boolean",
                    ));
                }
                if let Some(d) = delay {
                    if is_boolean_expr(document, d) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            document.expressions[d].span,
                            "event delay expression must evaluate to a numeric value",
                        ));
                    }
                    infer_expression(document, &mut symbols, d, &rules, &mut next_seen)?;
                }
                infer_expression(document, &mut symbols, trigger, &rules, &mut next_seen)?;
                if let Some(p) = priority {
                    if is_boolean_expr(document, p) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            document.expressions[p].span,
                            "event priority expression must evaluate to a numeric value",
                        ));
                    }
                    infer_expression(document, &mut symbols, p, &rules, &mut next_seen)?;
                }
                let mut seen_targets = BTreeSet::new();
                for a in &assignments {
                    if !seen_targets.insert(a.variable.text.clone()) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            a.span,
                            "duplicate event assignment target in the same event",
                        ));
                    }
                    if is_boolean_expr(document, a.expression) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            a.span,
                            "event assignment expression must evaluate to a numeric value",
                        ));
                    }
                    merge(
                        document,
                        &mut symbols,
                        &a.variable,
                        Symbol::Parameter,
                        None,
                        &rules,
                        &mut next_seen,
                    )?;
                    if let Some(entry) = symbols.get_mut(&a.variable.text)
                        && matches!(entry.symbol, Symbol::Parameter | Symbol::Compartment { .. })
                    {
                        entry.qualifier = Some(false);
                    }
                    infer_expression(document, &mut symbols, a.expression, &rules, &mut next_seen)?;
                }
                let id = if !name.text.is_empty() {
                    if symbols.contains_key(&name.text) || reserved_units.contains(&name.text) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            name.span,
                            "event identifier conflicts with a model symbol or unit definition",
                        ));
                    }
                    name.text
                } else {
                    loop {
                        let candidate = format!("_E{next_event}");
                        next_event += 1;
                        if !symbols.contains_key(&candidate)
                            && !reserved_units.contains(&candidate)
                            && !events.iter().any(|e| e.id == candidate)
                        {
                            break candidate;
                        }
                    }
                };
                let resolved_ev = ResolvedEvent {
                    id: id.clone(),
                    trigger,
                    initial_value,
                    persistent,
                    from_trigger: use_values_from_trigger_time,
                    delay,
                    priority,
                    assignments,
                    span,
                };
                if let Some(existing) = events.iter_mut().find(|e| e.id == id) {
                    *existing = resolved_ev;
                } else {
                    document
                        .limits
                        .checked_add(
                            crate::LimitKind::ModelEntities,
                            symbols.len() + document.functions.len() + events.len(),
                            1,
                        )
                        .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
                    events.push(resolved_ev);
                }
            }
            Statement::Alias { .. } | Statement::Submodel { .. } | Statement::Delete { .. } => {
                unreachable!("flattened before symbol resolution")
            }
        }
    }
    for func in &document.functions {
        if func.name.text == document.name {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                func.name.span,
                "function identifier conflicts with model name",
            ));
        }
        if symbols.contains_key(&func.name.text)
            || reserved_units.contains(&func.name.text)
            || events.iter().any(|e| e.id == func.name.text)
        {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                func.name.span,
                "function identifier conflicts with a model symbol or unit definition",
            ));
        }
    }
    let mut event_targets = BTreeSet::new();
    for ev in &events {
        if symbols.contains_key(&ev.id) || reserved_units.contains(&ev.id) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                ev.span,
                "event identifier conflicts with a model symbol or unit definition",
            ));
        }
        for a in &ev.assignments {
            let entry = &symbols[&a.variable.text];
            if matches!(entry.symbol, Symbol::Reaction) {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    a.span,
                    "reaction identifiers cannot be event assignment targets",
                ));
            }
            if matches!(entry.symbol, Symbol::Parameter | Symbol::Compartment { .. })
                && entry.qualifier == Some(true)
            {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    a.span,
                    "constant parameter or compartment cannot be an event assignment target",
                ));
            }
            if rules
                .get(&a.variable.text)
                .is_some_and(|r| r.kind == crate::RuleKind::Assignment)
            {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    a.span,
                    "assignment-rule target cannot also be an event assignment target",
                ));
            }
            event_targets.insert(a.variable.text.clone());
        }
    }
    for (name, rule) in &rules {
        match symbols[name].symbol {
            Symbol::Reaction => {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    rule.span,
                    "reaction rules are not supported as species/parameter rules",
                ));
            }
            Symbol::Compartment { .. } | Symbol::Species { .. } | Symbol::Parameter => (),
        }
    }
    for (name, entry) in &mut symbols {
        if let Symbol::Compartment { size, .. } = &mut entry.symbol {
            if let Some(rule) = rules
                .get(name)
                .filter(|r| r.kind == crate::RuleKind::Assignment)
            {
                if let Some(value) = rule.lexical_scalar {
                    if value <= 0.0 {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            rule.span,
                            "compartment size must be positive",
                        ));
                    }
                    *size = Some(value);
                } else {
                    *size = None;
                }
            } else if let Some((initial, span)) = initials.get(name) {
                match initial {
                    InitialValue::Scalar(value) if *value > 0.0 => *size = Some(*value),
                    InitialValue::Scalar(_) => {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            *span,
                            "compartment size must be positive",
                        ));
                    }
                    InitialValue::Expression(_) => {
                        *size = None;
                    }
                }
            }
        }
    }
    Ok((symbols, initials, rules, events, event_targets))
}

fn check_transient_cycle(
    document: &ParsedDocument,
    initials: &Initials,
    rules: &Rules,
    target: &str,
) -> Result<(), Diagnostic> {
    let algebraic_root = |sym: &str| -> Option<(usize, crate::Span)> {
        if let Some((InitialValue::Expression(root), span)) = initials.get(sym) {
            Some((*root, *span))
        } else if let Some(rule) = rules.get(sym)
            && rule.kind == crate::RuleKind::Assignment
        {
            Some((rule.expression, rule.span))
        } else {
            None
        }
    };
    let Some((start_root, start_span)) = algebraic_root(target) else {
        return Ok(());
    };
    let mut visited = BTreeSet::new();
    let mut queue = Vec::new();
    let mut expr_stack = vec![start_root];
    while let Some(id) = expr_stack.pop() {
        let kind = &document.expressions[id].kind;
        if let ExprKind::Symbol(sym) = kind {
            if sym.text == target {
                return Err(error(
                    DiagnosticCode::InitializationCycle,
                    start_span,
                    format!("initialization dependency cycle includes {target}"),
                ));
            }
            if visited.insert(sym.text.clone()) {
                queue.push(sym.text.clone());
            }
        }
        expr_stack.extend(kind.children());
    }
    while let Some(curr) = queue.pop() {
        if let Some((root, _)) = algebraic_root(&curr) {
            expr_stack.push(root);
            while let Some(id) = expr_stack.pop() {
                let kind = &document.expressions[id].kind;
                if let ExprKind::Symbol(sym) = kind {
                    if sym.text == target {
                        return Err(error(
                            DiagnosticCode::InitializationCycle,
                            start_span,
                            format!("initialization dependency cycle includes {target}"),
                        ));
                    }
                    if visited.insert(sym.text.clone()) {
                        queue.push(sym.text.clone());
                    }
                }
                expr_stack.extend(kind.children());
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assign(
    document: &mut ParsedDocument,
    symbols: &mut BTreeMap<String, Entry>,
    initials: &mut Initials,
    rules: &Rules,
    reactions: &mut BTreeMap<String, usize>,
    name: &Name,
    value: InitialValue,
    next_seen: &mut usize,
) -> Result<(), Diagnostic> {
    if let InitialValue::Expression(root) = value {
        infer_expression(document, symbols, root, rules, next_seen)?;
    }
    if rules
        .get(&name.text)
        .is_some_and(|r| r.kind == crate::RuleKind::Assignment)
    {
        return Err(error(
            DiagnosticCode::DeclarationConflict,
            name.span,
            "ordinary initialization is not permitted after an assignment rule",
        ));
    }
    if let Some(&index) = reactions.get(&name.text) {
        let root = match value {
            InitialValue::Expression(root) => root,
            InitialValue::Scalar(value) => {
                let depth = if value < 0.0 { 2 } else { 1 };
                document
                    .limits
                    .check(crate::LimitKind::ExpressionDepth, depth)
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, name.span, e.to_string()))?;
                let root = document.expressions.len();
                document.expressions.push(Expr {
                    kind: ExprKind::Number {
                        lexeme: value.abs().to_string(),
                        value: crate::syntax::numeric::expression_value(
                            &value.abs().to_string(),
                            value.abs(),
                        ),
                    },
                    span: name.span,
                    depth: 1,
                });
                if value < 0.0 {
                    document.expressions.push(Expr {
                        kind: ExprKind::Unary {
                            op: UnaryOp::Minus,
                            child: root,
                        },
                        span: name.span,
                        depth: 2,
                    });
                    root + 1
                } else {
                    root
                }
            }
        };
        if let Statement::Reaction { rate, .. } = &mut document.statements[index] {
            *rate = root;
        }
    } else {
        if !rules.is_empty() {
            check_transient_cycle(document, initials, rules, &name.text)?;
        }
        initials.insert(name.text.clone(), (value, name.span));
    }
    Ok(())
}

fn infer_expression(
    document: &ParsedDocument,
    symbols: &mut BTreeMap<String, Entry>,
    root: usize,
    rules: &Rules,
    next_seen: &mut usize,
) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        let kind = &document.expressions[index].kind;
        if let ExprKind::Symbol(name) = kind {
            merge(
                document,
                symbols,
                name,
                Symbol::Parameter,
                None,
                rules,
                next_seen,
            )?;
        }
        pending.extend(kind.children().rev());
    }
    Ok(())
}

fn merge(
    document: &ParsedDocument,
    symbols: &mut BTreeMap<String, Entry>,
    name: &Name,
    incoming: Symbol,
    qualifier: Option<bool>,
    rules: &Rules,
    next_seen: &mut usize,
) -> Result<(), Diagnostic> {
    if let Some(entry) = symbols.get_mut(&name.text) {
        let promoted_ruled_species = matches!(entry.symbol, Symbol::Parameter)
            && matches!(incoming, Symbol::Species { .. })
            && rules.contains_key(&name.text);
        match (&mut entry.symbol, incoming) {
            (_, Symbol::Parameter) => (),
            (old @ Symbol::Parameter, role) => *old = role,
            (
                Symbol::Species {
                    compartment,
                    substance_only,
                    ..
                },
                Symbol::Species {
                    compartment: new,
                    substance_only: new_so,
                    ..
                },
            ) => {
                if new.is_some() {
                    *compartment = new;
                }
                if new_so.is_some() {
                    *substance_only = new_so;
                }
            }
            (Symbol::Compartment { .. }, Symbol::Compartment { .. })
            | (Symbol::Reaction, Symbol::Reaction) => (),
            _ => {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    name.span,
                    "incompatible symbol roles",
                )
                .with_label(Label::secondary(entry.span, "first occurrence")));
            }
        }
        if promoted_ruled_species {
            entry.qualifier = Some(true);
        } else if let Some(value) = qualifier {
            entry.qualifier = Some(value);
        }
        if let Symbol::Species { boundary, .. } = &mut entry.symbol {
            *boundary = entry.qualifier.unwrap_or(*boundary);
        }
    } else {
        declare(document, symbols, name, incoming, next_seen)?;
        let entry = symbols.get_mut(&name.text).unwrap();
        entry.qualifier = qualifier;
    }
    Ok(())
}

fn is_boolean_expr(document: &ParsedDocument, root: usize) -> bool {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        match &document.expressions[id].kind {
            ExprKind::Boolean(_)
            | ExprKind::Not { .. }
            | ExprKind::Compare { .. }
            | ExprKind::Logical { .. } => {}
            ExprKind::Piecewise {
                branches,
                otherwise,
            } => {
                if branches.is_empty() && otherwise.is_none() {
                    return false;
                }
                for (val, _) in branches {
                    stack.push(*val);
                }
                if let Some(other) = otherwise {
                    stack.push(*other);
                }
            }
            ExprKind::UserCall { name, .. } => {
                if let Some(func) = document.functions.iter().find(|f| f.name.text == name.text) {
                    stack.push(func.body);
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
