//! Audited Antimony species/default-unit mapping, separate from source unit algebra.
//!
//! Raw alias identity matters: upstream reduces inverse pairs BEFORE expansion.
//! Its finalize/export passes differ for unannotated explicit compartments. We
//! reject export-time dangling references rather than manufacture missing units.
use super::units::{ZERO, coefficient_parts, definition_dimension, primitive_dimension};
use super::{Entry, Symbol};
use crate::model::units::*;
use crate::syntax::{ExprKind, ParsedDocument, Statement, error};
use crate::{Diagnostic, DiagnosticCode, LimitKind, Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
struct RawComponent {
    name: String,
    exponent: i32,
    scale: i32,
    multiplier: f64,
}
#[derive(Clone)]
struct RawDefinition {
    id: String,
    components: Vec<RawComponent>,
}
type Signature = Vec<(String, i32, i32, u64)>;
fn signature(components: &[RawComponent]) -> Signature {
    components
        .iter()
        .map(|c| (c.name.clone(), c.exponent, c.scale, c.multiplier.to_bits()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn canonical_signature(components: &[UnitComponent]) -> Signature {
    components
        .iter()
        .map(|c| {
            (
                c.kind.as_str().to_owned(),
                c.exponent,
                c.scale,
                c.multiplier.to_bits(),
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn primitive(name: &str) -> RawDefinition {
    let name = UnitKind::from_name(name)
        .expect("audited primitive")
        .as_str()
        .to_owned();
    RawDefinition {
        id: name.clone(),
        components: vec![RawComponent {
            name,
            exponent: 1,
            scale: 0,
            multiplier: 1.0,
        }],
    }
}
fn combined(left: &RawDefinition, right: &RawDefinition) -> RawDefinition {
    let mut components = left.components.clone();
    components.extend_from_slice(&right.components);
    // Pinned UnitDef::Reduce skips a two-component inverse pair. Its outer
    // iterator advances after erasing a pair, so preserve that order as well.
    if components.len() > 2 {
        let mut first = 0;
        while first < components.len() {
            let found = (first + 1..components.len()).find(|&other| {
                let a = &components[first];
                let b = &components[other];
                a.name == b.name
                    && a.exponent == -b.exponent
                    && a.scale == b.scale
                    && a.multiplier == b.multiplier
            });
            if let Some(other) = found {
                components.remove(other);
                components.remove(first);
                first = 0;
            }
            first += 1;
        }
    }
    let mut id = format!("{}_{}", left.id, right.id);
    // Upstream FixUnitName singularizes generated names too.
    if id.ends_with('s') && !id.ends_with("_s") && !id.contains("siemens") && !id.contains("per_nM")
    {
        id.pop();
    }
    RawDefinition { id, components }
}
fn expand(
    raw: &RawDefinition,
    definitions: &BTreeMap<String, UnitDefinition>,
    span: Span,
) -> Result<UnitDefinition, Diagnostic> {
    let mut components = Vec::new();
    for c in &raw.components {
        let expanded = if let Some(kind) = UnitKind::from_name(&c.name) {
            vec![UnitComponent {
                kind,
                exponent: 1,
                scale: c.scale,
                multiplier: c.multiplier,
            }]
        } else {
            definitions[&c.name].components.clone()
        };
        for mut component in expanded {
            component.exponent = component
                .exponent
                .checked_mul(c.exponent)
                .filter(|n| n.abs() <= UnitLimits::RESOLVED_EXPONENT)
                .ok_or_else(|| {
                    error(
                        DiagnosticCode::ResourceLimit,
                        span,
                        "generated unit exponent exceeds magnitude 64",
                    )
                })?;
            if components.len() == UnitLimits::GENERATED_COMPONENTS {
                return Err(error(
                    DiagnosticCode::ResourceLimit,
                    span,
                    "generated species unit exceeds ten preserved components",
                ));
            }
            components.push(component);
        }
    }
    Ok(UnitDefinition {
        id: raw.id.clone(),
        components,
    })
}

pub(super) struct Mapping {
    pub model_units: BTreeMap<ModelUnit, String>,
    pub species_units: BTreeMap<String, String>,
}

pub(super) fn complete(
    document: &ParsedDocument,
    symbols: &BTreeMap<String, Entry>,
    first_seen: &BTreeMap<String, usize>,
    definitions: &mut BTreeMap<String, UnitDefinition>,
    annotations: &mut BTreeMap<String, String>,
    dimensions: &mut BTreeMap<String, DimensionalStatus>,
) -> Result<(Mapping, Vec<String>), Diagnostic> {
    let origin = document.source.span(0, 0).expect("origin");
    let source_definition_count = definitions.len();
    let mut raw = BTreeMap::new();
    let mut order = Vec::new();
    for statement in &document.statements {
        if let Statement::UnitDefinition { name, factors } = statement {
            let components = factors
                .iter()
                .map(|f| {
                    let (multiplier, scale) = f
                        .coefficient
                        .as_ref()
                        .map(|s| coefficient_parts(s, f.name.span))
                        .transpose()?
                        .unwrap_or((1.0, 0));
                    Ok(RawComponent {
                        name: UnitKind::from_name(&f.name.text)
                            .map_or_else(|| f.name.text.clone(), |k| k.as_str().to_owned()),
                        exponent: f.exponent,
                        scale,
                        multiplier,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            raw.insert(
                name.text.clone(),
                RawDefinition {
                    id: name.text.clone(),
                    components,
                },
            );
        }
    }
    for kind in [
        UnitKind::Mole,
        UnitKind::Second,
        UnitKind::Litre,
        UnitKind::Metre,
        UnitKind::Dimensionless,
    ] {
        raw.insert(kind.as_str().to_owned(), primitive(kind.as_str()));
    }
    // Unit lookup order includes primitive references, not just definitions.
    let mut seen = first_seen
        .iter()
        .filter_map(|(name, pos)| {
            let canonical = UnitKind::from_name(name).map_or(name.as_str(), |k| k.as_str());
            raw.contains_key(canonical)
                .then_some((canonical.to_owned(), *pos))
        })
        .collect::<Vec<_>>();
    seen.sort_by_key(|(_, pos)| *pos);
    let mut inserted = BTreeSet::new();
    for (name, _) in seen {
        if inserted.insert(name.clone()) {
            order.push(name);
        }
    }
    // All five builtins remain available without adding model defaults.
    for kind in [
        UnitKind::Mole,
        UnitKind::Litre,
        UnitKind::Metre,
        UnitKind::Second,
        UnitKind::Dimensionless,
    ] {
        if inserted.insert(kind.as_str().to_owned()) {
            order.push(kind.as_str().to_owned());
        }
    }
    let mut lookup = BTreeMap::<Signature, String>::new();
    for name in &order {
        lookup
            .entry(signature(&raw[name].components))
            .or_insert(name.clone());
    }
    let mut species = symbols
        .iter()
        .filter(|(_, e)| matches!(e.symbol, Symbol::Species { .. }))
        .map(|(n, e)| (n.clone(), e.span.start()))
        .collect::<BTreeMap<_, _>>();
    let mut note = |name: &str, pos: usize| {
        if let Some(first) = species.get_mut(name) {
            *first = (*first).min(pos);
        }
    };
    for statement in &document.statements {
        match statement {
            Statement::UnitAnnotation { name, .. }
            | Statement::Initial { name, .. }
            | Statement::Rule { name, .. } => note(&name.text, name.span.start()),
            Statement::Reaction {
                reactants,
                products,
                ..
            } => {
                for p in reactants.iter().chain(products) {
                    note(&p.name.text, p.name.span.start());
                }
            }
            _ => (),
        }
    }
    let fn_nodes = document.function_body_nodes();
    for (id, expression) in document.expressions.iter().enumerate() {
        if !fn_nodes[id]
            && let ExprKind::Symbol(name) = &expression.kind
        {
            note(&name.text, name.span.start());
        }
    }
    let mut species = species.into_iter().collect::<Vec<_>>();
    species.sort_by_key(|(_, pos)| *pos);
    let mut generated = Vec::new();
    // Finalize creates units for implicit and explicitly annotated compartments.
    for (name, _) in &species {
        let Some(annotation) = annotations.get(name) else {
            continue;
        };
        let Symbol::Species { compartment, .. } = &symbols[name].symbol else {
            unreachable!()
        };
        let volume = match compartment {
            None => Some("litre"),
            Some(c) => annotations.get(c).map(String::as_str),
        };
        let Some(volume) = volume else { continue };
        let candidate = combined(&raw[annotation], &raw[volume]);
        if let Some(existing) = raw.get(&candidate.id)
            && signature(&existing.components) != signature(&candidate.components)
        {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                symbols[name].span,
                "generated species unit identifier collides with a different source unit",
            ));
        }
        if symbols.contains_key(&candidate.id)
            || document
                .functions
                .iter()
                .any(|f| f.name.text == candidate.id)
        {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                symbols[name].span,
                "generated species unit identifier collides with a model symbol",
            ));
        }
        let key = signature(&candidate.components);
        if lookup.contains_key(&key) {
            continue;
        }
        document
            .limits
            .checked_add(
                LimitKind::ModelEntities,
                symbols.len() + document.functions.len() + source_definition_count,
                generated.len() + 1,
            )
            .map_err(|e| {
                error(
                    DiagnosticCode::ResourceLimit,
                    symbols[name].span,
                    e.to_string(),
                )
            })?;
        let definition = expand(&candidate, definitions, symbols[name].span)?;
        lookup.insert(key, candidate.id.clone());
        generated.push(candidate.id.clone());
        order.push(candidate.id.clone());
        definitions.insert(candidate.id.clone(), definition);
        raw.insert(candidate.id.clone(), candidate);
    }
    // Redirect the six special names only. Regular aliases retain their identity.
    let mut redirects = BTreeMap::new();
    let mut model_units = BTreeMap::new();
    for default in [
        ModelUnit::Substance,
        ModelUnit::Volume,
        ModelUnit::Area,
        ModelUnit::Length,
        ModelUnit::Extent,
        ModelUnit::Time,
    ] {
        let Some(definition) = definitions.get(default.as_str()) else {
            continue;
        };
        let dimension = definition_dimension(definition);
        let expected = match default {
            ModelUnit::Substance | ModelUnit::Extent => Dimensions {
                substance: 1,
                ..ZERO
            },
            ModelUnit::Volume => Dimensions { length: 3, ..ZERO },
            ModelUnit::Area => Dimensions { length: 2, ..ZERO },
            ModelUnit::Length => Dimensions { length: 1, ..ZERO },
            ModelUnit::Time => Dimensions { time: 1, ..ZERO },
        };
        if dimension != expected && dimension != ZERO {
            return Err(error(
                DiagnosticCode::UnitsInconsistency,
                origin,
                format!(
                    "model {} default has incompatible dimensions",
                    default.as_str()
                ),
            ));
        }
        let reference = if let [c] = definition.components.as_slice()
            && c.exponent == 1
            && c.scale == 0
            && c.multiplier == 1.0
        {
            c.kind.as_str().to_owned()
        } else {
            let key = canonical_signature(&definition.components);
            order
                .iter()
                .find(|id| {
                    ModelUnit::from_name(id).is_none()
                        && definitions
                            .get(*id)
                            .is_some_and(|d| canonical_signature(&d.components) == key)
                })
                .cloned()
                .unwrap_or_else(|| default.as_str().to_owned())
        };
        if reference != default.as_str() {
            redirects.insert(default.as_str().to_owned(), reference.clone());
        }
        model_units.insert(default, reference);
    }
    let mut species_units = BTreeMap::new();
    for (name, _) in &species {
        let Some(annotation) = annotations.get(name) else {
            continue;
        };
        let Symbol::Species {
            compartment,
            substance_only,
            ..
        } = &symbols[name].symbol
        else {
            unreachable!()
        };
        if substance_only.unwrap_or(false) {
            species_units.insert(
                name.clone(),
                redirects
                    .get(annotation)
                    .cloned()
                    .unwrap_or_else(|| annotation.clone()),
            );
            continue;
        }
        let candidate = if let Some(c) = compartment {
            if let Some(volume) = annotations.get(c) {
                combined(&raw[annotation], &raw[volume])
            } else if let Some(volume) = raw.get("volume") {
                combined(&raw[annotation], volume)
            } else {
                let mut volume = primitive("litre");
                volume.id = "volume".to_owned();
                combined(&raw[annotation], &volume)
            }
        } else {
            combined(&raw[annotation], &raw["litre"])
        };
        let key = signature(&candidate.components);
        // Same-name matching precedes structural reuse upstream.
        let resolved=raw.get(&candidate.id).filter(|d|signature(&d.components)==key).map(|d|d.id.clone()).or_else(||lookup.get(&key).cloned())
            .ok_or_else(||error(DiagnosticCode::UnsupportedFeature,symbols[name].span,"upstream export would reference a missing generated substance unit; explicitly annotate the compartment units"))?;
        species_units.insert(
            name.clone(),
            redirects.get(&resolved).cloned().unwrap_or(resolved),
        );
    }
    // Effective dimensions for unannotated symbols may be inherited, but metadata
    // is never copied onto the source annotation map or species attribute.
    for (name, entry) in symbols {
        if matches!(entry.symbol, Symbol::Compartment { .. })
            && !annotations.contains_key(name)
            && let Some(definition) = definitions.get("volume")
        {
            dimensions.insert(
                name.clone(),
                DimensionalStatus::Known(definition_dimension(definition)),
            );
        }
    }
    for (name, _) in &species {
        let Symbol::Species {
            compartment,
            substance_only,
            ..
        } = &symbols[name].symbol
        else {
            unreachable!()
        };
        if substance_only.unwrap_or(false) {
            if !annotations.contains_key(name) {
                if let Some(substance) = definitions.get("substance") {
                    dimensions.insert(
                        name.clone(),
                        DimensionalStatus::Known(definition_dimension(substance)),
                    );
                }
            } else if let DimensionalStatus::Known(dim) = dimensions[name]
                && dim
                    != (Dimensions {
                        substance: 1,
                        ..ZERO
                    })
                && dim != ZERO
            {
                return Err(error(
                    DiagnosticCode::UnitsInconsistency,
                    symbols[name].span,
                    "substanceOnly species units must represent substance or dimensionless units",
                ));
            }
            continue;
        }
        let compartment = compartment.as_deref().unwrap_or("default_compartment");
        if !annotations.contains_key(name) {
            if let Some(substance) = definitions.get("substance")
                && let Some(DimensionalStatus::Known(volume)) = dimensions.get(compartment)
            {
                let amount = definition_dimension(substance);
                dimensions.insert(
                    name.clone(),
                    DimensionalStatus::Known(Dimensions {
                        substance: amount.substance - volume.substance,
                        time: amount.time - volume.time,
                        length: amount.length - volume.length,
                    }),
                );
            }
        } else if let Some(DimensionalStatus::Known(volume)) = dimensions.get(compartment) {
            let reference = &species_units[name];
            let amount = if let Some(kind) = UnitKind::from_name(reference) {
                primitive_dimension(kind)
            } else {
                definition_dimension(&definitions[reference])
            };
            let effective = Dimensions {
                substance: amount.substance - volume.substance,
                time: amount.time - volume.time,
                length: amount.length - volume.length,
            };
            if dimensions[name] != DimensionalStatus::Known(effective) {
                return Err(error(
                    DiagnosticCode::UnitsInconsistency,
                    symbols[name].span,
                    "source concentration dimensions conflict with upstream substance/default-compartment mapping",
                ));
            }
        }
    }
    for (name, unit) in annotations.iter_mut() {
        if !matches!(symbols[name].symbol, Symbol::Species { .. })
            && let Some(reference) = redirects.get(unit)
        {
            *unit = reference.clone();
        }
    }
    for name in redirects.keys() {
        definitions.remove(name);
    }
    Ok((
        Mapping {
            model_units,
            species_units,
        },
        generated,
    ))
}
