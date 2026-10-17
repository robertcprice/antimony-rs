//! Resolution for the bounded modular, event, rule, function, and units subset.
mod evaluation;
mod expression_normalization;
mod initialization;
mod modules;
mod symbols;
mod unit_mapping;
mod units;
use crate::model::*;
use crate::syntax::{ExprKind, InitialValue, Name, ParsedDocument, Participant, Statement, error};
use crate::{Diagnostic, DiagnosticCode, Label, LimitKind, Severity, Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
enum Symbol {
    Compartment {
        size: Option<f64>,
        implicit: bool,
    },
    Species {
        compartment: Option<String>,
        boundary: bool,
        substance_only: Option<bool>,
    },
    Parameter,
    Reaction,
}
struct Entry {
    symbol: Symbol,
    span: Span,
    qualifier: Option<bool>,
    first_seen: usize,
}

pub(crate) fn analyze_document(document: &ParsedDocument) -> Result<ValidatedModel, Diagnostic> {
    let flattened = modules::flatten(document)?;
    let mut normalized = expression_normalization::normalize(&flattened)?;
    let (mut symbols, initials, rules, resolved_events, event_targets) =
        symbols::resolve(&mut normalized)?;
    let document = &normalized;
    let fn_nodes = document.function_body_nodes();
    for (id, expression) in document.expressions.iter().enumerate() {
        if !fn_nodes[id]
            && let ExprKind::Symbol(name) = &expression.kind
            && matches!(symbols[&name.text].symbol, Symbol::Reaction)
        {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                name.span,
                "references to reaction rates remain outside this slice",
            ));
        }
    }
    let needs_default = symbols.values().any(|e| {
        matches!(
            e.symbol,
            Symbol::Species {
                compartment: None,
                ..
            }
        )
    });
    if needs_default {
        if symbols.contains_key("default_compartment") {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                symbols["default_compartment"].span,
                "explicit default_compartment collides with an implicit compartment; provide explicit species compartments",
            ));
        }
        let name = Name {
            text: "default_compartment".into(),
            span: document.source.span(0, 0).expect("empty source span"),
        };
        let mut default_seen = symbols.len();
        declare(
            document,
            &mut symbols,
            &name,
            Symbol::Compartment {
                size: Some(1.0),
                implicit: true,
            },
            &mut default_seen,
        )?;
    }
    let resolved = initialization::resolve(document, &initials, &rules, &event_targets, &symbols)?;
    let unit_analysis = units::analyze(document, &symbols, &initials, &rules, &resolved_events)?;
    let mut initial_assignments = initials
        .iter()
        .filter_map(|(name, (value, _))| {
            if let InitialValue::Expression(root) = value {
                Some(InitialAssignment {
                    symbol: name.clone(),
                    expression: ExpressionId(*root),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    // Antimony groups initial assignments and rules by species, then compartments,
    // then parameters, ordered by first appearance within each class.
    let symbol_rank = |sym: &str| match symbols[sym].symbol {
        Symbol::Species { .. } => 0usize,
        Symbol::Compartment { .. } => 1usize,
        Symbol::Parameter => 2usize,
        Symbol::Reaction => 3usize,
    };
    initial_assignments.sort_by_key(|a| (symbol_rank(&a.symbol), symbols[&a.symbol].first_seen));
    let mut model_rules = rules
        .iter()
        .map(|(name, rule)| Rule {
            kind: rule.kind,
            target: name.clone(),
            expression: ExpressionId(rule.expression),
            span: rule.span,
        })
        .collect::<Vec<_>>();
    model_rules.sort_by_key(|r| (symbol_rank(&r.target), symbols[&r.target].first_seen));
    let mut compartments = Vec::new();
    let mut species = Vec::new();
    let mut parameters = Vec::new();
    for (name, entry) in &symbols {
        match &entry.symbol {
            Symbol::Compartment { size, implicit } => {
                let resolved_size = resolved.get(name).and_then(|r| r.value).or(*size);
                if resolved_size.is_some_and(|v| v <= 0.0) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        entry.span,
                        "compartment size must be positive",
                    ));
                }
                compartments.push(Compartment {
                    id: name.clone(),
                    size: resolved_size,
                    initial_expression: expression_initial(&initials, name),
                    implicit: *implicit,
                    constant: resolved[name].constant,
                    span: entry.span,
                });
            }
            Symbol::Species {
                compartment,
                boundary,
                substance_only,
            } => {
                let compartment = compartment.as_deref().unwrap_or("default_compartment");
                if !matches!(
                    symbols.get(compartment).map(|e| &e.symbol),
                    Some(Symbol::Compartment { .. })
                ) {
                    return Err(error(
                        DiagnosticCode::UnresolvedSymbol,
                        entry.span,
                        format!(
                            "species compartment {compartment:?} is not a declared compartment"
                        ),
                    ));
                }
                let has_only_substance_units = substance_only.unwrap_or(false);
                let init_val = resolved.get(name).and_then(|r| r.value);
                if init_val.is_some_and(|v| v < 0.0) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        entry.span,
                        "initial species concentration must be nonnegative",
                    ));
                }
                let (concentration, initial_amount) = if has_only_substance_units {
                    (None, init_val)
                } else {
                    (init_val, None)
                };
                species.push(Species {
                    id: name.clone(),
                    compartment: compartment.to_owned(),
                    concentration,
                    initial_amount,
                    initial_expression: expression_initial(&initials, name),
                    boundary: *boundary,
                    has_only_substance_units,
                    span: entry.span,
                });
            }
            Symbol::Parameter => parameters.push(Parameter {
                id: name.clone(),
                value: resolved.get(name).and_then(|r| r.value),
                span: entry.span,
                initial_expression: expression_initial(&initials, name),
                constant: resolved[name].constant,
            }),
            Symbol::Reaction => (),
        }
    }
    let expressions = document
        .expressions
        .iter()
        .enumerate()
        .map(|(id, expr)| {
            let kind = match &expr.kind {
                ExprKind::Boolean(value) => ExpressionKind::Boolean(*value),
                ExprKind::Not { child } => ExpressionKind::Not {
                    child: ExpressionId(*child),
                },
                ExprKind::Compare { op, args, .. } => ExpressionKind::Compare {
                    op: *op,
                    args: args.iter().map(|id| ExpressionId(*id)).collect(),
                },
                ExprKind::Logical { op, args, .. } => ExpressionKind::Logical {
                    op: *op,
                    args: args.iter().map(|id| ExpressionId(*id)).collect(),
                },
                ExprKind::Piecewise {
                    branches,
                    otherwise,
                } => ExpressionKind::Piecewise {
                    branches: branches
                        .iter()
                        .map(|(v, c)| PiecewiseBranch {
                            value: ExpressionId(*v),
                            condition: ExpressionId(*c),
                        })
                        .collect(),
                    otherwise: otherwise.map(ExpressionId),
                },
                ExprKind::Call { function, args } => ExpressionKind::Call {
                    function: *function,
                    args: args.iter().map(|id| ExpressionId(*id)).collect(),
                },
                ExprKind::UserCall { name, args } => ExpressionKind::UserCall {
                    name: name.text.clone(),
                    args: args.iter().map(|id| ExpressionId(*id)).collect(),
                },
                ExprKind::Rational {
                    numerator,
                    denominator,
                } => ExpressionKind::Rational {
                    numerator: *numerator,
                    denominator: *denominator,
                },
                ExprKind::Time => ExpressionKind::Time,
                ExprKind::Constant(constant) => ExpressionKind::Constant(*constant),
                ExprKind::Number { lexeme, value } => ExpressionKind::Number {
                    lexeme: lexeme.clone(),
                    value: *value,
                },
                ExprKind::Symbol(name) => ExpressionKind::Symbol {
                    name: name.text.clone(),
                    kind: if fn_nodes[id] {
                        SymbolKind::FunctionParameter
                    } else {
                        match symbols[&name.text].symbol {
                            Symbol::Species { .. } => SymbolKind::Species,
                            Symbol::Compartment { .. } => SymbolKind::Compartment,
                            Symbol::Parameter => SymbolKind::Parameter,
                            Symbol::Reaction => unreachable!("reaction references rejected above"),
                        }
                    },
                },
                ExprKind::Unary { op, child } => ExpressionKind::Unary {
                    op: *op,
                    child: ExpressionId(*child),
                },
                ExprKind::Binary { op, left, right } => ExpressionKind::Binary {
                    op: *op,
                    left: ExpressionId(*left),
                    right: ExpressionId(*right),
                },
            };
            Expression {
                kind,
                span: expr.span,
            }
        })
        .collect::<Vec<_>>();
    let mut reactions = BTreeMap::new();
    for statement in &document.statements {
        if let Statement::Reaction {
            name,
            reactants,
            products,
            reversible,
            rate,
            ..
        } = statement
        {
            if reactants.is_empty() && products.is_empty() {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    name.span,
                    "reactions with neither reactants nor products are outside this slice",
                ));
            }
            let reactants = references(reactants)?;
            let products = references(products)?;
            let mut modifiers = BTreeSet::new();
            let mut pending = vec![ExpressionId(*rate)];
            while let Some(id) = pending.pop() {
                match &expressions[id.0].kind {
                    ExpressionKind::Symbol {
                        name,
                        kind: SymbolKind::Species,
                    } if !reactants
                        .iter()
                        .chain(&products)
                        .any(|r| r.species == *name) =>
                    {
                        modifiers.insert(name.clone());
                    }
                    ExpressionKind::Call { args, .. }
                    | ExpressionKind::UserCall { args, .. }
                    | ExpressionKind::Compare { args, .. }
                    | ExpressionKind::Logical { args, .. } => pending.extend(args),
                    ExpressionKind::Not { child } => pending.push(*child),
                    ExpressionKind::Piecewise {
                        branches,
                        otherwise,
                    } => {
                        pending.extend(branches.iter().flat_map(|b| [b.value, b.condition]));
                        pending.extend(otherwise);
                    }
                    ExpressionKind::Unary { child, .. } => pending.push(*child),
                    ExpressionKind::Binary { left, right, .. } => {
                        pending.push(*left);
                        pending.push(*right);
                    }
                    _ => (),
                }
            }
            reactions.insert(
                name.text.clone(),
                Reaction {
                    span: symbols[&name.text].span,
                    id: name.text.clone(),
                    reversible: *reversible,
                    reactants,
                    products,
                    modifiers: modifiers.into_iter().collect(),
                    rate: ExpressionId(*rate),
                },
            );
        }
    }
    let reactions: Vec<Reaction> = reactions.into_values().collect();
    for reaction in &reactions {
        for participant in reaction.reactants.iter().chain(&reaction.products) {
            if let Some(rule) = rules.get(&participant.species)
                && matches!(
                    symbols[&participant.species].symbol,
                    Symbol::Species {
                        boundary: false,
                        ..
                    }
                )
            {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    rule.span,
                    "variable (non-boundary) species cannot be both determined by a rule and modified by a reaction; declare boundary species ($S or const S)",
                ));
            }
        }
    }
    let origin = document.source.span(0, 0).expect("empty source span");
    let mut warnings = vec![Diagnostic::new(Severity::Warning, DiagnosticCode::UnspecifiedUnits, "dimensional checking is partial: symbols without explicit or inherited units and bare arithmetic numbers have unspecified units; no full model consistency certificate").with_label(Label::primary(origin, "model"))];
    if needs_default {
        warnings.push(
            Diagnostic::new(
                Severity::Note,
                DiagnosticCode::DefaultCompartment,
                "applied upstream default_compartment: size 1, spatial dimensions 3, SBO:0000410",
            )
            .with_label(Label::primary(origin, "implicit compartment")),
        );
    }
    document
        .limits
        .check(LimitKind::Diagnostics, warnings.len())
        .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
    let function_definitions = document
        .functions
        .iter()
        .map(|f| FunctionDefinition {
            id: f.name.text.clone(),
            parameters: f.parameters.iter().map(|p| p.text.clone()).collect(),
            body: ExpressionId(f.body),
            span: f.span,
        })
        .collect();
    let events = resolved_events
        .into_iter()
        .map(|ev| Event {
            id: ev.id,
            trigger: EventTrigger {
                expression: ExpressionId(ev.trigger),
                initial_value: ev.initial_value,
                persistent: ev.persistent,
            },
            use_values_from_trigger_time: ev.from_trigger,
            delay: ev.delay.map(ExpressionId),
            priority: ev.priority.map(ExpressionId),
            assignments: ev
                .assignments
                .into_iter()
                .map(|a| EventAssignment {
                    variable: a.variable.text,
                    expression: ExpressionId(a.expression),
                    span: a.span,
                })
                .collect(),
            span: ev.span,
        })
        .collect();
    Ok(ValidatedModel {
        source: document.source.clone(),
        id: document.name.clone(),
        function_definitions,
        compartments,
        species,
        parameters,
        reactions,
        events,
        expressions,
        initial_assignments,
        rules: model_rules,
        warnings,
        unit_definitions: unit_analysis.definitions,
        unit_annotations: unit_analysis.annotations,
        model_units: unit_analysis.model_units,
        species_substance_units: unit_analysis.species_units,
        symbol_dimensions: unit_analysis.symbols,
        expression_dimensions: unit_analysis.expressions,
    })
}
fn declare(
    document: &ParsedDocument,
    symbols: &mut BTreeMap<String, Entry>,
    name: &Name,
    symbol: Symbol,
    next_seen: &mut usize,
) -> Result<(), Diagnostic> {
    if crate::model::units::ModelUnit::from_name(&name.text).is_some() {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            name.span,
            "model default unit identifiers cannot be numeric model symbols; use an explicit unit declaration",
        ));
    }
    if let Some(previous) = symbols.get(&name.text) {
        return Err(error(
            DiagnosticCode::DeclarationConflict,
            name.span,
            format!("duplicate or conflicting declaration of {}", name.text),
        )
        .with_label(Label::secondary(previous.span, "previous declaration")));
    }
    document
        .limits
        .checked_add(
            LimitKind::ModelEntities,
            symbols.len() + document.functions.len(),
            1,
        )
        .map_err(|e| error(DiagnosticCode::ResourceLimit, name.span, e.to_string()))?;
    let first_seen = *next_seen;
    *next_seen += 1;
    symbols.insert(
        name.text.clone(),
        Entry {
            symbol,
            span: name.span,
            qualifier: None,
            first_seen,
        },
    );
    Ok(())
}
fn expression_initial(initials: &initialization::Initials, name: &str) -> Option<ExpressionId> {
    match initials.get(name).map(|(value, _)| value) {
        Some(InitialValue::Expression(root)) => Some(ExpressionId(*root)),
        _ => None,
    }
}
fn references(participants: &[Participant]) -> Result<Vec<SpeciesReference>, Diagnostic> {
    let mut found = BTreeSet::new();
    participants
        .iter()
        .map(|p| {
            if !found.insert(&p.name.text) {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    p.name.span,
                    "repeated species in one reaction side are deferred; use a numeric coefficient",
                ));
            }
            Ok(SpeciesReference {
                species: p.name.text.clone(),
                stoichiometry: p.stoichiometry,
            })
        })
        .collect()
}
