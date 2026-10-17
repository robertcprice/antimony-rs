//! Conservative explicit unit handling, independent of numeric initialization.
use super::{Entry, Symbol};
use crate::model::RuleKind;
use crate::model::units::*;
use crate::syntax::{
    BinaryOp, ExprKind, InitialValue, ParsedDocument, Statement, UnitFactor, error,
};
use crate::{Diagnostic, DiagnosticCode, LimitKind, Span};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct UnitAnalysis {
    pub definitions: Vec<UnitDefinition>,
    pub annotations: BTreeMap<String, String>,
    pub symbols: BTreeMap<String, DimensionalStatus>,
    pub expressions: Vec<DimensionalStatus>,
    pub model_units: BTreeMap<ModelUnit, String>,
    pub species_units: BTreeMap<String, String>,
}
pub(super) const ZERO: Dimensions = Dimensions {
    substance: 0,
    time: 0,
    length: 0,
};

pub(super) fn analyze(
    document: &ParsedDocument,
    symbols: &BTreeMap<String, Entry>,
    initials: &super::initialization::Initials,
    rules: &super::symbols::Rules,
    events: &[super::symbols::ResolvedEvent],
) -> Result<UnitAnalysis, Diagnostic> {
    let mut specs = BTreeMap::<String, (&[UnitFactor], Span)>::new();
    let mut first_seen = BTreeMap::<String, usize>::new();
    for statement in &document.statements {
        match statement {
            Statement::UnitDefinition { name, factors } => {
                if UnitKind::from_name(&name.text).is_some() {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        name.span,
                        "redefinition of builtin units is deferred",
                    ));
                }
                if name.text.ends_with('s') {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        name.span,
                        "custom unit identifiers ending in s are deferred because upstream singularizes them",
                    ));
                }
                if specs.contains_key(&name.text) || symbols.contains_key(&name.text) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        name.span,
                        "duplicate unit definition or unit/model-symbol namespace conflict",
                    ));
                }
                document
                    .limits
                    .checked_add(
                        LimitKind::ModelEntities,
                        symbols.len() + document.functions.len(),
                        specs.len() + 1,
                    )
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, name.span, e.to_string()))?;
                specs.insert(name.text.clone(), (factors, name.span));
                first_seen
                    .entry(name.text.clone())
                    .or_insert(name.span.start());
                for factor in factors {
                    first_seen
                        .entry(factor.name.text.clone())
                        .or_insert(factor.name.span.start());
                }
            }
            Statement::UnitAnnotation { unit, .. } => {
                first_seen
                    .entry(unit.text.clone())
                    .or_insert(unit.span.start());
            }
            _ => (),
        }
    }
    let mut pending = BTreeMap::<String, usize>::new();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    let mut ready = BTreeSet::new();
    for (name, (factors, _)) in &specs {
        let mut dependencies = BTreeSet::new();
        for factor in *factors {
            if symbols.contains_key(&factor.name.text) {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    factor.name.span,
                    "numeric model symbols cannot also be unit factors",
                ));
            }
            if UnitKind::from_name(&factor.name.text).is_some() {
                continue;
            }
            if !specs.contains_key(&factor.name.text) {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    factor.name.span,
                    "unit name is undefined or outside the supported builtin catalog; never assumed dimensionless",
                ));
            }
            dependencies.insert(factor.name.text.clone());
        }
        for dep in &dependencies {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(name.clone());
        }
        if dependencies.is_empty() {
            ready.insert(name.clone());
        }
        pending.insert(name.clone(), dependencies.len());
    }
    let mut definitions = BTreeMap::<String, UnitDefinition>::new();
    while let Some(name) = ready.pop_first() {
        let (factors, span) = specs[&name];
        let mut components = Vec::new();
        let mut kinds = BTreeSet::new();
        for factor in factors {
            let expanded = if let Some(kind) = UnitKind::from_name(&factor.name.text) {
                let (multiplier, scale) = if let Some(coefficient) = &factor.coefficient {
                    coefficient_parts(coefficient, factor.name.span)?
                } else {
                    (1.0, 0)
                };
                vec![UnitComponent {
                    kind,
                    exponent: 1,
                    scale,
                    multiplier,
                }]
            } else {
                if factor.coefficient.is_some() {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        factor.name.span,
                        "scalar coefficients on unit aliases are deferred",
                    ));
                }
                definitions[&factor.name.text].components.clone()
            };
            for mut component in expanded {
                component.exponent = component
                    .exponent
                    .checked_mul(factor.exponent)
                    .filter(|n| n.abs() <= UnitLimits::RESOLVED_EXPONENT)
                    .ok_or_else(|| {
                        error(
                            DiagnosticCode::ResourceLimit,
                            factor.name.span,
                            "expanded unit exponent exceeds magnitude 64",
                        )
                    })?;
                // Checking before insertion bounds expansion, including branching aliases.
                if components.len() == UnitLimits::COMPONENTS {
                    return Err(error(
                        DiagnosticCode::ResourceLimit,
                        span,
                        "expanded unit has more than five components",
                    ));
                }
                if !kinds.insert(component.kind) {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        factor.name.span,
                        "repeated primitive unit kinds are deferred; no algebraic merging or cancellation is performed",
                    ));
                }
                components.push(component);
            }
        }
        definitions.insert(
            name.clone(),
            UnitDefinition {
                id: name.clone(),
                components,
            },
        );
        if let Some(users) = dependents.get(&name) {
            for user in users {
                let count = pending.get_mut(user).expect("unit dependent");
                *count -= 1;
                if *count == 0 {
                    ready.insert(user.clone());
                }
            }
        }
    }
    if definitions.len() != specs.len() {
        let (name, _) = pending
            .iter()
            .find(|(_, n)| **n > 0)
            .expect("unresolved unit dependency");
        return Err(error(
            DiagnosticCode::DeclarationConflict,
            specs[name].1,
            "unit definitions contain a dependency cycle",
        ));
    }
    let mut annotations = BTreeMap::new();
    let mut symbol_dimensions = symbols
        .keys()
        .map(|name| (name.clone(), DimensionalStatus::Unspecified))
        .collect::<BTreeMap<_, _>>();
    for statement in &document.statements {
        if let Statement::UnitAnnotation { name, unit } = statement {
            let Some(entry) = symbols.get(&name.text) else {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    name.span,
                    "unit annotation target is undefined/uninitialized; use unit declarations for model defaults; unit-definition annotations are deferred",
                ));
            };
            if let Symbol::Reaction = entry.symbol {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    name.span,
                    "reaction unit annotations are deferred",
                ));
            }
            if annotations.contains_key(&name.text) {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    name.span,
                    "repeated unit annotation is rejected by this strict slice",
                ));
            }
            if symbols.contains_key(&unit.text) {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    unit.span,
                    "numeric model symbol cannot also name a unit",
                ));
            }
            let (canonical, dimension) = if let Some(kind) = UnitKind::from_name(&unit.text) {
                (kind.as_str().to_owned(), primitive_dimension(kind))
            } else if let Some(definition) = definitions.get(&unit.text) {
                (unit.text.clone(), definition_dimension(definition))
            } else {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    unit.span,
                    "unit name is undefined or outside the supported builtin catalog; never assumed dimensionless",
                ));
            };
            if matches!(entry.symbol, Symbol::Compartment { .. })
                && dimension != (Dimensions { length: 3, ..ZERO })
                && dimension != ZERO
            {
                return Err(error(
                    DiagnosticCode::UnitsInconsistency,
                    unit.span,
                    "three-dimensional compartment units must represent volume or explicit dimensionless units",
                ));
            }
            annotations.insert(name.text.clone(), canonical);
            symbol_dimensions.insert(name.text.clone(), DimensionalStatus::Known(dimension));
        }
    }
    let (mapping, generated) = super::unit_mapping::complete(
        document,
        symbols,
        &first_seen,
        &mut definitions,
        &mut annotations,
        &mut symbol_dimensions,
    )?;
    let time_dimensions =
        mapping
            .model_units
            .get(&ModelUnit::Time)
            .map_or(DimensionalStatus::Unspecified, |unit| {
                DimensionalStatus::Known(UnitKind::from_name(unit).map_or_else(
                    || definition_dimension(&definitions[unit]),
                    primitive_dimension,
                ))
            });
    let expression_dimensions = check_expressions(document, &symbol_dimensions, time_dimensions)?;
    for (name, (initial, span)) in initials {
        if let InitialValue::Expression(root) = initial
            && let (DimensionalStatus::Known(target), DimensionalStatus::Known(value)) =
                (symbol_dimensions[name], expression_dimensions[*root])
            && target != value
        {
            return Err(error(
                DiagnosticCode::UnitsInconsistency,
                *span,
                "initialization expression dimensions conflict with the explicitly annotated target",
            ));
        }
    }
    for (name, rule) in rules {
        if let (DimensionalStatus::Known(target), DimensionalStatus::Known(value)) = (
            symbol_dimensions[name],
            expression_dimensions[rule.expression],
        ) {
            match rule.kind {
                RuleKind::Assignment => {
                    if target != value {
                        return Err(error(
                            DiagnosticCode::UnitsInconsistency,
                            rule.span,
                            "assignment rule expression dimensions conflict with the explicitly annotated target",
                        ));
                    }
                }
                RuleKind::Rate => {
                    if let DimensionalStatus::Known(time) = time_dimensions {
                        let expected = Dimensions {
                            substance: target.substance - time.substance,
                            time: target.time - time.time,
                            length: target.length - time.length,
                        };
                        if value != expected {
                            return Err(error(
                                DiagnosticCode::UnitsInconsistency,
                                rule.span,
                                "rate rule expression dimensions conflict with target per model time unit",
                            ));
                        }
                    }
                }
            }
        }
    }
    for ev in events {
        boolean_units(expression_dimensions[ev.trigger], ev.span)?;
        if let Some(delay) = ev.delay
            && let (DimensionalStatus::Known(time), DimensionalStatus::Known(d)) =
                (time_dimensions, expression_dimensions[delay])
            && time != d
        {
            return Err(error(
                DiagnosticCode::UnitsInconsistency,
                ev.span,
                "event delay dimensions conflict with model time unit",
            ));
        }
        if let Some(priority) = ev.priority
            && matches!(expression_dimensions[priority], DimensionalStatus::Known(d) if d != ZERO)
        {
            return Err(error(
                DiagnosticCode::UnitsInconsistency,
                ev.span,
                "event priority requires dimensionless units",
            ));
        }
        for a in &ev.assignments {
            if let (DimensionalStatus::Known(target), DimensionalStatus::Known(value)) = (
                symbol_dimensions[&a.variable.text],
                expression_dimensions[a.expression],
            ) && target != value
            {
                return Err(error(
                    DiagnosticCode::UnitsInconsistency,
                    a.span,
                    "event assignment expression dimensions conflict with the explicitly annotated target",
                ));
            }
        }
    }
    let mut definitions = definitions.into_values().collect::<Vec<_>>();
    let generated_order = generated
        .iter()
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect::<BTreeMap<_, _>>();
    definitions.sort_by_key(|d| {
        if let Some(i) = generated_order.get(&d.id) {
            (1, *i)
        } else {
            (0, first_seen[&d.id])
        }
    });
    Ok(UnitAnalysis {
        definitions,
        annotations,
        symbols: symbol_dimensions,
        expressions: expression_dimensions,
        model_units: mapping.model_units,
        species_units: mapping.species_units,
    })
}

pub(super) fn coefficient_parts(text: &str, span: Span) -> Result<(f64, i32), Diagnostic> {
    let unsupported = || {
        error(
            DiagnosticCode::UnsupportedFeature,
            span,
            "unit coefficient requires a positive finite decimal with at most 15 significant digits and normalized scale [-12,12]",
        )
    };
    let (mantissa, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
    let exponent = exponent.parse::<i32>().map_err(|_| unsupported())?;
    let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits = mantissa
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let leading = digits.len() - digits.trim_start_matches('0').len();
    let significant = &digits[leading..];
    if significant.is_empty() || significant.len() > UnitLimits::COEFFICIENT_DIGITS {
        return Err(unsupported());
    }
    let scale = exponent
        .checked_add(decimal)
        .and_then(|n| n.checked_sub(leading as i32 + 1))
        .filter(|n| (UnitLimits::MIN_SCALE..=UnitLimits::MAX_SCALE).contains(n))
        .ok_or_else(unsupported)?;
    // Decimal-string normalization avoids rounding by floating-point division.
    let normalized = format!("{}.{}", &significant[..1], &significant[1..]);
    let multiplier = normalized.parse::<f64>().map_err(|_| unsupported())?;
    Ok((multiplier, scale))
}
pub(super) fn primitive_dimension(kind: UnitKind) -> Dimensions {
    match kind {
        UnitKind::Mole => Dimensions {
            substance: 1,
            ..ZERO
        },
        UnitKind::Second => Dimensions { time: 1, ..ZERO },
        UnitKind::Litre => Dimensions { length: 3, ..ZERO },
        UnitKind::Metre => Dimensions { length: 1, ..ZERO },
        UnitKind::Dimensionless => ZERO,
    }
}
pub(super) fn definition_dimension(definition: &UnitDefinition) -> Dimensions {
    let mut dimension = ZERO;
    for c in &definition.components {
        let primitive = primitive_dimension(c.kind);
        dimension.substance += primitive.substance * c.exponent;
        dimension.time += primitive.time * c.exponent;
        dimension.length += primitive.length * c.exponent;
    }
    dimension
}
fn check_expressions(
    document: &ParsedDocument,
    symbols: &BTreeMap<String, DimensionalStatus>,
    time_dimensions: DimensionalStatus,
) -> Result<Vec<DimensionalStatus>, Diagnostic> {
    let fn_nodes = document.function_body_nodes();
    let mut dimensions = Vec::with_capacity(document.expressions.len());
    for (id, expression) in document.expressions.iter().enumerate() {
        let state = if let ExprKind::UserCall { name, args } = &expression.kind {
            let arg_states = args.iter().map(|i| dimensions[*i]).collect::<Vec<_>>();
            user_call_dimensions(
                document,
                &name.text,
                &arg_states,
                time_dimensions,
                expression.span,
            )?
        } else {
            let child_dims = expression
                .kind
                .children()
                .map(|i| dimensions[i])
                .collect::<Vec<_>>();
            node_dimension(
                &expression.kind,
                &child_dims,
                fn_nodes[id],
                None,
                symbols,
                time_dimensions,
                expression.span,
            )?
        };
        dimensions.push(state);
    }
    Ok(dimensions)
}

fn user_call_dimensions(
    document: &ParsedDocument,
    fn_name: &str,
    arg_states: &[DimensionalStatus],
    time_dimensions: DimensionalStatus,
    call_span: Span,
) -> Result<DimensionalStatus, Diagnostic> {
    enum DimTask {
        Visit(usize),
        Apply(usize),
        PopFrame,
    }
    let func = document
        .functions
        .iter()
        .find(|f| f.name.text == fn_name)
        .expect("validated user function");
    let mut initial_frame = BTreeMap::new();
    for (param, state) in func.parameters.iter().zip(arg_states) {
        initial_frame.insert(param.text.clone(), *state);
    }
    let mut frames = vec![initial_frame];
    let mut pending = vec![DimTask::Visit(func.body)];
    let mut operands = Vec::<DimensionalStatus>::new();
    let empty_symbols = BTreeMap::new();
    while let Some(task) = pending.pop() {
        match task {
            DimTask::PopFrame => {
                frames.pop();
            }
            DimTask::Visit(id) => {
                pending.push(DimTask::Apply(id));
                pending.extend(
                    document.expressions[id]
                        .kind
                        .children()
                        .rev()
                        .map(DimTask::Visit),
                );
            }
            DimTask::Apply(id) => {
                let expr = &document.expressions[id];
                if let ExprKind::UserCall { name, args } = &expr.kind {
                    let start = operands.len() - args.len();
                    let callee = document
                        .functions
                        .iter()
                        .find(|f| f.name.text == name.text)
                        .expect("validated user function");
                    let mut frame = BTreeMap::new();
                    for (param, state) in callee.parameters.iter().zip(&operands[start..]) {
                        frame.insert(param.text.clone(), *state);
                    }
                    operands.truncate(start);
                    frames.push(frame);
                    pending.push(DimTask::PopFrame);
                    pending.push(DimTask::Visit(callee.body));
                    continue;
                }
                let arity = expr.kind.children().count();
                let start = operands.len() - arity;
                let state = node_dimension(
                    &expr.kind,
                    &operands[start..],
                    false,
                    frames.last(),
                    &empty_symbols,
                    time_dimensions,
                    call_span,
                )?;
                operands.truncate(start);
                operands.push(state);
            }
        }
    }
    debug_assert_eq!(operands.len(), 1);
    Ok(operands.pop().expect("user call dimension"))
}

fn node_dimension(
    kind: &ExprKind,
    child_dims: &[DimensionalStatus],
    uncalled_fn_body: bool,
    frame: Option<&BTreeMap<String, DimensionalStatus>>,
    symbols: &BTreeMap<String, DimensionalStatus>,
    time_dimensions: DimensionalStatus,
    span: Span,
) -> Result<DimensionalStatus, Diagnostic> {
    use DimensionalStatus::{Indeterminate, Known, Unspecified};
    Ok(match kind {
        ExprKind::Boolean(_) => Known(ZERO),
        ExprKind::Not { .. } => {
            boolean_units(child_dims[0], span)?;
            Known(ZERO)
        }
        ExprKind::Logical { .. } => {
            for &d in child_dims {
                boolean_units(d, span)?;
            }
            Known(ZERO)
        }
        ExprKind::Compare { .. } => {
            compatible_dimensions(child_dims.iter().copied(), span)?;
            Known(ZERO)
        }
        ExprKind::Piecewise {
            branches,
            otherwise,
        } => {
            for i in 0..branches.len() {
                boolean_units(child_dims[2 * i + 1], span)?;
            }
            compatible_dimensions(
                (0..branches.len())
                    .map(|i| child_dims[2 * i])
                    .chain(otherwise.iter().map(|_| child_dims[2 * branches.len()])),
                span,
            )?
        }
        ExprKind::Number { .. } | ExprKind::Rational { .. } => Unspecified,
        ExprKind::Constant(_) => Known(ZERO),
        ExprKind::Time => time_dimensions,
        ExprKind::Call { function, .. } => builtin_dimensions(*function, child_dims, span)?,
        ExprKind::UserCall { .. } => unreachable!("handled by caller"),
        ExprKind::Symbol(name) => {
            if uncalled_fn_body {
                Unspecified
            } else if let Some(frame) = frame {
                frame[&name.text]
            } else {
                symbols[&name.text]
            }
        }
        ExprKind::Unary { .. } => child_dims[0],
        ExprKind::Binary { op, .. } => {
            let left = child_dims[0];
            let right = child_dims[1];
            match (op, left, right) {
                (BinaryOp::Add | BinaryOp::Subtract, Known(a), Known(b)) => {
                    if a != b {
                        return Err(error(
                            DiagnosticCode::UnitsInconsistency,
                            span,
                            "addition/subtraction combines provably incompatible dimensions",
                        ));
                    }
                    Known(a)
                }
                (BinaryOp::Multiply | BinaryOp::Divide, Known(a), Known(b)) => {
                    let sign = if *op == BinaryOp::Divide { -1 } else { 1 };
                    let combine = |left: i32, right: i32| {
                        right
                            .checked_mul(sign)
                            .and_then(|right| left.checked_add(right))
                            .ok_or_else(|| {
                                error(
                                    DiagnosticCode::ResourceLimit,
                                    span,
                                    "dimensional exponent arithmetic overflow",
                                )
                            })
                    };
                    Known(Dimensions {
                        substance: combine(a.substance, b.substance)?,
                        time: combine(a.time, b.time)?,
                        length: combine(a.length, b.length)?,
                    })
                }
                (BinaryOp::Power, _, Known(b)) if b != ZERO => {
                    return Err(error(
                        DiagnosticCode::UnitsInconsistency,
                        span,
                        "power exponent has provably non-dimensionless units",
                    ));
                }
                (BinaryOp::Power, Known(a), Known(b)) if a == ZERO && b == ZERO => Known(ZERO),
                _ => Indeterminate,
            }
        }
    })
}

fn builtin_dimensions(
    function: crate::Builtin,
    states: &[DimensionalStatus],
    span: Span,
) -> Result<DimensionalStatus, Diagnostic> {
    use crate::builtins::UnitBehavior;
    use DimensionalStatus::{Indeterminate, Known};
    let mismatch = || {
        error(
            DiagnosticCode::UnitsInconsistency,
            span,
            "builtin combines provably incompatible dimensions",
        )
    };
    Ok(match function.unit_behavior() {
        UnitBehavior::Preserve => states[0],
        UnitBehavior::Dimensionless => {
            if states.iter().any(|s| matches!(s, Known(d) if *d != ZERO)) {
                return Err(mismatch());
            }
            if states.iter().all(|s| *s == Known(ZERO)) {
                Known(ZERO)
            } else {
                Indeterminate
            }
        }
        UnitBehavior::Compatible => {
            let known = states
                .iter()
                .filter_map(|s| if let Known(d) = s { Some(*d) } else { None })
                .collect::<Vec<_>>();
            if known.windows(2).any(|pair| pair[0] != pair[1]) {
                return Err(mismatch());
            }
            if known.len() == states.len() {
                Known(known[0])
            } else {
                Indeterminate
            }
        }
        UnitBehavior::Root => match states[0] {
            Known(d) if d.substance % 2 == 0 && d.time % 2 == 0 && d.length % 2 == 0 => {
                Known(Dimensions {
                    substance: d.substance / 2,
                    time: d.time / 2,
                    length: d.length / 2,
                })
            }
            _ => Indeterminate,
        },
        UnitBehavior::Power => {
            if matches!(states[1], Known(d) if d != ZERO) {
                return Err(mismatch());
            }
            if states[0] == Known(ZERO) && states[1] == Known(ZERO) {
                Known(ZERO)
            } else {
                Indeterminate
            }
        }
    })
}

fn boolean_units(state: DimensionalStatus, span: crate::Span) -> Result<(), Diagnostic> {
    if matches!(state, DimensionalStatus::Known(d) if d != ZERO) {
        return Err(error(
            DiagnosticCode::UnitsInconsistency,
            span,
            "Boolean context requires dimensionless known operands",
        ));
    }
    Ok(())
}
fn compatible_dimensions(
    states: impl Iterator<Item = DimensionalStatus>,
    span: crate::Span,
) -> Result<DimensionalStatus, Diagnostic> {
    let mut known = None;
    let mut all_known = true;
    let mut all_unspecified = true;
    for state in states {
        all_unspecified &= state == DimensionalStatus::Unspecified;
        if let DimensionalStatus::Known(d) = state {
            if known.is_some_and(|k| k != d) {
                return Err(error(
                    DiagnosticCode::UnitsInconsistency,
                    span,
                    "conditional operands or alternatives have provably incompatible dimensions",
                ));
            }
            known = Some(d);
        } else {
            all_known = false;
        }
    }
    Ok(if all_known {
        known.map_or(DimensionalStatus::Unspecified, DimensionalStatus::Known)
    } else if all_unspecified {
        DimensionalStatus::Unspecified
    } else {
        DimensionalStatus::Indeterminate
    })
}
