//! Native SBML Level 3 Version 2 Core reader and Antimony decompiler.
//!
//! Parses a supported SBML L3V2 Core XML document using `quick-xml`, enforces
//! `Limits`, rejects DTD/entity expansion and unsupported SBML packages/constructs,
//! reconstructs canonical Antimony source, and validates the resulting model
//! through the standard `parse` + `analyze` pipeline.

use crate::model::units::{ModelUnit, UnitKind};
use crate::syntax::error;
use crate::{
    Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, SourceId, Span, ValidatedModel,
    analyze, format_antimony, parse,
};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::{BTreeMap, BTreeSet};

const SBML_L3V2_NS: &str = "http://www.sbml.org/sbml/level3/version2/core";
const SBML_L3V1_NS: &str = "http://www.sbml.org/sbml/level3/version1/core";
const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";
const TIME_CSYMBOL_URL: &str = "http://www.sbml.org/sbml/symbols/time";

fn parse_sbml_level_version(sbml: &XmlElement, origin: Span) -> Result<(u32, u32), Diagnostic> {
    let ns = sbml.attrs.get("xmlns").map(String::as_str).unwrap_or("");
    let level = sbml.attrs.get("level").map(String::as_str).unwrap_or("");
    let version = sbml.attrs.get("version").map(String::as_str).unwrap_or("");
    match (level, version, ns) {
        ("3", "2", SBML_L3V2_NS) => Ok((3, 2)),
        ("3", "1", SBML_L3V1_NS) => Ok((3, 1)),
        ("2", "1", "http://www.sbml.org/sbml/level2") => Ok((2, 1)),
        ("2", "2", "http://www.sbml.org/sbml/level2/version2") => Ok((2, 2)),
        ("2", "3", "http://www.sbml.org/sbml/level2/version3") => Ok((2, 3)),
        ("2", "4", "http://www.sbml.org/sbml/level2/version4") => Ok((2, 4)),
        ("2", "5", "http://www.sbml.org/sbml/level2/version5") => Ok((2, 5)),
        _ => Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "only SBML Level 2 (v1-v5) and Level 3 (v1-v2) Core documents are supported",
        )),
    }
}

fn is_benign_metadata_xmlns(key: &str) -> bool {
    matches!(
        key,
        "xmlns:rdf"
            | "xmlns:dc"
            | "xmlns:dcterms"
            | "xmlns:vCard"
            | "xmlns:vcard"
            | "xmlns:bqbiol"
            | "xmlns:bqmodel"
            | "xmlns:xhtml"
            | "xmlns:xsi"
            | "xsi:schemaLocation"
    )
}

#[derive(Clone, Debug)]
struct XmlElement {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<XmlNode>,
}

#[derive(Clone, Debug)]
enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    fn child_elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|n| match n {
            XmlNode::Element(e) => Some(e),
            XmlNode::Text(_) => None,
        })
    }

    fn first_child_element(&self, name: &str) -> Option<&XmlElement> {
        self.child_elements().find(|e| e.name == name)
    }

    fn text_content(&self) -> String {
        let mut out = String::new();
        for node in &self.children {
            if let XmlNode::Text(t) = node {
                out.push_str(t);
            }
        }
        out.trim().to_owned()
    }
}

#[derive(Clone, Debug)]
struct RawUnitComponent {
    kind: String,
    exponent: i32,
    scale: i32,
    multiplier: f64,
}

#[derive(Clone, Debug)]
struct RawUnitDef {
    id: String,
    components: Vec<RawUnitComponent>,
}

/// Decompile a supported SBML Level 2 or Level 3 Core XML string into canonical Antimony text.
pub fn sbml_to_antimony(xml: &str, limits: &Limits) -> Result<String, Diagnostic> {
    let source = SourceFile::new(SourceId::new(0), "sbml_input", xml, limits).map_err(|e| {
        let dummy = SourceFile::new(SourceId::new(0), "sbml_input", "", &Limits::default())
            .expect("empty source");
        error(
            DiagnosticCode::ResourceLimit,
            dummy.span(0, 0).expect("origin"),
            e.to_string(),
        )
    })?;
    let origin = source.span(0, 0).expect("origin");
    let root = parse_xml_dom(xml, limits, origin)?;
    let raw_antimony = decompile_sbml_dom(&root, origin)?;
    let ant_source = SourceFile::new(
        SourceId::new(0),
        "decompiled_antimony",
        &raw_antimony,
        limits,
    )
    .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
    let parsed = parse(&ant_source, limits)?;
    format_antimony(&parsed)
}

/// Parse and validate a supported SBML Level 2 or Level 3 Core XML string into a [`ValidatedModel`].
pub fn read_sbml(xml: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let canonical_antimony = sbml_to_antimony(xml, limits)?;
    let origin_source = SourceFile::new(SourceId::new(0), "sbml_input", "", &Limits::default())
        .expect("empty source");
    let origin = origin_source.span(0, 0).expect("origin");
    let ant_source = SourceFile::new(
        SourceId::new(0),
        "decompiled_antimony",
        &canonical_antimony,
        limits,
    )
    .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
    let parsed = parse(&ant_source, limits)?;
    analyze(&parsed)
}

fn parse_xml_dom(xml: &str, limits: &Limits, origin: Span) -> Result<XmlElement, Diagnostic> {
    let upper = xml.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "XML DOCTYPE and ENTITY declarations are rejected for security",
        ));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut stack: Vec<XmlElement> = Vec::new();
    let mut root: Option<XmlElement> = None;
    let mut node_count = 0usize;
    let mut skip_depth = 0usize;

    loop {
        let ev = reader.read_event().map_err(|e| {
            error(
                DiagnosticCode::Syntax,
                origin,
                format!("malformed SBML XML: {e}"),
            )
        })?;
        match ev {
            Event::Decl(_) | Event::Comment(_) => {}
            Event::DocType(_) | Event::PI(_) | Event::CData(_) => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    "XML DOCTYPE, PI, and CDATA constructs are outside this SBML slice",
                ));
            }
            Event::Start(tag) => {
                node_count += 1;
                limits
                    .check(LimitKind::Tokens, node_count)
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
                limits
                    .check(LimitKind::ExpressionDepth, stack.len() + skip_depth + 1)
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
                if skip_depth > 0 {
                    skip_depth += 1;
                    continue;
                }
                let tag_name = tag.name();
                let raw_name = tag_name.as_ref();
                if matches!(raw_name, "notes" | "annotation") && !stack.is_empty() {
                    skip_depth = 1;
                    continue;
                }
                stack.push(build_element(&tag, origin)?);
            }
            Event::Empty(tag) => {
                node_count += 1;
                limits
                    .check(LimitKind::Tokens, node_count)
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
                if skip_depth > 0 {
                    continue;
                }
                let tag_name = tag.name();
                let raw_name = tag_name.as_ref();
                if matches!(raw_name, "notes" | "annotation") && !stack.is_empty() {
                    continue;
                }
                let elem = build_element(&tag, origin)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Element(elem));
                } else if root.is_none() {
                    root = Some(elem);
                } else {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        origin,
                        "multiple root elements in SBML XML",
                    ));
                }
            }
            Event::Text(text) => {
                if skip_depth > 0 {
                    continue;
                }
                let decoded = unescape_xml_str(text.as_ref());
                if !decoded.trim().is_empty()
                    && let Some(parent) = stack.last_mut()
                {
                    if let Some(XmlNode::Text(existing)) = parent.children.last_mut() {
                        existing.push_str(&decoded);
                    } else {
                        parent.children.push(XmlNode::Text(decoded));
                    }
                }
            }
            Event::GeneralRef(entity) => {
                if skip_depth > 0 {
                    continue;
                }
                let resolved = match entity.as_ref() {
                    "lt" => "<",
                    "gt" => ">",
                    "amp" => "&",
                    "quot" => "\"",
                    "apos" => "'",
                    other => {
                        return Err(error(
                            DiagnosticCode::Syntax,
                            origin,
                            format!("unsupported XML entity reference &{other};"),
                        ));
                    }
                };
                if let Some(parent) = stack.last_mut() {
                    if let Some(XmlNode::Text(existing)) = parent.children.last_mut() {
                        existing.push_str(resolved);
                    } else {
                        parent.children.push(XmlNode::Text(resolved.to_owned()));
                    }
                }
            }
            Event::End(_) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    continue;
                }
                let elem = stack.pop().ok_or_else(|| {
                    error(
                        DiagnosticCode::Syntax,
                        origin,
                        "unexpected closing tag in XML",
                    )
                })?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Element(elem));
                } else if root.is_none() {
                    root = Some(elem);
                } else {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        origin,
                        "multiple root elements in SBML XML",
                    ));
                }
            }
            Event::Eof => break,
        }
    }

    if !stack.is_empty() || skip_depth > 0 {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            "unclosed XML element in SBML input",
        ));
    }
    root.ok_or_else(|| error(DiagnosticCode::Syntax, origin, "empty SBML XML document"))
}

fn unescape_xml_str(input: &str) -> String {
    if !input.contains('&') {
        return input.to_owned();
    }
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn build_element(tag: &BytesStart<'_>, origin: Span) -> Result<XmlElement, Diagnostic> {
    let tag_name = tag.name();
    let raw_name = tag_name.as_ref();
    if raw_name.contains(':') {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("prefixed XML element <{raw_name}> is outside SBML Core"),
        ));
    }
    let mut attrs = BTreeMap::new();
    for attr in tag.attributes() {
        let attr = attr.map_err(|e| {
            error(
                DiagnosticCode::Syntax,
                origin,
                format!("malformed XML attribute: {e}"),
            )
        })?;
        let key = attr.key.as_ref().to_owned();
        if key.contains(':') && !(raw_name == "sbml" && is_benign_metadata_xmlns(&key)) {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!("extension attribute {key:?} is outside SBML Core"),
            ));
        }
        let val = unescape_xml_str(attr.value.as_ref());
        if attrs.insert(key.clone(), val).is_some() {
            return Err(error(
                DiagnosticCode::Syntax,
                origin,
                format!("duplicate XML attribute {key:?}"),
            ));
        }
    }
    Ok(XmlElement {
        name: raw_name.to_owned(),
        attrs,
        children: Vec::new(),
    })
}

fn decompile_sbml_dom(sbml: &XmlElement, origin: Span) -> Result<String, Diagnostic> {
    if sbml.name != "sbml" {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            format!("expected <sbml> root element, found <{}>", sbml.name),
        ));
    }
    let (sbml_level, _sbml_version) = parse_sbml_level_version(sbml, origin)?;
    let is_l2 = sbml_level == 2;
    let models: Vec<&XmlElement> = sbml.child_elements().collect();
    if models.len() != 1 || models[0].name != "model" {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "expected a single <model> child inside <sbml>",
        ));
    }
    let model = models[0];
    let model_id = model
        .attrs
        .get("id")
        .cloned()
        .unwrap_or_else(|| "__main".to_owned());

    let mut fn_blocks = Vec::<String>::new();
    let mut unit_defs = Vec::<RawUnitDef>::new();
    let mut compartments = Vec::<&XmlElement>::new();
    let mut species_list = Vec::<&XmlElement>::new();
    let mut parameters = Vec::<&XmlElement>::new();
    let mut initial_assignments = Vec::<&XmlElement>::new();
    let mut rules = Vec::<&XmlElement>::new();
    let mut reactions = Vec::<&XmlElement>::new();
    let mut events = Vec::<&XmlElement>::new();

    for section in model.child_elements() {
        match section.name.as_str() {
            "listOfFunctionDefinitions" => {
                for f in section.child_elements() {
                    if f.name != "functionDefinition" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfFunctionDefinitions", f.name),
                        ));
                    }
                    fn_blocks.push(decompile_function(f, origin)?);
                }
            }
            "listOfUnitDefinitions" => {
                for u in section.child_elements() {
                    if u.name != "unitDefinition" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfUnitDefinitions", u.name),
                        ));
                    }
                    unit_defs.push(parse_unit_definition(u, origin)?);
                }
            }
            "listOfCompartments" => {
                for c in section.child_elements() {
                    if c.name != "compartment" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfCompartments", c.name),
                        ));
                    }
                    compartments.push(c);
                }
            }
            "listOfSpecies" => {
                for s in section.child_elements() {
                    if s.name != "species" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfSpecies", s.name),
                        ));
                    }
                    species_list.push(s);
                }
            }
            "listOfParameters" => {
                for p in section.child_elements() {
                    if p.name != "parameter" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfParameters", p.name),
                        ));
                    }
                    parameters.push(p);
                }
            }
            "listOfInitialAssignments" => {
                for ia in section.child_elements() {
                    if ia.name != "initialAssignment" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfInitialAssignments", ia.name),
                        ));
                    }
                    initial_assignments.push(ia);
                }
            }
            "listOfRules" => {
                for r in section.child_elements() {
                    rules.push(r);
                }
            }
            "listOfReactions" => {
                for rxn in section.child_elements() {
                    if rxn.name != "reaction" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfReactions", rxn.name),
                        ));
                    }
                    reactions.push(rxn);
                }
            }
            "listOfEvents" => {
                for ev in section.child_elements() {
                    if ev.name != "event" {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            format!("unexpected <{}> in listOfEvents", ev.name),
                        ));
                    }
                    events.push(ev);
                }
            }
            other => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!("SBML section <{other}> is outside this slice"),
                ));
            }
        }
    }

    let unit_def_map: BTreeMap<String, &RawUnitDef> =
        unit_defs.iter().map(|u| (u.id.clone(), u)).collect();

    // Determine compartment unit annotations and whether default_compartment is implicit.
    let mut comp_units = BTreeMap::<String, String>::new();
    let mut has_implicit_default_comp = false;
    for c in &compartments {
        let id = req_attr(c, "id", origin)?;
        if let Some(u) = c.attrs.get("units") {
            comp_units.insert(id.to_owned(), u.clone());
        }
        if id == "default_compartment"
            && c.attrs.get("sboTerm").map(String::as_str) == Some("SBO:0000410")
            && c.attrs.get("size").map(String::as_str) == Some("1")
            && c.attrs.get("constant").map(String::as_str) == Some("true")
            && !c.attrs.contains_key("units")
        {
            has_implicit_default_comp = true;
        }
    }

    // Track which unitDefinitions are directly referenced (so we can identify auto-generated
    // species substance units like `mM_litre` and avoid emitting redundant definitions).
    let mut direct_unit_refs = BTreeSet::<String>::new();
    for default in [
        ModelUnit::Substance,
        ModelUnit::Volume,
        ModelUnit::Area,
        ModelUnit::Length,
        ModelUnit::Extent,
        ModelUnit::Time,
    ] {
        if let Some(u) = model.attrs.get(default.sbml_attribute()) {
            direct_unit_refs.insert(u.clone());
        }
    }
    for u in comp_units.values() {
        direct_unit_refs.insert(u.clone());
    }
    for p in &parameters {
        if let Some(u) = p.attrs.get("units") {
            direct_unit_refs.insert(u.clone());
        }
    }

    // Resolve species unit annotations (`S has u`) and identify generated compound substance units.
    let mut species_unit_annotations = Vec::<(String, String)>::new();
    let mut generated_substance_units = BTreeSet::<String>::new();
    let mut synth_unit_defs = Vec::<(String, String)>::new();

    for s in &species_list {
        let sp_id = req_attr(s, "id", origin)?;
        let subst_only = opt_bool_attr(s, "hasOnlySubstanceUnits", false, origin)?;
        let Some(su) = s.attrs.get("substanceUnits") else {
            continue;
        };
        if subst_only {
            direct_unit_refs.insert(su.clone());
            species_unit_annotations.push((sp_id.to_owned(), su.clone()));
            continue;
        }
        let comp_id = req_attr(s, "compartment", origin)?;
        let vol_unit = if comp_id == "default_compartment" && has_implicit_default_comp {
            "litre".to_owned()
        } else if let Some(cu) = comp_units.get(comp_id) {
            cu.clone()
        } else if let Some(vu) = model.attrs.get(ModelUnit::Volume.sbml_attribute()) {
            vu.clone()
        } else {
            "litre".to_owned()
        };

        // Check if an existing unit definition `u_conc` in `<listOfUnitDefinitions>` produces `su`
        // when combined with `vol_unit` via Antimony's species-substance mapping.
        let mut matched_conc_unit = None;
        for u_def in &unit_defs {
            let gen_id = combined_unit_name(&u_def.id, &vol_unit);
            if gen_id == *su {
                matched_conc_unit = Some(u_def.id.clone());
                if unit_def_map.contains_key(su) {
                    generated_substance_units.insert(su.clone());
                }
                break;
            }
        }
        if matched_conc_unit.is_none() {
            for prim in ["mole", "litre", "metre", "second", "dimensionless"] {
                let gen_id = combined_unit_name(prim, &vol_unit);
                if gen_id == *su {
                    matched_conc_unit = Some(prim.to_owned());
                    if unit_def_map.contains_key(su) {
                        generated_substance_units.insert(su.clone());
                    }
                    break;
                }
            }
        }
        if matched_conc_unit.is_none() {
            let su_comps = resolve_unit_components(su, &unit_def_map, origin)?;
            let vol_comps = resolve_unit_components(&vol_unit, &unit_def_map, origin)?;
            for u_def in &unit_defs {
                if generated_substance_units.contains(&u_def.id) {
                    continue;
                }
                let prod = multiply_components(&u_def.components, &vol_comps);
                if same_unit_signature(&prod, &su_comps) {
                    matched_conc_unit = Some(u_def.id.clone());
                    let gen_id = combined_unit_name(&u_def.id, &vol_unit);
                    if gen_id == *su && unit_def_map.contains_key(su) {
                        generated_substance_units.insert(su.clone());
                    }
                    break;
                }
            }
            if matched_conc_unit.is_none() {
                for prim in ["mole", "litre", "metre", "second", "dimensionless"] {
                    let prim_comps = resolve_unit_components(prim, &unit_def_map, origin)?;
                    let prod = multiply_components(&prim_comps, &vol_comps);
                    if same_unit_signature(&prod, &su_comps) {
                        matched_conc_unit = Some(prim.to_owned());
                        let gen_id = combined_unit_name(prim, &vol_unit);
                        if gen_id == *su && unit_def_map.contains_key(su) {
                            generated_substance_units.insert(su.clone());
                        }
                        break;
                    }
                }
            }
            if matched_conc_unit.is_none() {
                let synth_id = format!("{su}_per_{vol_unit}");
                if !unit_def_map.contains_key(&synth_id)
                    && !synth_unit_defs.iter().any(|(id, _)| id == &synth_id)
                {
                    let mut inv_vol = vol_comps;
                    for c in &mut inv_vol {
                        c.exponent = -c.exponent;
                    }
                    let mut conc_comps = su_comps;
                    conc_comps.extend(inv_vol);
                    let rhs = format_unit_components(&conc_comps, origin)?;
                    synth_unit_defs.push((synth_id.clone(), rhs));
                }
                matched_conc_unit = Some(synth_id);
            }
        }
        let conc_u = matched_conc_unit.expect("resolved concentration unit");
        direct_unit_refs.insert(conc_u.clone());
        species_unit_annotations.push((sp_id.to_owned(), conc_u));
    }

    let mut stmts = Vec::<String>::new();

    // 1. Emit unit definitions (skipping purely auto-generated compound substance units).
    for (idx, u_def) in unit_defs.iter().enumerate() {
        if generated_substance_units.contains(&u_def.id) && !direct_unit_refs.contains(&u_def.id) {
            continue;
        }
        let has_unreduced_generated = generated_substance_units
            .iter()
            .any(|g| g.starts_with(&format!("{}_", u_def.id)));
        let alias_target = if has_unreduced_generated && u_def.components.len() >= 2 {
            unit_defs[..idx]
                .iter()
                .find(|prev| {
                    !generated_substance_units.contains(&prev.id)
                        && same_unit_signature(&prev.components, &u_def.components)
                })
                .map(|prev| prev.id.clone())
        } else {
            None
        };
        let rhs = match alias_target {
            Some(alias_id) => alias_id,
            None => format_unit_components(&u_def.components, origin)?,
        };
        stmts.push(format!("unit {} = {};", u_def.id, rhs));
    }
    for (synth_id, rhs) in &synth_unit_defs {
        stmts.push(format!("unit {synth_id} = {rhs};"));
    }

    // 2. Emit model default unit definitions.
    for default in [
        ModelUnit::Substance,
        ModelUnit::Volume,
        ModelUnit::Area,
        ModelUnit::Length,
        ModelUnit::Extent,
        ModelUnit::Time,
    ] {
        if let Some(u_ref) = model.attrs.get(default.sbml_attribute()) {
            if u_ref == default.as_str() && unit_def_map.contains_key(u_ref) {
                continue;
            }
            let rhs = if UnitKind::from_name(u_ref).is_some() || unit_def_map.contains_key(u_ref) {
                u_ref.clone()
            } else {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!("model unit default {u_ref:?} references an unknown unit"),
                ));
            };
            stmts.push(format!("unit {} = {};", default.as_str(), rhs));
        }
    }

    // Track symbols targeted by initialAssignments, rules, and events.
    let mut init_assign_targets = BTreeSet::<String>::new();
    let mut ia_order = Vec::<String>::new();
    for ia in &initial_assignments {
        let sym = req_attr(ia, "symbol", origin)?.to_owned();
        init_assign_targets.insert(sym.clone());
        ia_order.push(sym);
    }
    let mut rule_targets = BTreeSet::<String>::new();
    let mut assign_rule_targets = BTreeSet::<String>::new();
    let mut rule_order = Vec::<String>::new();
    for r in &rules {
        if matches!(r.name.as_str(), "assignmentRule" | "rateRule") {
            let var = req_attr(r, "variable", origin)?.to_owned();
            if r.name == "assignmentRule" {
                assign_rule_targets.insert(var.clone());
            }
            rule_targets.insert(var.clone());
            rule_order.push(var);
        }
    }
    let target_order = merge_target_order(&ia_order, &rule_order);
    let mut event_targets = BTreeSet::<String>::new();
    for ev in &events {
        if let Some(loa) = ev.first_child_element("listOfEventAssignments") {
            for ea in loa.child_elements() {
                if let Some(v) = ea.attrs.get("variable") {
                    event_targets.insert(v.clone());
                }
            }
        }
    }

    let mut ordered_compartments: Vec<(usize, &XmlElement)> =
        compartments.iter().copied().enumerate().collect();
    ordered_compartments.sort_by_key(|(idx, c)| {
        let id = c.attrs.get("id").map(String::as_str).unwrap_or("");
        (target_order.get(id).copied().unwrap_or(usize::MAX), *idx)
    });

    // 3. Emit compartments.
    let mut comp_sizes = BTreeMap::<String, String>::new();
    for (_, c) in &ordered_compartments {
        let id = req_attr(c, "id", origin)?;
        if let Some(sz) = c.attrs.get("size") {
            comp_sizes.insert(id.to_owned(), sz.clone());
        }
        if id == "default_compartment" && has_implicit_default_comp && !species_list.is_empty() {
            continue;
        }
        let dims = c
            .attrs
            .get("spatialDimensions")
            .map(String::as_str)
            .unwrap_or("3");
        if !matches!(dims, "3" | "3.0") {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                "only 3D compartments (spatialDimensions=\"3\") are supported in this slice",
            ));
        }
        let constant = opt_bool_attr(c, "constant", true, origin)?;
        let qual = if !constant && !rule_targets.contains(id) && !event_targets.contains(id) {
            "var "
        } else {
            ""
        };
        let init_part = if !init_assign_targets.contains(id)
            && !assign_rule_targets.contains(id)
            && let Some(sz) = c.attrs.get("size")
        {
            format!(" = {sz}")
        } else {
            String::new()
        };
        stmts.push(format!("{qual}compartment {id}{init_part};"));
    }

    let mut ordered_species: Vec<(usize, &XmlElement)> =
        species_list.iter().copied().enumerate().collect();
    ordered_species.sort_by_key(|(idx, s)| {
        let id = s.attrs.get("id").map(String::as_str).unwrap_or("");
        let su_pos = s
            .attrs
            .get("substanceUnits")
            .and_then(|su| unit_defs.iter().position(|u| &u.id == su))
            .unwrap_or(usize::MAX);
        (
            target_order.get(id).copied().unwrap_or(usize::MAX),
            su_pos,
            *idx,
        )
    });

    // 4. Emit species.
    let mut boundary_species = BTreeSet::<String>::new();
    for (_, s) in &ordered_species {
        let id = req_attr(s, "id", origin)?;
        let comp = req_attr(s, "compartment", origin)?;
        let subst_only = opt_bool_attr(s, "hasOnlySubstanceUnits", false, origin)?;
        let boundary = opt_bool_attr(s, "boundaryCondition", false, origin)?;
        let constant = opt_bool_attr(s, "constant", false, origin)?;
        if constant && !(boundary && is_l2) {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!("species {id:?} has constant=\"true\", which is outside this slice"),
            ));
        }
        if boundary {
            boundary_species.insert(id.to_owned());
        }
        let subst_kw = if subst_only { "substanceOnly " } else { "" };
        let dollar = if boundary { "$" } else { "" };
        let comp_clause = if comp == "default_compartment" && has_implicit_default_comp {
            String::new()
        } else {
            format!(" in {comp}")
        };
        if s.attrs.contains_key("initialAmount") && s.attrs.contains_key("initialConcentration") {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                origin,
                format!(
                    "species {id:?} cannot specify both initialAmount and initialConcentration"
                ),
            ));
        }
        if !subst_only && s.attrs.contains_key("initialAmount") && !is_l2 {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!(
                    "concentration species {id:?} with initialAmount is outside this slice; use initialConcentration or hasOnlySubstanceUnits=\"true\""
                ),
            ));
        }
        if subst_only && s.attrs.contains_key("initialConcentration") {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!(
                    "substanceOnly species {id:?} with initialConcentration is outside this slice"
                ),
            ));
        }
        let init_clause = if !init_assign_targets.contains(id) && !assign_rule_targets.contains(id)
        {
            if let Some(conc) = s.attrs.get("initialConcentration") {
                format!(" = {conc}")
            } else if let Some(amt) = s.attrs.get("initialAmount") {
                if subst_only {
                    format!(" = {amt}")
                } else {
                    let c_sz = comp_sizes.get(comp).map(String::as_str).unwrap_or("1");
                    if matches!(c_sz, "1" | "1.0") {
                        format!(" = {amt}")
                    } else {
                        format!(" = {amt} / {comp}")
                    }
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        stmts.push(format!(
            "{subst_kw}species {dollar}{id}{comp_clause}{init_clause};"
        ));
    }

    let mut ordered_parameters: Vec<(usize, &XmlElement)> =
        parameters.iter().copied().enumerate().collect();
    ordered_parameters.sort_by_key(|(idx, p)| {
        let id = p.attrs.get("id").map(String::as_str).unwrap_or("");
        (target_order.get(id).copied().unwrap_or(usize::MAX), *idx)
    });

    // 5. Emit parameters.
    let mut global_symbols = BTreeSet::<String>::new();
    for c in &compartments {
        if let Some(id) = c.attrs.get("id") {
            global_symbols.insert(id.clone());
        }
    }
    for s in &species_list {
        if let Some(id) = s.attrs.get("id") {
            global_symbols.insert(id.clone());
        }
    }
    for p in &parameters {
        if let Some(id) = p.attrs.get("id") {
            global_symbols.insert(id.clone());
        }
    }
    for rxn in &reactions {
        if let Some(id) = rxn.attrs.get("id") {
            global_symbols.insert(id.clone());
        }
    }
    for ev in &events {
        if let Some(id) = ev.attrs.get("id") {
            global_symbols.insert(id.clone());
        }
    }

    for (_, p) in &ordered_parameters {
        let id = req_attr(p, "id", origin)?;
        let constant = opt_bool_attr(p, "constant", true, origin)?;
        let has_val = !init_assign_targets.contains(id)
            && !assign_rule_targets.contains(id)
            && p.attrs.contains_key("value");
        if !constant {
            if let Some(val) = p.attrs.get("value")
                && !init_assign_targets.contains(id)
                && !assign_rule_targets.contains(id)
            {
                if !rule_targets.contains(id) && !event_targets.contains(id) {
                    stmts.push(format!("var {id} = {val};"));
                } else {
                    stmts.push(format!("{id} = {val};"));
                }
            } else if !rule_targets.contains(id) && !event_targets.contains(id) {
                stmts.push(format!("var {id};"));
            } else {
                stmts.push(format!("const {id};"));
            }
        } else if has_val {
            let val = &p.attrs["value"];
            stmts.push(format!("{id} = {val};"));
        } else {
            stmts.push(format!("const {id};"));
        }
    }

    let mut scalar_attr_map = BTreeMap::<String, String>::new();
    for c in &compartments {
        if let (Some(id), Some(sz)) = (c.attrs.get("id"), c.attrs.get("size")) {
            scalar_attr_map.insert(id.clone(), sz.clone());
        }
    }
    for s in &species_list {
        if let Some(id) = s.attrs.get("id")
            && let Some(val) = s
                .attrs
                .get("initialConcentration")
                .or_else(|| s.attrs.get("initialAmount"))
        {
            scalar_attr_map.insert(id.clone(), val.clone());
        }
    }
    for p in &parameters {
        if let (Some(id), Some(val)) = (p.attrs.get("id"), p.attrs.get("value")) {
            scalar_attr_map.insert(id.clone(), val.clone());
        }
    }

    // 6. Emit initial assignments.
    let empty_renames = BTreeMap::<String, String>::new();
    for ia in &initial_assignments {
        let sym = req_attr(ia, "symbol", origin)?;
        let math_node = ia_math_root(ia, origin)?;
        let expr_str = decompile_math(math_node, &empty_renames, origin)?;
        // Wrap bare numeric literals in parentheses so Antimony preserves InitialAssignment semantics.
        let rhs = if matches!(math_node.name.as_str(), "cn") {
            format!("({expr_str})")
        } else {
            expr_str
        };
        stmts.push(format!("{sym} = {rhs};"));
    }

    // 7. Emit rules.
    for r in &rules {
        match r.name.as_str() {
            "assignmentRule" => {
                let var = req_attr(r, "variable", origin)?;
                let math_node = ia_math_root(r, origin)?;
                let expr_str = decompile_math(math_node, &empty_renames, origin)?;
                let rhs = if matches!(math_node.name.as_str(), "cn") {
                    match scalar_attr_map.get(var) {
                        None => format!("({expr_str})"),
                        Some(attr_val) if attr_val == "-0" && expr_str == "0" => "-0".to_owned(),
                        Some(_) => expr_str,
                    }
                } else {
                    expr_str
                };
                stmts.push(format!("{var} := {rhs};"));
            }
            "rateRule" => {
                let var = req_attr(r, "variable", origin)?;
                let math_node = ia_math_root(r, origin)?;
                let expr_str = decompile_math(math_node, &empty_renames, origin)?;
                stmts.push(format!("{var}' = {expr_str};"));
            }
            "algebraicRule" => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    "SBML algebraicRule is outside this slice",
                ));
            }
            other => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!("unexpected <{other}> in listOfRules"),
                ));
            }
        }
    }

    // 8. Emit reactions (and any reaction-scoped local parameters from SBML L2/L3).
    let mut lifted_local_params = BTreeMap::<String, (Option<String>, Option<String>)>::new();
    let mut lifted_param_unit_annotations = Vec::<(String, String)>::new();

    for rxn in &reactions {
        let id = req_attr(rxn, "id", origin)?;
        let reversible = opt_bool_attr(rxn, "reversible", true, origin)?;
        let mut reactants = Vec::<String>::new();
        let mut products = Vec::<String>::new();
        let mut kinetic_law: Option<&XmlElement> = None;
        let mut local_renames = BTreeMap::<String, String>::new();

        for child in rxn.child_elements() {
            match child.name.as_str() {
                "listOfReactants" => {
                    for sr in child.child_elements() {
                        reactants.push(decompile_species_ref(sr, &boundary_species, origin)?);
                    }
                }
                "listOfProducts" => {
                    for sr in child.child_elements() {
                        products.push(decompile_species_ref(sr, &boundary_species, origin)?);
                    }
                }
                "listOfModifiers" => {}
                "kineticLaw" => {
                    for kl_child in child.child_elements() {
                        match kl_child.name.as_str() {
                            "math" => {}
                            "listOfLocalParameters" | "listOfParameters" => {
                                for lp in kl_child.child_elements() {
                                    if !matches!(lp.name.as_str(), "localParameter" | "parameter") {
                                        return Err(error(
                                            DiagnosticCode::UnsupportedFeature,
                                            origin,
                                            format!(
                                                "unexpected <{}> in <{}>",
                                                lp.name, kl_child.name
                                            ),
                                        ));
                                    }
                                    let lp_id = req_attr(lp, "id", origin)?;
                                    let lp_val = lp.attrs.get("value").cloned();
                                    let lp_units = lp.attrs.get("units").cloned();
                                    let target_name = if !global_symbols.contains(lp_id) {
                                        if let Some(existing) = lifted_local_params.get(lp_id) {
                                            if existing == &(lp_val.clone(), lp_units.clone()) {
                                                lp_id.to_owned()
                                            } else {
                                                let prefixed = format!("{id}_{lp_id}");
                                                local_renames
                                                    .insert(lp_id.to_owned(), prefixed.clone());
                                                prefixed
                                            }
                                        } else {
                                            lp_id.to_owned()
                                        }
                                    } else {
                                        let prefixed = format!("{id}_{lp_id}");
                                        local_renames.insert(lp_id.to_owned(), prefixed.clone());
                                        prefixed
                                    };
                                    if !lifted_local_params.contains_key(&target_name) {
                                        lifted_local_params.insert(
                                            target_name.clone(),
                                            (lp_val.clone(), lp_units.clone()),
                                        );
                                        match lp_val {
                                            Some(v) => stmts.push(format!("{target_name} = {v};")),
                                            None => stmts.push(format!("const {target_name};")),
                                        }
                                        if let Some(u) = lp_units {
                                            lifted_param_unit_annotations.push((target_name, u));
                                        }
                                    }
                                }
                            }
                            _ => {
                                return Err(error(
                                    DiagnosticCode::UnsupportedFeature,
                                    origin,
                                    format!(
                                        "<{}> inside <kineticLaw> is outside this slice",
                                        kl_child.name
                                    ),
                                ));
                            }
                        }
                    }
                    kinetic_law = Some(child);
                }
                other => {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        origin,
                        format!("unexpected <{other}> in <reaction>"),
                    ));
                }
            }
        }
        let kl = kinetic_law.ok_or_else(|| {
            error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!("reaction {id:?} is missing a <kineticLaw>"),
            )
        })?;
        let math_node = ia_math_root(kl, origin)?;
        let rate_str = decompile_math(math_node, &local_renames, origin)?;
        let arrow = if reversible { "->" } else { "=>" };
        let lhs = reactants.join(" + ");
        let rhs = products.join(" + ");
        let participants = match (lhs.is_empty(), rhs.is_empty()) {
            (true, true) => arrow.to_owned(),
            (true, false) => format!("{arrow} {rhs}"),
            (false, true) => format!("{lhs} {arrow}"),
            (false, false) => format!("{lhs} {arrow} {rhs}"),
        };
        stmts.push(format!("{id}: {participants}; {rate_str};"));
    }

    // 9. Emit events.
    for ev in &events {
        let id = ev.attrs.get("id").map(String::as_str).unwrap_or("");
        let from_trigger = opt_bool_attr(ev, "useValuesFromTriggerTime", true, origin)?;
        let trigger_el = ev
            .first_child_element("trigger")
            .ok_or_else(|| error(DiagnosticCode::Syntax, origin, "event is missing <trigger>"))?;
        let initial_value = opt_bool_attr(trigger_el, "initialValue", true, origin)?;
        let persistent = opt_bool_attr(trigger_el, "persistent", true, origin)?;
        let trigger_math = ia_math_root(trigger_el, origin)?;
        let trigger_str = decompile_math(trigger_math, &empty_renames, origin)?;

        let delay_str = if let Some(delay_el) = ev.first_child_element("delay") {
            let d_math = ia_math_root(delay_el, origin)?;
            Some(decompile_math(d_math, &empty_renames, origin)?)
        } else {
            None
        };
        let priority_str = if let Some(prio_el) = ev.first_child_element("priority") {
            let p_math = ia_math_root(prio_el, origin)?;
            Some(decompile_math(p_math, &empty_renames, origin)?)
        } else {
            None
        };
        let loa = ev
            .first_child_element("listOfEventAssignments")
            .ok_or_else(|| {
                error(
                    DiagnosticCode::Syntax,
                    origin,
                    "event is missing <listOfEventAssignments>",
                )
            })?;
        let mut assigns = Vec::<String>::new();
        for ea in loa.child_elements() {
            if ea.name != "eventAssignment" {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!("unexpected <{}> in listOfEventAssignments", ea.name),
                ));
            }
            let var = req_attr(ea, "variable", origin)?;
            let ea_math = ia_math_root(ea, origin)?;
            let ea_expr = decompile_math(ea_math, &empty_renames, origin)?;
            assigns.push(format!("{var} = {ea_expr}"));
        }
        if assigns.is_empty() {
            return Err(error(
                DiagnosticCode::Syntax,
                origin,
                "event must contain at least one <eventAssignment>",
            ));
        }

        let prefix = if id.is_empty() {
            String::new()
        } else {
            format!("{id}: ")
        };
        let delay_part = match delay_str {
            Some(d) => format!("{d} after "),
            None => String::new(),
        };
        let mut mods = Vec::<String>::new();
        if !from_trigger {
            mods.push("fromTrigger = false".to_owned());
        }
        if !persistent {
            mods.push("persistent = false".to_owned());
        }
        if !initial_value {
            mods.push("t0 = false".to_owned());
        }
        if let Some(p) = priority_str {
            mods.push(format!("priority = {p}"));
        }
        let mod_part = if mods.is_empty() {
            String::new()
        } else {
            format!(", {}", mods.join(", "))
        };
        stmts.push(format!(
            "{prefix}at {delay_part}({trigger_str}){mod_part}: {};",
            assigns.join(", ")
        ));
    }

    // 10. Emit unit annotations (`has`).
    for c in &compartments {
        let id = req_attr(c, "id", origin)?;
        if let Some(u) = c.attrs.get("units") {
            stmts.push(format!("{id} has {u};"));
        }
    }
    for (sp_id, conc_u) in &species_unit_annotations {
        stmts.push(format!("{sp_id} has {conc_u};"));
    }
    for p in &parameters {
        let id = req_attr(p, "id", origin)?;
        if let Some(u) = p.attrs.get("units") {
            stmts.push(format!("{id} has {u};"));
        }
    }
    for (lp_id, u) in &lifted_param_unit_annotations {
        stmts.push(format!("{lp_id} has {u};"));
    }

    let mut out = String::new();
    for fb in &fn_blocks {
        out.push_str(fb);
        out.push('\n');
    }
    if model_id == "__main" {
        for s in &stmts {
            out.push_str(s);
            out.push('\n');
        }
    } else {
        out.push_str(&format!("model {model_id}()\n"));
        for s in &stmts {
            out.push_str("  ");
            out.push_str(s);
            out.push('\n');
        }
        out.push_str("end\n");
    }
    Ok(out)
}

fn merge_target_order(ia_order: &[String], rule_order: &[String]) -> BTreeMap<String, usize> {
    let mut merged = Vec::<String>::new();
    let mut seen = BTreeSet::<String>::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < ia_order.len() || j < rule_order.len() {
        if i < ia_order.len() && seen.contains(&ia_order[i]) {
            i += 1;
            continue;
        }
        if j < rule_order.len() && seen.contains(&rule_order[j]) {
            j += 1;
            continue;
        }
        if i < ia_order.len() && (j >= rule_order.len() || !rule_order[j..].contains(&ia_order[i]))
        {
            seen.insert(ia_order[i].clone());
            merged.push(ia_order[i].clone());
            i += 1;
        } else if j < rule_order.len() {
            seen.insert(rule_order[j].clone());
            merged.push(rule_order[j].clone());
            j += 1;
        }
    }
    merged
        .into_iter()
        .enumerate()
        .map(|(idx, sym)| (sym, idx))
        .collect()
}

fn decompile_function(f: &XmlElement, origin: Span) -> Result<String, Diagnostic> {
    let id = req_attr(f, "id", origin)?;
    let math = f.first_child_element("math").ok_or_else(|| {
        error(
            DiagnosticCode::Syntax,
            origin,
            format!("functionDefinition {id:?} is missing <math>"),
        )
    })?;
    if math.attrs.get("xmlns").map(String::as_str) != Some(MATHML_NS) {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "MathML <math> element must declare xmlns=\"http://www.w3.org/1998/Math/MathML\"",
        ));
    }
    let children: Vec<&XmlElement> = math.child_elements().collect();
    if children.len() != 1 || children[0].name != "lambda" {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            format!("functionDefinition {id:?} <math> must contain a single <lambda>"),
        ));
    }
    let lambda = children[0];
    let l_children: Vec<&XmlElement> = lambda.child_elements().collect();
    if l_children.is_empty() {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("functionDefinition {id:?} <lambda> is missing a body expression"),
        ));
    }
    let mut params = Vec::new();
    for bvar in &l_children[..l_children.len() - 1] {
        if bvar.name != "bvar" {
            return Err(error(
                DiagnosticCode::Syntax,
                origin,
                "expected <bvar> before lambda body in functionDefinition",
            ));
        }
        let ci = bvar.first_child_element("ci").ok_or_else(|| {
            error(
                DiagnosticCode::Syntax,
                origin,
                "<bvar> in functionDefinition must contain <ci>",
            )
        })?;
        params.push(ci.text_content());
    }
    let body_el = l_children[l_children.len() - 1];
    if body_el.name == "bvar" {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("functionDefinition {id:?} has no body expression"),
        ));
    }
    let empty_renames = BTreeMap::new();
    let body_str = decompile_math(body_el, &empty_renames, origin)?;
    Ok(format!(
        "function {id}({})\n  {body_str};\nend",
        params.join(", ")
    ))
}

fn parse_unit_definition(u: &XmlElement, origin: Span) -> Result<RawUnitDef, Diagnostic> {
    let id = req_attr(u, "id", origin)?.to_owned();
    let lou = u.first_child_element("listOfUnits").ok_or_else(|| {
        error(
            DiagnosticCode::Syntax,
            origin,
            format!("unitDefinition {id:?} is missing <listOfUnits>"),
        )
    })?;
    let mut components = Vec::new();
    for comp in lou.child_elements() {
        if comp.name != "unit" {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!("unexpected <{}> in listOfUnits", comp.name),
            ));
        }
        let kind = req_attr(comp, "kind", origin)?.to_owned();
        if UnitKind::from_name(&kind).is_none() {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                format!("unit kind {kind:?} is outside the supported primitive catalog"),
            ));
        }
        let exponent: i32 = comp
            .attrs
            .get("exponent")
            .map(String::as_str)
            .unwrap_or("1")
            .parse()
            .map_err(|_| error(DiagnosticCode::Syntax, origin, "invalid unit exponent"))?;
        let scale: i32 = comp
            .attrs
            .get("scale")
            .map(String::as_str)
            .unwrap_or("0")
            .parse()
            .map_err(|_| error(DiagnosticCode::Syntax, origin, "invalid unit scale"))?;
        let multiplier: f64 = comp
            .attrs
            .get("multiplier")
            .map(String::as_str)
            .unwrap_or("1")
            .parse()
            .map_err(|_| error(DiagnosticCode::Syntax, origin, "invalid unit multiplier"))?;
        components.push(RawUnitComponent {
            kind,
            exponent,
            scale,
            multiplier,
        });
    }
    Ok(RawUnitDef { id, components })
}

fn format_unit_components(
    components: &[RawUnitComponent],
    origin: Span,
) -> Result<String, Diagnostic> {
    if components.is_empty() {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            "unitDefinition must have at least one unit component",
        ));
    }
    // Order positive-exponent components before negative-exponent components if needed so the
    // first component can carry a leading coefficient or `1 / ...`.
    let mut ordered: Vec<&RawUnitComponent> = components.iter().collect();
    if ordered[0].exponent < 0
        && let Some(pos_idx) = ordered.iter().position(|c| c.exponent > 0)
    {
        ordered.swap(0, pos_idx);
    }

    let mut out = String::new();
    for (idx, c) in ordered.iter().enumerate() {
        let has_coeff = c.multiplier != 1.0 || c.scale != 0;
        if idx == 0 {
            if c.exponent < 0 {
                if has_coeff {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        origin,
                        "scaled reciprocal unit components without a positive base unit are outside this slice",
                    ));
                }
                let abs_exp = -c.exponent;
                if abs_exp == 1 {
                    out.push_str(&format!("1 / {}", c.kind));
                } else {
                    out.push_str(&format!("1 / {}^{abs_exp}", c.kind));
                }
            } else {
                if has_coeff {
                    if c.exponent != 1 {
                        return Err(error(
                            DiagnosticCode::UnsupportedFeature,
                            origin,
                            "scalar coefficients on powered units are outside this slice",
                        ));
                    }
                    let coeff = format_unit_coeff(c.multiplier, c.scale);
                    out.push_str(&format!("{coeff} {}", c.kind));
                } else if c.exponent == 1 {
                    out.push_str(&c.kind);
                } else {
                    out.push_str(&format!("{}^{}", c.kind, c.exponent));
                }
            }
        } else {
            if has_coeff {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    "internal scalar coefficients on non-leading unit components are outside this slice",
                ));
            }
            if c.exponent > 0 {
                if c.exponent == 1 {
                    out.push_str(&format!(" * {}", c.kind));
                } else {
                    out.push_str(&format!(" * {}^{}", c.kind, c.exponent));
                }
            } else {
                let abs_exp = -c.exponent;
                if abs_exp == 1 {
                    out.push_str(&format!(" / {}", c.kind));
                } else {
                    out.push_str(&format!(" / {}^{abs_exp}", c.kind));
                }
            }
        }
    }
    Ok(out)
}

fn format_unit_coeff(multiplier: f64, scale: i32) -> String {
    if scale == 0 {
        multiplier.to_string()
    } else if multiplier == 1.0 {
        format!("1e{scale}")
    } else {
        let product = multiplier * 10f64.powi(scale);
        product.to_string()
    }
}

fn resolve_unit_components(
    unit_name: &str,
    unit_def_map: &BTreeMap<String, &RawUnitDef>,
    origin: Span,
) -> Result<Vec<RawUnitComponent>, Diagnostic> {
    if let Some(kind) = UnitKind::from_name(unit_name) {
        Ok(vec![RawUnitComponent {
            kind: kind.as_str().to_owned(),
            exponent: 1,
            scale: 0,
            multiplier: 1.0,
        }])
    } else if let Some(def) = unit_def_map.get(unit_name) {
        Ok(def.components.clone())
    } else {
        Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("unknown unit reference {unit_name:?} in SBML document"),
        ))
    }
}

fn combined_unit_name(left: &str, right: &str) -> String {
    let mut id = format!("{left}_{right}");
    if id.ends_with('s') && !id.ends_with("_s") && !id.contains("siemens") && !id.contains("per_nM")
    {
        id.pop();
    }
    id
}

fn multiply_components(
    left: &[RawUnitComponent],
    right: &[RawUnitComponent],
) -> Vec<RawUnitComponent> {
    let mut components = left.to_vec();
    components.extend_from_slice(right);
    if components.len() > 2 {
        let mut first = 0;
        while first < components.len() {
            let found = (first + 1..components.len()).find(|&other| {
                let a = &components[first];
                let b = &components[other];
                a.kind == b.kind
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
    components
}

fn same_unit_signature(a: &[RawUnitComponent], b: &[RawUnitComponent]) -> bool {
    let sig = |cs: &[RawUnitComponent]| -> BTreeSet<(String, i32, i32, u64)> {
        cs.iter()
            .map(|c| (c.kind.clone(), c.exponent, c.scale, c.multiplier.to_bits()))
            .collect()
    };
    sig(a) == sig(b)
}

fn decompile_species_ref(
    sr: &XmlElement,
    _boundary_species: &BTreeSet<String>,
    origin: Span,
) -> Result<String, Diagnostic> {
    if sr.name != "speciesReference" {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("unexpected <{}> in reaction participant list", sr.name),
        ));
    }
    let species = req_attr(sr, "species", origin)?;
    let constant = opt_bool_attr(sr, "constant", true, origin)?;
    if !constant {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "variable stoichiometry (constant=\"false\") on speciesReference is outside this slice",
        ));
    }
    let stoich = sr
        .attrs
        .get("stoichiometry")
        .map(String::as_str)
        .unwrap_or("1");
    if stoich == "1" {
        Ok(species.to_owned())
    } else {
        Ok(format!("{stoich} {species}"))
    }
}

fn ia_math_root(parent: &XmlElement, origin: Span) -> Result<&XmlElement, Diagnostic> {
    let math = parent.first_child_element("math").ok_or_else(|| {
        error(
            DiagnosticCode::Syntax,
            origin,
            format!("<{}> is missing <math>", parent.name),
        )
    })?;
    if math.attrs.get("xmlns").map(String::as_str) != Some(MATHML_NS) {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            "MathML <math> element must declare xmlns=\"http://www.w3.org/1998/Math/MathML\"",
        ));
    }
    let children: Vec<&XmlElement> = math.child_elements().collect();
    if children.len() != 1 {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            "<math> must contain exactly one root MathML expression element",
        ));
    }
    Ok(children[0])
}

fn decompile_math(
    node: &XmlElement,
    renames: &BTreeMap<String, String>,
    origin: Span,
) -> Result<String, Diagnostic> {
    match node.name.as_str() {
        "true" => Ok("true".to_owned()),
        "false" => Ok("false".to_owned()),
        "pi" => Ok("pi".to_owned()),
        "exponentiale" => Ok("exponentiale".to_owned()),
        "ci" => {
            let ident = node.text_content();
            if ident.is_empty() {
                return Err(error(DiagnosticCode::Syntax, origin, "empty <ci> element"));
            }
            if let Some(mapped) = renames.get(&ident) {
                Ok(mapped.clone())
            } else {
                Ok(ident)
            }
        }
        "csymbol" => {
            if node.attrs.get("definitionURL").map(String::as_str) == Some(TIME_CSYMBOL_URL) {
                Ok("time".to_owned())
            } else {
                Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!(
                        "MathML <csymbol definitionURL={:?}> is outside this slice",
                        node.attrs.get("definitionURL")
                    ),
                ))
            }
        }
        "cn" => decompile_cn(node, origin),
        "piecewise" => decompile_piecewise(node, renames, origin),
        "apply" => decompile_apply(node, renames, origin),
        other => Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("MathML element <{other}> is outside this slice"),
        )),
    }
}

fn decompile_cn(node: &XmlElement, origin: Span) -> Result<String, Diagnostic> {
    let cn_type = node.attrs.get("type").map(String::as_str).unwrap_or("real");
    match cn_type {
        "integer" | "real" => {
            let txt = node.text_content();
            if txt.is_empty() {
                return Err(error(DiagnosticCode::Syntax, origin, "empty <cn> literal"));
            }
            Ok(txt)
        }
        "e-notation" => {
            let mut parts = Vec::new();
            for child in &node.children {
                if let XmlNode::Text(t) = child {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_owned());
                    }
                }
            }
            if parts.len() != 2 {
                return Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "malformed <cn type=\"e-notation\"> literal",
                ));
            }
            Ok(format!("{}e{}", parts[0], parts[1]))
        }
        "rational" => {
            let mut parts = Vec::new();
            for child in &node.children {
                if let XmlNode::Text(t) = child {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_owned());
                    }
                }
            }
            if parts.len() != 2 {
                return Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "malformed <cn type=\"rational\"> literal",
                ));
            }
            let num = &parts[0];
            let den = &parts[1];
            if let Some(stripped) = num.strip_prefix('-') {
                Ok(format!("-(({stripped} / {den}))"))
            } else {
                Ok(format!("({num} / {den})"))
            }
        }
        other => Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("MathML <cn type={other:?}> is outside this slice"),
        )),
    }
}

fn decompile_piecewise(
    node: &XmlElement,
    renames: &BTreeMap<String, String>,
    origin: Span,
) -> Result<String, Diagnostic> {
    let mut args = Vec::<String>::new();
    for child in node.child_elements() {
        match child.name.as_str() {
            "piece" => {
                let elems: Vec<&XmlElement> = child.child_elements().collect();
                if elems.len() != 2 {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        origin,
                        "<piece> must have value and condition children",
                    ));
                }
                args.push(decompile_math(elems[0], renames, origin)?);
                args.push(decompile_math(elems[1], renames, origin)?);
            }
            "otherwise" => {
                let elems: Vec<&XmlElement> = child.child_elements().collect();
                if elems.len() != 1 {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        origin,
                        "<otherwise> must have one value child",
                    ));
                }
                args.push(decompile_math(elems[0], renames, origin)?);
            }
            other => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    origin,
                    format!("unexpected <{other}> inside <piecewise>"),
                ));
            }
        }
    }
    Ok(format!("piecewise({})", args.join(", ")))
}

fn decompile_apply(
    node: &XmlElement,
    renames: &BTreeMap<String, String>,
    origin: Span,
) -> Result<String, Diagnostic> {
    let children: Vec<&XmlElement> = node.child_elements().collect();
    if children.is_empty() {
        return Err(error(
            DiagnosticCode::Syntax,
            origin,
            "empty <apply> element in MathML",
        ));
    }
    let op = children[0];
    let args = &children[1..];

    match op.name.as_str() {
        "plus" => decompile_nary_infix(args, "+", renames, origin),
        "times" => decompile_nary_infix(args, "*", renames, origin),
        "minus" => {
            if args.len() == 1 {
                let inner = decompile_math(args[0], renames, origin)?;
                Ok(format!("(-({inner}))"))
            } else if args.len() == 2 {
                let l = decompile_math(args[0], renames, origin)?;
                let r = decompile_math(args[1], renames, origin)?;
                Ok(format!("(({l}) - ({r}))"))
            } else {
                Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "<minus/> requires 1 or 2 arguments",
                ))
            }
        }
        "divide" => {
            if args.len() != 2 {
                return Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "<divide/> requires 2 arguments",
                ));
            }
            let l = decompile_math(args[0], renames, origin)?;
            let r = decompile_math(args[1], renames, origin)?;
            Ok(format!("(({l}) / ({r}))"))
        }
        "power" => {
            if args.len() != 2 {
                return Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "<power/> requires 2 arguments",
                ));
            }
            let l = decompile_math(args[0], renames, origin)?;
            let r = decompile_math(args[1], renames, origin)?;
            Ok(format!("(({l})^({r}))"))
        }
        "not" => {
            if args.len() != 1 {
                return Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "<not/> requires 1 argument",
                ));
            }
            let inner = decompile_math(args[0], renames, origin)?;
            Ok(format!("(!({inner}))"))
        }
        "eq" | "neq" | "lt" | "leq" | "gt" | "geq" | "and" | "or" => {
            let mut rendered = Vec::with_capacity(args.len());
            for a in args {
                rendered.push(decompile_math(a, renames, origin)?);
            }
            Ok(format!("{}({})", op.name, rendered.join(", ")))
        }
        "root" => {
            if args.len() == 2 && args[0].name == "degree" {
                let deg_children: Vec<&XmlElement> = args[0].child_elements().collect();
                if deg_children.len() == 1
                    && deg_children[0].name == "cn"
                    && deg_children[0].text_content() == "2"
                {
                    let val = decompile_math(args[1], renames, origin)?;
                    return Ok(format!("sqrt({val})"));
                }
            }
            Err(error(
                DiagnosticCode::UnsupportedFeature,
                origin,
                "only degree-2 <root/> (sqrt) is supported in this slice",
            ))
        }
        "log" => {
            if args.len() == 1 && args[0].name != "logbase" {
                let val = decompile_math(args[0], renames, origin)?;
                Ok(format!("log10({val})"))
            } else if args.len() == 2 && args[0].name == "logbase" {
                let base_children: Vec<&XmlElement> = args[0].child_elements().collect();
                if base_children.len() != 1 {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        origin,
                        "<logbase> must have 1 child",
                    ));
                }
                let base_el = base_children[0];
                let val = decompile_math(args[1], renames, origin)?;
                if base_el.name == "cn" && base_el.text_content() == "10" {
                    Ok(format!("log10({val})"))
                } else {
                    let base = decompile_math(base_el, renames, origin)?;
                    Ok(format!("log({base}, {val})"))
                }
            } else {
                Err(error(
                    DiagnosticCode::Syntax,
                    origin,
                    "malformed <log/> application in MathML",
                ))
            }
        }
        "ln" | "sin" | "cos" | "tan" | "exp" | "abs" | "floor" | "ceiling" | "min" | "max" => {
            let mut rendered = Vec::with_capacity(args.len());
            for a in args {
                rendered.push(decompile_math(a, renames, origin)?);
            }
            Ok(format!("{}({})", op.name, rendered.join(", ")))
        }
        "ci" => {
            let fn_name = op.text_content();
            let mut rendered = Vec::with_capacity(args.len());
            for a in args {
                rendered.push(decompile_math(a, renames, origin)?);
            }
            Ok(format!("{fn_name}({})", rendered.join(", ")))
        }
        other => Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("MathML operator <{other}/> is outside this slice"),
        )),
    }
}

fn decompile_nary_infix(
    args: &[&XmlElement],
    op_str: &str,
    renames: &BTreeMap<String, String>,
    origin: Span,
) -> Result<String, Diagnostic> {
    if args.len() < 2 {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("n-ary MathML operator {op_str:?} requires at least 2 operands"),
        ));
    }
    let mut rendered = Vec::with_capacity(args.len());
    for a in args {
        let s = decompile_math(a, renames, origin)?;
        // Preserve nested same-operator group boundaries vs left-associative flat chain.
        let is_same_op = a.name == "apply"
            && a.child_elements().next().is_some_and(|first| {
                (op_str == "+" && first.name == "plus") || (op_str == "*" && first.name == "times")
            });
        if is_same_op {
            rendered.push(format!("({s})"));
        } else {
            rendered.push(s);
        }
    }
    Ok(format!("({})", rendered.join(&format!(" {op_str} "))))
}

fn req_attr<'a>(el: &'a XmlElement, key: &str, origin: Span) -> Result<&'a str, Diagnostic> {
    el.attrs.get(key).map(String::as_str).ok_or_else(|| {
        error(
            DiagnosticCode::Syntax,
            origin,
            format!("<{}> is missing required attribute {key:?}", el.name),
        )
    })
}

fn opt_bool_attr(
    el: &XmlElement,
    key: &str,
    default: bool,
    origin: Span,
) -> Result<bool, Diagnostic> {
    let Some(val) = el.attrs.get(key) else {
        return Ok(default);
    };
    match val.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(error(
            DiagnosticCode::Syntax,
            origin,
            format!(
                "<{}> attribute {key:?} must be \"true\" or \"false\", found {other:?}",
                el.name
            ),
        )),
    }
}
