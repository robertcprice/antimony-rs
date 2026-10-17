//! Initialization-only dependencies: never reaction-network edges or runtime rules.
use super::evaluation::evaluate;
use super::{Entry, Symbol};
use crate::syntax::{ExprKind, InitialValue, ParsedDocument, error};
use crate::{Diagnostic, DiagnosticCode, Span};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Initials = BTreeMap<String, (InitialValue, Span)>;
pub(super) struct Resolved {
    pub value: Option<f64>,
    pub constant: bool,
}

/// Kahn traversal is iterative even for chains much longer than expression depth.
/// Each syntax expression belongs to one initializer, so work/storage is bounded
/// by admitted tokens/entities (plus ordered-map lookup costs), not chain squared.
pub(super) fn resolve(
    document: &ParsedDocument,
    initials: &Initials,
    rules: &super::symbols::Rules,
    event_targets: &BTreeSet<String>,
    symbols: &BTreeMap<String, Entry>,
) -> Result<BTreeMap<String, Resolved>, Diagnostic> {
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    let mut remaining = BTreeMap::new();
    let mut ready = BTreeSet::new();
    for (name, entry) in symbols {
        if matches!(entry.symbol, Symbol::Reaction) {
            continue;
        }
        let initial = initials.get(name).map(|(value, _)| value);
        let assign_rule = rules
            .get(name)
            .filter(|r| r.kind == crate::RuleKind::Assignment);
        let mut deps = BTreeSet::new();
        if let Some(root) = match (initial, assign_rule) {
            (Some(InitialValue::Expression(root)), _) => Some(*root),
            (_, Some(rule)) => Some(rule.expression),
            _ => None,
        } {
            let mut pending = vec![root];
            while let Some(id) = pending.pop() {
                let kind = &document.expressions[id].kind;
                if let ExprKind::Symbol(reference) = kind {
                    deps.insert(reference.text.clone());
                }
                pending.extend(kind.children());
            }
        }
        for dep in &deps {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(name.clone());
        }
        if deps.is_empty() {
            ready.insert(name.clone());
        }
        remaining.insert(name.clone(), deps.len());
        dependencies.insert(name.clone(), deps);
    }
    let mut values = BTreeMap::<String, Resolved>::new();
    while let Some(name) = ready.pop_first() {
        let value = if let Some(rule) = rules
            .get(&name)
            .filter(|r| r.kind == crate::RuleKind::Assignment)
        {
            rule.lexical_scalar
        } else {
            match initials.get(&name).map(|(value, _)| value) {
                Some(InitialValue::Scalar(value)) => Some(*value),
                Some(InitialValue::Expression(root)) => evaluate(document, *root, &values)?,
                None => match symbols[&name].symbol {
                    Symbol::Compartment { size, .. } => size,
                    _ => None,
                },
            }
        };
        let constant = if let Symbol::Species { boundary, .. } = &symbols[&name].symbol {
            *boundary
        } else if rules.contains_key(&name) || event_targets.contains(&name) {
            false
        } else {
            symbols[&name]
                .qualifier
                .unwrap_or_else(|| match &symbols[&name].symbol {
                    Symbol::Species { boundary, .. } => *boundary,
                    Symbol::Compartment { .. } | Symbol::Parameter => {
                        dependencies[&name].iter().all(|dep| values[dep].constant)
                    }
                    Symbol::Reaction => {
                        unreachable!("reaction initialization rejected before resolution")
                    }
                })
        };
        values.insert(name.clone(), Resolved { value, constant });
        if let Some(users) = dependents.get(&name) {
            for user in users {
                let count = remaining.get_mut(user).expect("initialized dependent");
                *count -= 1;
                if *count == 0 {
                    ready.insert(user.clone());
                }
            }
        }
    }
    if values.len() != dependencies.len() {
        // Walk only unresolved edges to locate a genuine cycle member, rather
        // than labeling an arbitrary downstream dependent as the cycle itself.
        let mut name = remaining.iter().find(|(_, n)| **n > 0).unwrap().0;
        let mut seen = BTreeSet::new();
        while seen.insert(name) {
            name = dependencies[name]
                .iter()
                .find(|dep| !values.contains_key(*dep))
                .unwrap();
        }
        let span = initials
            .get(name)
            .map(|(_, span)| *span)
            .or_else(|| rules.get(name).map(|r| r.span))
            .expect("cycle member has initializer or assignment rule");
        return Err(error(
            DiagnosticCode::InitializationCycle,
            span,
            format!("initialization dependency cycle includes {name}"),
        ));
    }
    Ok(values)
}
